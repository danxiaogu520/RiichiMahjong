use riichi_core::game::{
    CallOption, CallType, GameEvent, ResponseAction, RoundEndReason, TurnAction,
};
use riichi_core::meld::{Meld, MeldKind};
use riichi_core::player::PlayerId;
use riichi_core::tile::Tile;
use riichi_logic::analysis::analyze_wait_tiles;

use crate::game::{GameError, GamePhase, GameState};

impl GameState {
    /// 记录某位玩家在当前响应窗口选择 Pass，但不结束整个响应窗口。
    ///
    /// 服务端收集多人响应时使用；最终无人鸣牌时仍由普通 Pass 流程统一
    /// 推进回合和处理所有玩家的临时振听。
    pub fn record_response_pass(&mut self, player: PlayerId) -> Result<(), GameError> {
        let discarded_tile = match self.phase {
            GamePhase::ResponsePhase {
                discarded_tile,
                discarder,
            } if player != discarder => discarded_tile,
            GamePhase::ChankanResponse {
                kakan_tile,
                kakan_player,
                ..
            } if player != kakan_player => kakan_tile,
            _ => {
                return Err(GameError::InvalidAction(
                    "当前不能记录响应 Pass".to_string(),
                ));
            }
        };

        let waiting = self.get_waiting_tile_types(player);
        if waiting.contains(&discarded_tile.tile_type()) {
            if self.players[player.0].is_riichi {
                self.players[player.0].furiten.riichi = true;
            } else {
                self.players[player.0].furiten.round = true;
            }
        }
        Ok(())
    }

    /// 执行玩家的行动（行动阶段）
    ///
    /// 支持的行动类型：
    /// - Discard: 打牌
    /// - RiichiDiscard: 立直宣言 + 打牌
    /// - Tsumo: 自摸和
    /// - KyuushuKyuuhai: 九种九牌（流局）
    /// - Ankan: 暗杠
    /// - Kakan: 加杠
    pub fn execute_action(&mut self, action: TurnAction) -> Result<Vec<GameEvent>, GameError> {
        self.validate_action(
            self.current_player,
            &crate::legal::LegalAction::Turn(action.clone()),
        )?;
        // 检查是否处于行动阶段
        if !matches!(self.phase, GamePhase::ActionPhase) {
            return Err(GameError::InvalidAction("不在行动阶段".to_string()));
        }

        let mut new_events = Vec::new();

        match action {
            // 打牌
            TurnAction::Discard(tile) => {
                self.discard(tile)?;
            }

            // 立直宣言 + 打牌
            TurnAction::RiichiDiscard(tile) => {
                // 检查是否满足立直条件
                if !self.can_declare_riichi(self.current_player) {
                    return Err(GameError::InvalidAction("不满足立直条件".to_string()));
                }
                // 提交自摸牌到手牌（hand 13→14），以便做听牌检查
                self.insert_tile();
                // 检查牌在手中
                if !self.players[self.current_player.0].hand.contains(tile) {
                    return Err(GameError::TileNotInHand(tile));
                }
                // 检查打出后是否听牌（hand 有 14 张，打一张剩 13 张）
                let mut simulated = self.players[self.current_player.0].hand.clone();
                simulated
                    .remove(tile)
                    .map_err(|_| GameError::TileNotInHand(tile))?;
                if analyze_wait_tiles(simulated.tiles()).is_empty() {
                    return Err(GameError::InvalidAction(
                        "立直宣言牌必须使手牌听牌".to_string(),
                    ));
                }
                // 宣告立直
                {
                    let p = &mut self.players[self.current_player.0];
                    p.points -= 1000; // 放置立直棒
                    p.is_riichi = true;
                }
                self.riichi_sticks += 1;
                new_events.push(GameEvent::PlayerDeclaredRiichi {
                    player: self.current_player,
                });
                // 打出宣言牌
                self.discard(tile)?;

                // 四风连打检查（立直宣言牌也参与判定）
                if matches!(self.phase, GamePhase::ResponsePhase { .. })
                    && self.check_suufon_renda()
                {
                    self.resolve_round_end(RoundEndReason::SuufonRenda);
                }
                // 四家立直检查（第四家立直宣言后，且未被荣和取消）
                else if matches!(self.phase, GamePhase::ResponsePhase { .. })
                    && self.rules.suucha_riichi_abort
                    && self.check_suucha_riichi()
                {
                    self.resolve_round_end(RoundEndReason::SuuchaRiichi);
                }
            }

            // 自摸和
            TurnAction::Tsumo => {
                let winning_tile = self.drawn_tile.ok_or_else(|| {
                    GameError::InvalidAction("没有摸到的牌，无法自摸".to_string())
                })?;
                let result = self.check_win(self.current_player, true, winning_tile, None, false);
                if let Some((changes, yaku_names)) = result {
                    self.insert_tile(); // 提交自摸牌到手牌
                                        // 应用点数变化
                    for (i, &change) in changes.iter().enumerate() {
                        self.players[i].points += change;
                    }
                    self.riichi_sticks = 0;
                    new_events.push(GameEvent::PlayerWon {
                        player: self.current_player,
                        is_tsumo: true,
                        points: changes[self.current_player.0],
                        yaku_names,
                    });
                    self.resolve_round_end(RoundEndReason::Win {
                        winner: self.current_player,
                        is_tsumo: true,
                    });
                } else {
                    return Err(GameError::InvalidAction("无法自摸和".to_string()));
                }
            }

            // 九种九牌（流局）
            TurnAction::KyuushuKyuuhai => {
                if !self.can_declare_kyuushu(self.current_player) {
                    return Err(GameError::InvalidAction("不满足九种九牌条件".to_string()));
                }
                self.resolve_round_end(RoundEndReason::KyuushuKyuuhai);
            }

            // 暗杠
            TurnAction::Ankan(tile) => {
                self.insert_tile(); // 提交自摸牌到手牌（暗杠需 4 张在手）
                let events = self.execute_ankan(self.current_player, tile)?;
                new_events.extend(events);
            }

            // 加杠
            TurnAction::Kakan(meld_index, tile) => {
                self.insert_tile(); // 提交自摸牌到手牌（加杠需手牌中有第 4 张）
                let events = self.execute_kakan(self.current_player, meld_index, tile)?;
                new_events.extend(events);
            }
        }

        self.record_events(&new_events);
        Ok(new_events)
    }

    /// 一次结算多个荣和者。
    ///
    /// 所有赢家都由同一放铳者支付各自手牌点数，但场上立直棒只在本次
    /// 和牌中支付一次，交给响应顺序中的第一位赢家。
    pub fn execute_multiple_ron(
        &mut self,
        winners: &[PlayerId],
    ) -> Result<Vec<GameEvent>, GameError> {
        let (discarded_tile, discarder) = match self.phase {
            GamePhase::ResponsePhase {
                discarded_tile,
                discarder,
            } => (discarded_tile, discarder),
            _ => {
                return Err(GameError::InvalidAction("当前不在荣和响应阶段".to_string()));
            }
        };
        if winners.is_empty() {
            return Err(GameError::InvalidAction("没有荣和者".to_string()));
        }

        let mut results = Vec::with_capacity(winners.len());
        for &winner in winners {
            if winner == discarder {
                return Err(GameError::InvalidAction("放铳者不能荣和".to_string()));
            }
            let result = self
                .check_win(winner, false, discarded_tile, Some(discarder), false)
                .ok_or_else(|| GameError::InvalidAction("存在无效的荣和".to_string()))?;
            results.push((winner, result));
        }

        let riichi_bonus = self.riichi_sticks * 1000;
        let mut events = Vec::new();
        for (index, (winner, (mut changes, yaku_names))) in results.into_iter().enumerate() {
            if index > 0 {
                changes[winner.0] -= riichi_bonus as i32;
            }
            for (player_index, change) in changes.iter().enumerate() {
                self.players[player_index].points += change;
            }
            self.players[winner.0].hand.add(discarded_tile);
            events.push(GameEvent::PlayerWon {
                player: winner,
                is_tsumo: false,
                points: changes[winner.0],
                yaku_names,
            });
        }
        self.riichi_sticks = 0;
        self.record_events(&events);
        self.resolve_round_end(RoundEndReason::MultiWin {
            winners: winners.to_vec(),
        });
        Ok(events)
    }

    /// 获取当前玩家可执行的副露选项（响应阶段）
    ///
    /// 根据当前阶段返回可选的副露操作：
    /// - ResponsePhase: 检测吃/碰/杠/荣和
    /// - ChankanResponse: 仅检测抢杠荣和
    pub fn get_call_options(&self) -> Vec<CallOption> {
        match self.phase {
            GamePhase::ResponsePhase {
                discarded_tile,
                discarder,
            } => {
                let mut options =
                    crate::call::detect_calls(&self.players, discarded_tile, discarder);
                // 仅完成牌型不代表可以荣和：还必须满足振听、至少一役和
                // 当前副露上下文。候选动作必须与真正结算使用同一判定入口。
                options.retain(|option| {
                    !matches!(&option.call_type, CallType::Ron)
                        || self
                            .check_win(option.player, false, discarded_tile, Some(discarder), false)
                            .is_some()
                });
                options
            }
            GamePhase::ChankanResponse {
                kakan_tile,
                kakan_player,
                ..
            } => {
                // 抢杠荣和：仅检测荣和，不检测吃/碰/杠
                let mut options = Vec::new();
                for idx in 0..4 {
                    let pid = PlayerId(idx);
                    if pid == kakan_player {
                        continue;
                    }
                    if self
                        .check_win(pid, false, kakan_tile, Some(kakan_player), true)
                        .is_some()
                    {
                        options.push(CallOption {
                            player: pid,
                            call_type: CallType::Ron,
                        });
                    }
                }
                options
            }
            _ => Vec::new(),
        }
    }

    /// 执行副露响应（响应阶段）
    ///
    /// 根据当前阶段分发到对应的处理函数：
    /// - ResponsePhase: 普通响应（吃/碰/杠/荣和/过）
    /// - ChankanResponse: 抢杠响应（仅荣和/过）
    pub fn execute_call(
        &mut self,
        player: PlayerId,
        action: ResponseAction,
    ) -> Result<Vec<GameEvent>, GameError> {
        self.validate_action(player, &crate::legal::LegalAction::Response(action.clone()))?;
        self.execute_call_inner(player, action)
    }

    /// 完成响应窗口的 Pass。
    ///
    /// 这是服务端在所有有资格响应的玩家都 Pass 后调用的内部推进动作，
    /// 不是玩家动作，因此允许由当前弃牌者/加杠者完成窗口。
    pub fn complete_response_pass(&mut self) -> Result<Vec<GameEvent>, GameError> {
        let player = match self.phase {
            GamePhase::ResponsePhase { discarder, .. } => discarder,
            GamePhase::ChankanResponse { kakan_player, .. } => kakan_player,
            _ => return Err(GameError::InvalidAction("不在响应阶段".to_string())),
        };
        self.execute_call_inner(player, ResponseAction::Pass)
    }

    fn execute_call_inner(
        &mut self,
        player: PlayerId,
        action: ResponseAction,
    ) -> Result<Vec<GameEvent>, GameError> {
        let mut new_events = Vec::new();

        match self.phase {
            GamePhase::ResponsePhase {
                discarded_tile,
                discarder,
            } => {
                self.execute_response_call(
                    player,
                    action,
                    discarded_tile,
                    discarder,
                    &mut new_events,
                )?;
            }
            GamePhase::ChankanResponse {
                kakan_tile,
                kakan_player,
                meld_index,
            } => {
                self.execute_chankan_call(
                    player,
                    action,
                    kakan_tile,
                    kakan_player,
                    meld_index,
                    &mut new_events,
                )?;
            }
            _ => return Err(GameError::InvalidAction("不在响应阶段".to_string())),
        }

        self.record_events(&new_events);
        Ok(new_events)
    }

    /// 处理普通响应阶段（吃/碰/杠/荣和/过）
    fn execute_response_call(
        &mut self,
        player: PlayerId,
        action: ResponseAction,
        discarded_tile: Tile,
        discarder: PlayerId,
        new_events: &mut Vec<GameEvent>,
    ) -> Result<(), GameError> {
        match action {
            // 过：将牌放入舍牌区，更新振听，进入摸牌阶段
            ResponseAction::Pass => {
                self.players[discarder.0].discards.push(discarded_tile);

                // 更新其他玩家的振听状态
                for idx in 0..4 {
                    let pid = PlayerId(idx);
                    if pid == discarder {
                        continue;
                    }
                    let waiting = self.get_waiting_tile_types(pid);
                    if waiting.contains(&discarded_tile.tile_type()) {
                        if self.players[idx].is_riichi {
                            self.players[idx].furiten.riichi = true;
                        } else {
                            self.players[idx].furiten.round = true;
                        }
                    }
                }

                self.update_all_discard_furiten();
                self.advance_turn();
                self.phase = GamePhase::DrawPhase;
            }
            // 荣和
            ResponseAction::Ron => {
                let result = self.check_win(player, false, discarded_tile, Some(discarder), false);
                if let Some((changes, yaku_names)) = result {
                    self.players[player.0].hand.add(discarded_tile);
                    // 应用点数变化
                    for (i, &change) in changes.iter().enumerate() {
                        self.players[i].points += change;
                    }
                    // 本局和牌后，场上供托由赢家取得；结算结果已经包含供托点数。
                    self.riichi_sticks = 0;
                    new_events.push(GameEvent::PlayerWon {
                        player,
                        is_tsumo: false,
                        points: changes[player.0],
                        yaku_names,
                    });
                    self.resolve_round_end(RoundEndReason::Win {
                        winner: player,
                        is_tsumo: false,
                    });
                } else {
                    // 荣和不成立（振听/无役等），将牌放入舍牌区
                    self.players[discarder.0].discards.push(discarded_tile);
                    self.update_all_discard_furiten();
                    self.advance_turn();
                    self.phase = GamePhase::DrawPhase;
                }
            }
            // 碰
            ResponseAction::Pon { hand_tiles } => {
                {
                    let p = &mut self.players[player.0];
                    for &tile in &hand_tiles {
                        p.hand
                            .remove(tile)
                            .map_err(|_| GameError::TileNotInHand(tile))?;
                    }
                    let mut meld_tiles = hand_tiles.to_vec();
                    meld_tiles.push(discarded_tile);
                    let meld = Meld::pon(meld_tiles, discarded_tile, discarder);
                    p.melds.push(meld);
                }
                self.current_player = player;
                self.phase = GamePhase::ActionPhase;
                self.kuikae_forbidden[player.0] = vec![discarded_tile.tile_type()];
                self.update_discard_furiten(player);
                new_events.push(GameEvent::PlayerCalledPon {
                    player,
                    tiles: hand_tiles.to_vec(),
                    from_player: discarder,
                });
            }
            // 吃（仅下家可用）
            ResponseAction::Chi { hand_tiles } => {
                {
                    let p = &mut self.players[player.0];
                    for &tile in &hand_tiles {
                        p.hand
                            .remove(tile)
                            .map_err(|_| GameError::TileNotInHand(tile))?;
                    }
                    let mut meld_tiles = hand_tiles.to_vec();
                    meld_tiles.push(discarded_tile);
                    let meld = Meld::chi(meld_tiles, discarded_tile, discarder);
                    p.melds.push(meld);
                }
                self.current_player = player;
                self.phase = GamePhase::ActionPhase;
                self.kuikae_forbidden[player.0] = vec![discarded_tile.tile_type()];
                self.update_discard_furiten(player);
                new_events.push(GameEvent::PlayerCalledChi {
                    player,
                    tiles: hand_tiles.to_vec(),
                    from_player: discarder,
                });
            }
            // 大明杠
            ResponseAction::Minkan { hand_tiles } => {
                if self.get_kan_count() >= 4 {
                    return Err(GameError::InvalidAction(
                        "四杠限制：不能继续开杠".to_string(),
                    ));
                }

                {
                    let p = &mut self.players[player.0];
                    for &tile in &hand_tiles {
                        p.hand
                            .remove(tile)
                            .map_err(|_| GameError::TileNotInHand(tile))?;
                    }
                    let mut meld_tiles = hand_tiles.to_vec();
                    meld_tiles.push(discarded_tile);
                    p.melds
                        .push(Meld::minkan(meld_tiles, discarded_tile, discarder));
                }
                self.current_player = player;
                self.reveal_dora_indicator();
                self.draw_rinshan()?; // 杠后补摸岭上牌
                new_events.push(GameEvent::PlayerCalledMinkan {
                    player,
                    tiles: hand_tiles.to_vec(),
                    from_player: discarder,
                });
                // 四杠散了检查
                if self.rules.suukan_sanra_abort && self.check_four_kan_abort() {
                    self.resolve_round_end(RoundEndReason::SuuKantsu);
                }
            }
        }

        Ok(())
    }

    /// 处理抢杠荣和响应阶段（仅荣和/过）
    fn execute_chankan_call(
        &mut self,
        player: PlayerId,
        action: ResponseAction,
        kakan_tile: Tile,
        kakan_player: PlayerId,
        meld_index: usize,
        new_events: &mut Vec<GameEvent>,
    ) -> Result<(), GameError> {
        match action {
            // 过：杠成立，摸岭上牌，进入行动阶段
            ResponseAction::Pass => {
                self.current_player = kakan_player;
                // 加杠只有在抢杠窗口结束后才正式成立；此时才翻开杠宝牌。
                self.reveal_dora_indicator();
                self.draw_rinshan()?;

                if self.rules.suukan_sanra_abort && self.check_four_kan_abort() {
                    self.resolve_round_end(RoundEndReason::SuuKantsu);
                } else {
                    self.phase = GamePhase::ActionPhase;
                }
            }
            // 抢杠荣和
            ResponseAction::Ron => {
                // 此杠不成立，副露恢复为碰
                {
                    let meld = &mut self.players[kakan_player.0].melds[meld_index];
                    debug_assert!(meld.kind == MeldKind::Kakan);
                    meld.tiles.pop(); // 移除第 4 张牌
                    meld.kind = MeldKind::Pon; // 恢复为碰
                }

                let result = self.check_win(player, false, kakan_tile, Some(kakan_player), true);
                if let Some((changes, yaku_names)) = result {
                    self.players[player.0].hand.add(kakan_tile);
                    // 应用点数变化
                    for (i, &change) in changes.iter().enumerate() {
                        self.players[i].points += change;
                    }
                    self.riichi_sticks = 0;
                    new_events.push(GameEvent::PlayerWon {
                        player,
                        is_tsumo: false,
                        points: changes[player.0],
                        yaku_names,
                    });
                    self.resolve_round_end(RoundEndReason::Win {
                        winner: player,
                        is_tsumo: false,
                    });
                }
            }
            _ => {
                return Err(GameError::InvalidAction(
                    "抢杠响应阶段只能荣和或过".to_string(),
                ));
            }
        }
        Ok(())
    }

    /// 获取当前玩家可执行的暗杠选项
    ///
    /// 考虑手牌（3n+1）与自摸牌缓冲区中的牌
    /// 手牌中有 4 张相同牌，或手牌 3 张 + 自摸牌 1 张
    pub fn get_ankan_options(&self, player: PlayerId) -> Vec<Tile> {
        if self.players[player.0].is_riichi {
            return self.get_riichi_ankan_options(player);
        }
        let hand = &self.players[player.0].hand;
        let mut seen = std::collections::HashSet::new();
        let mut options = Vec::new();
        for &tile in hand.tiles() {
            let tt = tile.tile_type();
            if seen.insert(tt) && hand.count_type(tt.0) == 4 {
                options.push(tile);
            }
        }
        // 自摸牌可能与手牌 3 张组合成暗杠（3+1=4）
        if let Some(drawn) = self.drawn_tile {
            let drawn_tt = drawn.tile_type();
            if !options.iter().any(|t| t.tile_type() == drawn_tt)
                && hand.count_type(drawn_tt.0) == 3
            {
                options.push(drawn);
            }
        }
        options
    }

    /// 执行暗杠
    ///
    /// 流程：
    /// 1. 检查手中是否有 4 张相同牌
    /// 2. 检查四杠限制
    /// 3. 立直后暗杠限制（不能改变听牌种类）
    /// 4. 从手牌移除 4 张牌，创建暗杠副露
    /// 5. 翻宝牌指示牌
    /// 6. 补摸岭上牌
    /// 7. 四杠散了检查
    pub fn execute_ankan(
        &mut self,
        player: PlayerId,
        tile: Tile,
    ) -> Result<Vec<GameEvent>, GameError> {
        let tt = tile.tile_type();
        if self.players[player.0].hand.count_type(tt.0) < 4 {
            return Err(GameError::InvalidAction("手中没有 4 张相同牌".to_string()));
        }

        // 四杠限制
        if self.get_kan_count() >= 4 {
            return Err(GameError::InvalidAction(
                "四杠限制：不能继续开杠".to_string(),
            ));
        }

        // 立直后暗杠限制
        if self.players[player.0].is_riichi {
            let valid_tiles = self.get_riichi_ankan_options(player);
            if !valid_tiles.iter().any(|t| t.tile_type() == tt) {
                return Err(GameError::InvalidAction(
                    "立直后暗杠不改变听牌种类".to_string(),
                ));
            }
        }

        // 从手牌移除 4 张牌
        let tiles_to_remove: Vec<Tile> = self.players[player.0]
            .hand
            .tiles()
            .iter()
            .filter(|t| t.tile_type() == tt)
            .take(4)
            .copied()
            .collect();

        {
            let p = &mut self.players[player.0];
            for &t in &tiles_to_remove {
                p.hand.remove(t).map_err(|_| GameError::TileNotInHand(t))?;
            }
            p.melds.push(Meld::ankan(tiles_to_remove.clone()));
        }

        let new_events = vec![GameEvent::PlayerCalledAnkan {
            player,
            tiles: tiles_to_remove,
        }];

        self.reveal_dora_indicator();
        self.current_player = player;
        self.draw_rinshan()?;

        // 四杠散了检查
        if self.rules.suukan_sanra_abort && self.check_four_kan_abort() {
            self.resolve_round_end(RoundEndReason::SuuKantsu);
        }

        Ok(new_events)
    }

    /// 获取当前玩家可执行的加杠选项
    ///
    /// 考虑手牌（3n+1）与自摸牌缓冲区中的牌
    /// 手牌或自摸牌中有与碰副露相同类型的牌
    pub fn get_kakan_options(&self, player: PlayerId) -> Vec<(usize, Tile)> {
        let p = &self.players[player.0];
        let mut options = Vec::new();
        for (i, meld) in p.melds.iter().enumerate() {
            if meld.kind == riichi_core::meld::MeldKind::Pon {
                let tt = meld.tiles[0].tile_type();
                // 手牌中有匹配的牌
                if let Some(&tile) = p.hand.tiles().iter().find(|t| t.tile_type() == tt) {
                    options.push((i, tile));
                }
                // 自摸牌也可能匹配碰副露
                if let Some(drawn) = self.drawn_tile {
                    if drawn.tile_type() == tt {
                        options.push((i, drawn));
                    }
                }
            }
        }
        options
    }

    /// 执行加杠（将碰升级为加杠）
    ///
    /// 流程：
    /// 1. 检查该副露是否为碰
    /// 2. 检查牌是否匹配
    /// 3. 检查四杠限制
    /// 4. 从手牌移除第 4 张牌，将碰升级为加杠
    /// 5. 翻宝牌指示牌
    /// 6. 进入抢杠荣和响应阶段（不立即摸岭上牌）
    pub fn execute_kakan(
        &mut self,
        player: PlayerId,
        meld_index: usize,
        tile: Tile,
    ) -> Result<Vec<GameEvent>, GameError> {
        // 验证副露是否为碰
        {
            let meld = &self.players[player.0].melds[meld_index];
            if meld.kind != riichi_core::meld::MeldKind::Pon {
                return Err(GameError::InvalidAction("该副露不是碰".to_string()));
            }
            let tt = meld.tiles[0].tile_type();
            if tile.tile_type() != tt {
                return Err(GameError::InvalidAction("牌与碰副露不匹配".to_string()));
            }
        }

        // 四杠限制
        if self.get_kan_count() >= 4 {
            return Err(GameError::InvalidAction(
                "四杠限制：不能继续开杠".to_string(),
            ));
        }

        // 执行加杠
        let original_pon;
        {
            let p = &mut self.players[player.0];
            p.hand
                .remove(tile)
                .map_err(|_| GameError::TileNotInHand(tile))?;

            let meld = &mut p.melds[meld_index];
            original_pon = meld.tiles.clone();
            let from_player = meld.from_player;
            let called_tile = meld.called_tile;
            let mut new_tiles = original_pon.clone();
            new_tiles.push(tile);
            *meld = Meld {
                kind: riichi_core::meld::MeldKind::Kakan,
                tiles: new_tiles,
                called_tile,
                from_player,
            };
        }

        let new_events = vec![GameEvent::PlayerCalledKakan {
            player,
            tile,
            original_pon,
        }];

        self.current_player = player;
        // 进入抢杠荣和响应阶段。抢杠成立时，杠宝牌才会翻开，
        // 因此这里不能提前调用 reveal_dora_indicator。
        self.phase = GamePhase::ChankanResponse {
            kakan_tile: tile,
            kakan_player: player,
            meld_index,
        };

        Ok(new_events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use riichi_core::game::CallType;
    use riichi_core::hand::Hand;
    use riichi_core::meld::Meld;

    #[test]
    fn kakan_does_not_reveal_dora_before_chankan_passes() {
        let mut state = GameState::new();
        let mut rng = rand::rngs::StdRng::seed_from_u64(7);
        state.start_round(&mut rng);

        let tile = Tile::from_raw(0);
        for existing in state.players[0].hand.tiles().to_vec().into_iter().take(4) {
            state.players[0].hand.remove(existing).unwrap();
        }
        for _ in 0..4 {
            state.players[0].hand.add(tile);
        }
        state.players[0]
            .melds
            .push(Meld::pon(vec![tile; 3], tile, PlayerId(1)));
        state.current_player = PlayerId(0);

        let initial_dora_count = state.dora.len();
        state.execute_kakan(PlayerId(0), 0, tile).unwrap();
        assert_eq!(state.dora.len(), initial_dora_count);

        let mut events = Vec::new();
        state
            .execute_chankan_call(
                PlayerId(1),
                ResponseAction::Pass,
                tile,
                PlayerId(0),
                0,
                &mut events,
            )
            .unwrap();
        assert_eq!(state.dora.len(), initial_dora_count + 1);
    }

    #[test]
    fn response_pass_by_discarder_advances_to_draw_phase() {
        let mut state = GameState::new();
        let mut rng = rand::rngs::StdRng::seed_from_u64(31);
        state.start_round(&mut rng);
        let drawn = state.drawn_tile.unwrap();

        state.execute_action(TurnAction::Discard(drawn)).unwrap();
        assert!(matches!(state.phase, GamePhase::ResponsePhase { .. }));
        state.complete_response_pass().unwrap();
        assert!(matches!(state.phase, GamePhase::DrawPhase));
    }

    #[test]
    fn open_white_dragon_pon_allows_ron_on_completing_tile() {
        let mut state = GameState::new();
        let mut rng = rand::rngs::StdRng::seed_from_u64(43);
        state.start_round(&mut rng);
        let white = Tile::from_raw(124);
        state.players[1].melds.push(Meld::pon(
            vec![white, Tile::from_raw(125), Tile::from_raw(126)],
            white,
            PlayerId(0),
        ));
        state.players[1].hand = Hand::from_tiles(&[
            Tile::from_raw(0),
            Tile::from_raw(4),
            Tile::from_raw(8),
            Tile::from_raw(12),
            Tile::from_raw(16),
            Tile::from_raw(20),
            Tile::from_raw(21),
            Tile::from_raw(24),
            Tile::from_raw(28),
            Tile::from_raw(36),
        ]);
        state.phase = GamePhase::ResponsePhase {
            discarded_tile: Tile::from_raw(37),
            discarder: PlayerId(0),
        };

        let options = state.get_call_options();
        assert!(options.iter().any(|option| {
            option.player == PlayerId(1) && matches!(option.call_type, CallType::Ron)
        }));
    }

    #[test]
    fn shape_only_wait_without_yaku_does_not_offer_ron() {
        let mut state = GameState::new();
        let mut rng = rand::rngs::StdRng::seed_from_u64(59);
        state.start_round(&mut rng);
        state.players[1].hand = Hand::from_tiles(&[
            Tile::from_raw(0),
            Tile::from_raw(4),
            Tile::from_raw(8),
            Tile::from_raw(12),
            Tile::from_raw(16),
            Tile::from_raw(20),
            Tile::from_raw(24),
            Tile::from_raw(28),
            Tile::from_raw(21),
            Tile::from_raw(60),
            Tile::from_raw(64),
            Tile::from_raw(68),
            Tile::from_raw(40),
        ]);
        let discarded_tile = Tile::from_raw(41);
        state.phase = GamePhase::ResponsePhase {
            discarded_tile,
            discarder: PlayerId(0),
        };

        let options = state.get_call_options();
        assert!(!options.iter().any(|option| {
            option.player == PlayerId(1) && matches!(option.call_type, CallType::Ron)
        }));
    }
}
