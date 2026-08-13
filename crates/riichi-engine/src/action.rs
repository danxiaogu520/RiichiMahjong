use riichi_core::game::{
    CallKind, CallOption, CallType, EventEnvelope, GameEvent, ResponseAction, RoundEndReason,
    TurnAction, WinKind,
};
use riichi_core::meld::{Meld, MeldKind};
use riichi_core::player::PlayerId;
use riichi_core::tile::Tile;
use riichi_logic::model::TileCounts;
use riichi_logic::shape::{analyze_wait_tiles, is_kokushi};

use crate::game::{extract_kuikae_tiles, GameError, GamePhase, GameState};

impl GameState {
    /// Resolve a turn command against a cloned state, then apply only the
    /// resulting authoritative events to the live state.
    pub fn execute_action_event_sourced(
        &mut self,
        action: TurnAction,
    ) -> Result<Vec<EventEnvelope>, GameError> {
        let mut candidate = self.clone();
        let start = candidate.event_log().len();
        candidate.execute_action(action)?;
        let events = candidate.event_log()[start..].to_vec();
        for envelope in &events {
            let applied = EventEnvelope {
                event_id: self.event_log().len() as u64 + 1,
                event: envelope.event.clone(),
            };
            self.apply_event(&applied)
                .map_err(GameError::InvalidAction)?;
            self.event_log.push(applied);
        }
        Ok(events)
    }

    pub fn execute_call_event_sourced(
        &mut self,
        player: PlayerId,
        action: ResponseAction,
    ) -> Result<Vec<EventEnvelope>, GameError> {
        let mut candidate = self.clone();
        let start = candidate.event_log().len();
        candidate.execute_call(player, action)?;
        let events = candidate.event_log()[start..].to_vec();
        for envelope in &events {
            let applied = EventEnvelope {
                event_id: self.event_log().len() as u64 + 1,
                event: envelope.event.clone(),
            };
            self.apply_event(&applied)
                .map_err(GameError::InvalidAction)?;
            self.event_log.push(applied);
        }
        Ok(events)
    }

    pub fn execute_multiple_ron_event_sourced(
        &mut self,
        winners: &[PlayerId],
    ) -> Result<Vec<EventEnvelope>, GameError> {
        let mut candidate = self.clone();
        let start = candidate.event_log().len();
        candidate.execute_multiple_ron(winners)?;
        let events = candidate.event_log()[start..].to_vec();
        for envelope in &events {
            let applied = EventEnvelope {
                event_id: self.event_log().len() as u64 + 1,
                event: envelope.event.clone(),
            };
            self.apply_event(&applied)
                .map_err(GameError::InvalidAction)?;
            self.event_log.push(applied);
        }
        Ok(events)
    }

    /// 更新大三元/大四喜的责任支付者。
    ///
    /// Mortal 在当前碰/大明杠处理完成后再检查副露集合；因此只有导致
    /// 第三个三元牌刻子或第四个风牌刻子成立的那次鸣牌，才记录放出该牌
    /// 的玩家。暗杠和加杠不触发新的包牌责任。
    fn update_pao_after_open_call(
        &mut self,
        player: PlayerId,
        from_player: PlayerId,
        called_tile: Tile,
    ) {
        let melds = &self.players[player.0].melds;
        let has_open_triplet = |tile_type: riichi_core::tile::TileType| {
            melds.iter().any(|meld| {
                matches!(meld.kind, MeldKind::Pon | MeldKind::Minkan)
                    && meld.tiles.iter().any(|tile| tile.tile_type() == tile_type)
            })
        };
        let dragons_complete =
            (31..34).all(|tile_type| has_open_triplet(riichi_core::tile::TileType(tile_type)));
        let winds_complete =
            (27..31).all(|tile_type| has_open_triplet(riichi_core::tile::TileType(tile_type)));
        let called_type = called_tile.tile_type();
        if (dragons_complete && called_type.is_dragon())
            || (winds_complete && called_type.is_wind())
        {
            self.pao_targets[player.0] = Some(from_player.0);
        }
    }

    /// 记录某位玩家在当前响应窗口选择 Pass，但不结束整个响应窗口。
    ///
    /// 服务端收集多人响应时使用；最终无人鸣牌时仍由普通 Pass 流程统一
    /// 推进回合和处理所有玩家的临时振听。
    pub fn record_response_pass(&mut self, player: PlayerId) -> Result<(), GameError> {
        let discarded_tile = match self.phase {
            GamePhase::ResponsePhase {
                discarded_tile,
                player: discarder,
            } if player != discarder => discarded_tile,
            GamePhase::ChankanResponse {
                kan_tile: kakan_tile,
                player: kakan_player,
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
        self.record_event(GameEvent::Pass { player });
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
        let current_player = self
            .current_player()
            .ok_or_else(|| GameError::InvalidAction("当前没有行动玩家".to_string()))?;
        self.validate_action(
            current_player,
            &crate::legal::LegalAction::Turn(action.clone()),
        )?;
        // 检查是否处于行动阶段
        if !matches!(self.phase, GamePhase::ActionPhase { .. }) {
            return Err(GameError::InvalidAction("不在行动阶段".to_string()));
        }

        let mut new_events = Vec::new();

        match action {
            // 打牌
            TurnAction::Discard(tile) => {
                self.discard(tile)?;
                // 四风连打检查：所有玩家的首巡弃牌（含普通弃牌）均参与判定
                if matches!(self.phase, GamePhase::ResponsePhase { .. })
                    && self.check_suufon_renda()
                {
                    new_events.push(GameEvent::AbortiveDraw {
                        player: None,
                        reason: RoundEndReason::SuufonRenda,
                    });
                    self.resolve_round_end(RoundEndReason::SuufonRenda);
                }
            }

            // 立直宣言 + 打牌
            TurnAction::RiichiDiscard(tile) => {
                // 检查是否满足立直条件
                if !self.can_declare_riichi(current_player) {
                    return Err(GameError::InvalidAction("不满足立直条件".to_string()));
                }
                // 提交自摸牌到手牌（hand 13→14），以便做听牌检查
                self.insert_tile();
                // 检查牌在手中
                if !self.players[current_player.0].hand.contains(tile) {
                    return Err(GameError::TileNotInHand(tile));
                }
                // 检查打出后是否听牌（hand 有 14 张，打一张剩 13 张）
                let mut simulated = self.players[current_player.0].hand.clone();
                simulated
                    .remove(tile)
                    .map_err(|_| GameError::TileNotInHand(tile))?;
                if analyze_wait_tiles(simulated.tiles()).is_empty() {
                    return Err(GameError::InvalidAction(
                        "立直宣言牌必须使手牌听牌".to_string(),
                    ));
                }
                // 宣告立直作为一个动作事件应用，随后再执行宣言牌弃牌。
                self.apply_riichi_event(current_player)?;
                self.record_event(GameEvent::Riichi {
                    player: current_player,
                });
                // 打出宣言牌
                self.discard(tile)?;

                // 四风连打检查（立直宣言牌也参与判定）
                if matches!(self.phase, GamePhase::ResponsePhase { .. })
                    && self.check_suufon_renda()
                {
                    new_events.push(GameEvent::AbortiveDraw {
                        player: None,
                        reason: RoundEndReason::SuufonRenda,
                    });
                    self.resolve_round_end(RoundEndReason::SuufonRenda);
                }
                // 四家立直在第四家立直被受理（宣言牌通过响应窗口或被鸣牌）后检查，
                // 不再在宣言时立即流局，宣言牌被荣和时和牌优先。
            }

            // 自摸和
            TurnAction::Tsumo => {
                let winning_tile = self.drawn_tile().ok_or_else(|| {
                    GameError::InvalidAction("没有摸到的牌，无法自摸".to_string())
                })?;
                let result = self.check_win(current_player, true, winning_tile, None, false);
                if let Some((changes, yaku_names)) = result {
                    self.insert_tile(); // 提交自摸牌到手牌
                                        // 应用点数变化
                    for (i, &change) in changes.iter().enumerate() {
                        self.players[i].points += change;
                    }
                    self.round_win_details = yaku_names
                        .into_iter()
                        .map(|detail| {
                            format!(
                                "{}家：{}",
                                ["东", "南", "西", "北"][current_player.0],
                                detail
                            )
                        })
                        .collect();
                    self.riichi_sticks = 0;
                    new_events.push(GameEvent::Win {
                        winners: vec![current_player],
                        tile: winning_tile,
                        kind: WinKind::Tsumo,
                        loser: None,
                    });
                    self.resolve_round_end(RoundEndReason::Win {
                        winner: current_player,
                        is_tsumo: true,
                    });
                } else {
                    return Err(GameError::InvalidAction("无法自摸和".to_string()));
                }
            }

            // 九种九牌（流局）
            TurnAction::KyuushuKyuuhai => {
                if !self.can_declare_kyuushu(current_player) {
                    return Err(GameError::InvalidAction("不满足九种九牌条件".to_string()));
                }
                new_events.push(GameEvent::AbortiveDraw {
                    player: Some(current_player),
                    reason: RoundEndReason::KyuushuKyuuhai,
                });
                self.resolve_round_end(RoundEndReason::KyuushuKyuuhai);
            }

            // 暗杠
            TurnAction::Ankan(tile) => {
                self.insert_tile(); // 提交自摸牌到手牌（暗杠需 4 张在手）
                let events = self.execute_ankan(current_player, tile)?;
                new_events.extend(events);
            }

            // 加杠
            TurnAction::Kakan(meld_index, tile) => {
                self.insert_tile(); // 提交自摸牌到手牌（加杠需手牌中有第 4 张）
                let events = self.execute_kakan(current_player, meld_index, tile)?;
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
                player: discarder,
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
        let honba_bonus = self.honba * 300;
        for (index, (winner, (mut changes, yaku_names))) in results.into_iter().enumerate() {
            // Mortal 的多家荣和规则：本场棒和立直棒只在第一位赢家
            // 的这次荣和中结算，后续赢家只取得本身的和牌点数。
            if index > 0 {
                changes[winner.0] -= riichi_bonus as i32;
                changes[winner.0] -= honba_bonus as i32;
                // 普通荣和的本场棒由放铳者支付；包牌荣和的本场棒
                // 已包含在责任支付者的负分中，因此要退回对应的一方。
                let honba_payer = self.pao_targets[winner.0].unwrap_or(discarder.0);
                changes[honba_payer] += honba_bonus as i32;
            }
            for (player_index, change) in changes.iter().enumerate() {
                self.players[player_index].points += change;
            }
            self.round_win_details.extend(
                yaku_names
                    .into_iter()
                    .map(|detail| format!("{}家：{}", ["东", "南", "西", "北"][winner.0], detail)),
            );
            self.players[winner.0]
                .hand
                .add(discarded_tile)
                .map_err(|error| GameError::InvalidAction(error.to_string()))?;
        }
        self.riichi_sticks = 0;
        let event = GameEvent::Win {
            winners: winners.to_vec(),
            tile: discarded_tile,
            kind: WinKind::Ron,
            loser: Some(discarder),
        };
        self.record_event(event.clone());
        self.resolve_round_end(RoundEndReason::MultiWin {
            winners: winners.to_vec(),
        });
        Ok(vec![event])
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
                player: discarder,
            } => {
                let mut options = crate::call::detect_calls(
                    &self.players,
                    discarded_tile,
                    discarder,
                    self.remaining_tiles(),
                );
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
                kan_tile,
                player: kakan_player,
                kind,
            } => {
                // 抢杠荣和：仅检测荣和，不检测吃/碰/杠。
                // 加杠可被任意和牌抢杠；暗杠只有国士无双可以抢（其他情况不允许）。
                let mut options = Vec::new();
                for idx in 0..4 {
                    let pid = PlayerId(idx);
                    if pid == kakan_player {
                        continue;
                    }
                    let can_rob = match kind {
                        CallKind::Kakan => true,
                        CallKind::Ankan => {
                            let p = &self.players[idx];
                            p.melds.is_empty() && {
                                let mut counts = TileCounts::from_tiles(p.hand.tiles());
                                counts.inc(kan_tile.tile_type());
                                is_kokushi(&counts)
                            }
                        }
                        _ => false,
                    };
                    if can_rob
                        && self
                            .check_win(pid, false, kan_tile, Some(kakan_player), true)
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
            GamePhase::ResponsePhase { player, .. } | GamePhase::ChankanResponse { player, .. } => {
                player
            }
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
                player: discarder,
            } => {
                // 四杠散了：第四杠后的舍牌只能被荣和；被鸣牌（吃/碰/明杠）时直接流局。
                // 荣和与过牌分别在其流程内处理。
                if self.four_kan_abort_pending
                    && self.check_four_kan_abort()
                    && !matches!(action, ResponseAction::Pass | ResponseAction::Ron)
                {
                    new_events.push(GameEvent::AbortiveDraw {
                        player: None,
                        reason: RoundEndReason::SuuKantsu,
                    });
                    self.resolve_round_end(RoundEndReason::SuuKantsu);
                    return Ok(new_events);
                }
                self.execute_response_call(
                    player,
                    action,
                    discarded_tile,
                    discarder,
                    &mut new_events,
                )?;
            }
            GamePhase::ChankanResponse {
                kan_tile,
                player: kakan_player,
                kind,
            } => {
                self.execute_chankan_call(
                    player,
                    action,
                    kan_tile,
                    kakan_player,
                    kind,
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
                // 四杠散了：第四杠后的舍牌未被荣和时流局。
                // 与 Mortal 一致，该判定优先于立直受理（四家立直）。
                if self.four_kan_abort_pending && self.check_four_kan_abort() {
                    new_events.push(GameEvent::AbortiveDraw {
                        player: None,
                        reason: RoundEndReason::SuuKantsu,
                    });
                    self.resolve_round_end(RoundEndReason::SuuKantsu);
                    return Ok(());
                }

                // 立直受理：宣言牌通过响应窗口未被荣和时扣立直棒（与 Mortal 一致）。
                self.accept_riichi(discarder);

                let river_index = self.players[discarder.0].discards.len();
                self.players[discarder.0].discards.push(discarded_tile);
                // 立直宣言牌被鸣走时，以立直后第一张入河的牌代替横置标记。
                if self.players[discarder.0].is_riichi
                    && self.players[discarder.0].riichi_declaration_index.is_none()
                {
                    self.players[discarder.0].riichi_declaration_index = Some(river_index);
                }

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

                // 四家立直：第四家立直宣言牌通过响应窗口后立直被受理，受理即流局。
                // 最后一巡（剩余 0 张）时立直不会被受理，优先荒牌流局。
                if self.check_suucha_riichi() && self.remaining_tiles() > 0 {
                    new_events.push(GameEvent::AbortiveDraw {
                        player: None,
                        reason: RoundEndReason::SuuchaRiichi,
                    });
                    self.resolve_round_end(RoundEndReason::SuuchaRiichi);
                    return Ok(());
                }

                self.advance_turn();
            }
            // 荣和
            ResponseAction::Ron => {
                let result = self.check_win(player, false, discarded_tile, Some(discarder), false);
                if let Some((changes, yaku_names)) = result {
                    self.players[player.0]
                        .hand
                        .add(discarded_tile)
                        .map_err(|error| GameError::InvalidAction(error.to_string()))?;
                    // 应用点数变化
                    for (i, &change) in changes.iter().enumerate() {
                        self.players[i].points += change;
                    }
                    self.round_win_details = yaku_names
                        .into_iter()
                        .map(|detail| {
                            format!("{}家：{}", ["东", "南", "西", "北"][player.0], detail)
                        })
                        .collect();
                    // 本局和牌后，场上供托由赢家取得；结算结果已经包含供托点数。
                    self.riichi_sticks = 0;
                    new_events.push(GameEvent::Win {
                        winners: vec![player],
                        tile: discarded_tile,
                        kind: WinKind::Ron,
                        loser: Some(discarder),
                    });
                    self.resolve_round_end(RoundEndReason::Win {
                        winner: player,
                        is_tsumo: false,
                    });
                } else {
                    // 荣和不成立（振听/无役等），将牌放入舍牌区
                    // 宣言牌未被有效荣和，立直照常受理。
                    self.accept_riichi(discarder);
                    let river_index = self.players[discarder.0].discards.len();
                    self.players[discarder.0].discards.push(discarded_tile);
                    if self.players[discarder.0].is_riichi
                        && self.players[discarder.0].riichi_declaration_index.is_none()
                    {
                        self.players[discarder.0].riichi_declaration_index = Some(river_index);
                    }
                    self.update_all_discard_furiten();
                    self.advance_turn();
                }
            }
            // 碰
            ResponseAction::Pon { hand_tiles } => {
                let forbidden;
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
                    forbidden = extract_kuikae_tiles(&meld);
                    p.melds.push(meld);
                }
                self.phase = GamePhase::ActionPhase {
                    player,
                    drawn_tile: None,
                };
                self.kuikae_forbidden[player.0] = forbidden;
                self.update_discard_furiten(player);
                // 立直宣言牌被碰时立直接被受理（与 Mortal 一致）
                self.accept_riichi(discarder);
                new_events.push(GameEvent::Call {
                    player,
                    tiles: hand_tiles.to_vec(),
                    kind: riichi_core::game::CallKind::Pon,
                    called_tile: Some(discarded_tile),
                    from_player: Some(discarder),
                    meld_index: None,
                });
                self.update_pao_after_open_call(player, discarder, discarded_tile);
                // 第四家立直宣言牌被鸣牌时立直接被受理，受理后立即流局
                if self.check_suucha_riichi() {
                    new_events.push(GameEvent::AbortiveDraw {
                        player: None,
                        reason: RoundEndReason::SuuchaRiichi,
                    });
                    self.resolve_round_end(RoundEndReason::SuuchaRiichi);
                    return Ok(());
                }
            }
            // 吃（仅下家可用）
            ResponseAction::Chi { hand_tiles } => {
                let forbidden;
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
                    forbidden = extract_kuikae_tiles(&meld);
                    p.melds.push(meld);
                }
                self.phase = GamePhase::ActionPhase {
                    player,
                    drawn_tile: None,
                };
                self.kuikae_forbidden[player.0] = forbidden;
                self.update_discard_furiten(player);
                // 立直宣言牌被吃时立直接被受理（与 Mortal 一致）
                self.accept_riichi(discarder);
                new_events.push(GameEvent::Call {
                    player,
                    tiles: hand_tiles.to_vec(),
                    kind: riichi_core::game::CallKind::Chi,
                    called_tile: Some(discarded_tile),
                    from_player: Some(discarder),
                    meld_index: None,
                });
                // 第四家立直宣言牌被鸣牌时立直接被受理，受理后立即流局
                if self.check_suucha_riichi() {
                    new_events.push(GameEvent::AbortiveDraw {
                        player: None,
                        reason: RoundEndReason::SuuchaRiichi,
                    });
                    self.resolve_round_end(RoundEndReason::SuuchaRiichi);
                    return Ok(());
                }
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
                // 大明杠不可被抢：杠立即成立，先记录鸣牌事件再翻宝牌并补摸岭上。
                // 立直宣言牌被明杠时立直接被受理（与 Mortal 一致）。
                self.accept_riichi(discarder);
                self.record_event(GameEvent::Call {
                    player,
                    tiles: hand_tiles.to_vec(),
                    kind: riichi_core::game::CallKind::Minkan,
                    called_tile: Some(discarded_tile),
                    from_player: Some(discarder),
                    meld_index: None,
                });
                self.phase = GamePhase::DrawPhase {
                    player,
                    position: riichi_core::game::DrawPosition::Rinshan,
                };
                // 明杠的宝牌不在杠成立时翻开：与 Mortal 一致，
                // 延迟到杠家下一次舍牌时翻开（岭上自摸和牌不计该宝牌）。
                self.dora_reveal_at_discard = true;
                self.draw_rinshan()?;
                self.update_pao_after_open_call(player, discarder, discarded_tile);
                // 四杠散了：第四杠成立后挂起，待下一次舍牌未被荣和时才流局
                if self.check_four_kan_abort() {
                    self.four_kan_abort_pending = true;
                }
                // 第四家立直宣言牌被明杠时立直接被受理，受理后立即流局
                if self.check_suucha_riichi() {
                    new_events.push(GameEvent::AbortiveDraw {
                        player: None,
                        reason: RoundEndReason::SuuchaRiichi,
                    });
                    self.resolve_round_end(RoundEndReason::SuuchaRiichi);
                    return Ok(());
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
        kan_kind: CallKind,
        new_events: &mut Vec<GameEvent>,
    ) -> Result<(), GameError> {
        match action {
            // 过：杠成立，摸岭上牌，进入行动阶段
            ResponseAction::Pass => {
                self.phase = GamePhase::DrawPhase {
                    player: kakan_player,
                    position: riichi_core::game::DrawPosition::Rinshan,
                };
                // 暗杠的宝牌在杠成立时已翻开；加杠只有在抢杠窗口结束后才翻开。
                if kan_kind == CallKind::Kakan {
                    self.reveal_dora_indicator();
                }
                self.draw_rinshan()?;

                // 四杠散了：第四杠成立后挂起，待下一次舍牌未被荣和时才流局
                if self.check_four_kan_abort() {
                    self.four_kan_abort_pending = true;
                }
            }
            // 抢杠荣和
            ResponseAction::Ron => {
                let meld_index = self.players[kakan_player.0]
                    .melds
                    .iter()
                    .position(|meld| {
                        matches!(meld.kind, MeldKind::Ankan | MeldKind::Kakan)
                            && meld
                                .tiles
                                .iter()
                                .any(|tile| tile.tile_type() == kakan_tile.tile_type())
                    })
                    .ok_or_else(|| GameError::InvalidAction("找不到待抢杠副露".to_string()))?;
                let meld = self.players[kakan_player.0].melds.remove(meld_index);
                match meld.kind {
                    MeldKind::Kakan => {
                        let mut tiles = meld.tiles;
                        tiles.pop();
                        self.players[kakan_player.0].melds.push(Meld {
                            kind: MeldKind::Pon,
                            tiles,
                            called_tile: meld.called_tile,
                            from_player: meld.from_player,
                        });
                    }
                    MeldKind::Ankan => {
                        for tile in meld.tiles {
                            self.players[kakan_player.0]
                                .hand
                                .add(tile)
                                .map_err(|error| GameError::InvalidAction(error.to_string()))?;
                        }
                        // 被抢的暗杠不成立：回滚杠成立时立即翻开的宝牌指示牌
                        self.dora.pop();
                        self.dora_indicators.pop();
                        self.ura_dora_indicators.pop();
                    }
                    _ => unreachable!(),
                }

                let result = self.check_win(player, false, kakan_tile, Some(kakan_player), true);
                if let Some((changes, yaku_names)) = result {
                    self.players[player.0]
                        .hand
                        .add(kakan_tile)
                        .map_err(|error| GameError::InvalidAction(error.to_string()))?;
                    // 应用点数变化
                    for (i, &change) in changes.iter().enumerate() {
                        self.players[i].points += change;
                    }
                    self.round_win_details = yaku_names
                        .into_iter()
                        .map(|detail| {
                            format!("{}家：{}", ["东", "南", "西", "北"][player.0], detail)
                        })
                        .collect();
                    self.riichi_sticks = 0;
                    new_events.push(GameEvent::Win {
                        winners: vec![player],
                        tile: kakan_tile,
                        kind: WinKind::Ron,
                        loser: Some(kakan_player),
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
        if self.remaining_tiles() == 0 {
            return vec![];
        }
        if self.players[player.0].is_riichi {
            return self.get_riichi_ankan_options(player);
        }
        let hand = &self.players[player.0].hand;
        let mut seen = std::collections::HashSet::new();
        let mut options = Vec::new();
        for &tile in hand.tiles() {
            let tt = tile.tile_type();
            if seen.insert(tt) && hand.count_type(tt) == 4 {
                options.push(tile);
            }
        }
        // 自摸牌可能与手牌 3 张组合成暗杠（3+1=4）
        if let Some(drawn) = self.drawn_tile() {
            let drawn_tt = drawn.tile_type();
            if !options.iter().any(|t| t.tile_type() == drawn_tt) && hand.count_type(drawn_tt) == 3
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
        if self.remaining_tiles() == 0 {
            return Err(GameError::InvalidAction("海底牌不能暗杠".to_string()));
        }
        let tt = tile.tile_type();
        let available = self.players[player.0].hand.count_type(tt)
            + usize::from(
                self.drawn_tile()
                    .is_some_and(|drawn| drawn.tile_type() == tt),
            );
        if available < 4 {
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

        // 行动阶段的摸牌暂存在 drawn_tile。暗杠后它仍属于手牌，
        // 因此先并入手牌，再统一移除四张杠牌；若摸到的正是杠牌，
        // 该牌也会被一并移入暗杠。
        self.insert_tile();

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

        let new_events = vec![GameEvent::Call {
            player,
            tiles: tiles_to_remove,
            kind: riichi_core::game::CallKind::Ankan,
            called_tile: None,
            from_player: None,
            meld_index: None,
        }];

        // 暗杠的宝牌在杠成立时立即翻开；若被国士无双抢杠，抢杠时回滚。
        self.reveal_dora_indicator();
        self.phase = GamePhase::ChankanResponse {
            player,
            kan_tile: tile,
            kind: riichi_core::game::CallKind::Ankan,
        };

        Ok(new_events)
    }

    /// 获取当前玩家可执行的加杠选项
    ///
    /// 考虑手牌（3n+1）与自摸牌缓冲区中的牌
    /// 手牌或自摸牌中有与碰副露相同类型的牌
    pub fn get_kakan_options(&self, player: PlayerId) -> Vec<(usize, Tile)> {
        if self.remaining_tiles() == 0 {
            return vec![];
        }
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
                if let Some(drawn) = self.drawn_tile() {
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
        if self.remaining_tiles() == 0 {
            return Err(GameError::InvalidAction("海底牌不能加杠".to_string()));
        }
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

        // 加杠使用摸到的第四张牌时，先把摸牌并入手牌；若使用手牌中的
        // 第四张牌，原有摸牌也必须保留在手牌中。
        self.insert_tile();

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

        let new_events = vec![GameEvent::Call {
            player,
            tiles: vec![tile],
            kind: riichi_core::game::CallKind::Kakan,
            called_tile: Some(tile),
            from_player: None,
            meld_index: Some(meld_index),
        }];

        // 进入抢杠荣和响应窗口。加杠只有抢杠窗口关闭后才正式成立，
        // 因此杠宝牌在窗口关闭时翻开；若被抢杠则恢复为碰。
        self.phase = GamePhase::ChankanResponse {
            player,
            kan_tile: tile,
            kind: riichi_core::game::CallKind::Kakan,
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
    use riichi_core::tile::TileType;

    #[test]
    fn riichi_is_accepted_and_stick_deducted_when_declaration_discard_is_called() {
        // 双立直宣言牌被碰走后：立直接被受理（扣 1000、放棒），
        // 且双立直标记在宣言时捕获，不受之后鸣牌影响（与 Mortal 一致）。
        let mut state = GameState::new();
        let mut rng = rand::rngs::StdRng::seed_from_u64(23);
        state.start_round(&mut rng);
        // 庄家 123m 456m 789m 11p 34s + 摸 2s，打 2s 立直听 2s/5s。
        state.players[0].hand = Hand::from_tiles(&[
            Tile::from_raw(0),
            Tile::from_raw(4),
            Tile::from_raw(8),
            Tile::from_raw(12),
            Tile::from_raw(16),
            Tile::from_raw(20),
            Tile::from_raw(24),
            Tile::from_raw(28),
            Tile::from_raw(32),
            Tile::from_raw(44),
            Tile::from_raw(45),
            Tile::from_raw(80),
            Tile::from_raw(84),
        ]);
        state.phase = GamePhase::ActionPhase {
            player: PlayerId(0),
            drawn_tile: Some(Tile::from_raw(79)),
        };

        state
            .execute_action(TurnAction::RiichiDiscard(Tile::from_raw(79)))
            .unwrap();
        // 宣言时捕获双立直条件：无鸣牌且是庄家第一打。
        assert!(state.players[0].double_riichi);
        assert!(!state.players[0].is_riichi);

        // 下家碰走宣言牌：立直被受理。
        state.players[1].hand = Hand::from_tiles(&[
            Tile::from_raw(0),
            Tile::from_raw(4),
            Tile::from_raw(8),
            Tile::from_raw(12),
            Tile::from_raw(16),
            Tile::from_raw(20),
            Tile::from_raw(24),
            Tile::from_raw(28),
            Tile::from_raw(32),
            Tile::from_raw(76),
            Tile::from_raw(77),
            Tile::from_raw(78),
            Tile::from_raw(84),
        ]);
        state
            .execute_call(
                PlayerId(1),
                ResponseAction::Pon {
                    hand_tiles: [Tile::from_raw(76), Tile::from_raw(77)],
                },
            )
            .unwrap();
        assert!(state.players[0].is_riichi);
        assert_eq!(state.players[0].points, 24_000);
        assert_eq!(state.riichi_sticks, 1);

        // 之后自摸 5s 和牌：双立直仍然成立（鸣牌不影响已捕获的判定）。
        state.phase = GamePhase::ActionPhase {
            player: PlayerId(0),
            drawn_tile: Some(Tile::from_raw(88)), // 5s
        };
        let (_, yaku_names) = state
            .check_tsumo(PlayerId(0))
            .expect("双立直自摸应当能和牌");
        assert!(yaku_names.iter().any(|name| name.contains("DoubleRiichi")));
    }

    #[test]
    fn riichi_is_not_accepted_when_declaration_discard_is_ronned() {
        // 宣言牌被荣和：立直不成立，不扣立直棒（与 Mortal 一致）。
        let mut state = GameState::new();
        let mut rng = rand::rngs::StdRng::seed_from_u64(23);
        state.start_round(&mut rng);
        state.players[0].hand = Hand::from_tiles(&[
            Tile::from_raw(0),
            Tile::from_raw(4),
            Tile::from_raw(8),
            Tile::from_raw(12),
            Tile::from_raw(16),
            Tile::from_raw(20),
            Tile::from_raw(24),
            Tile::from_raw(28),
            Tile::from_raw(32),
            Tile::from_raw(44),
            Tile::from_raw(48),
            Tile::from_raw(52),
            Tile::from_raw(78),
        ]);
        state.phase = GamePhase::ActionPhase {
            player: PlayerId(0),
            drawn_tile: Some(Tile::from_raw(79)),
        };
        state
            .execute_action(TurnAction::RiichiDiscard(Tile::from_raw(79)))
            .unwrap();
        assert!(!state.players[0].is_riichi);

        // 下家 123m 456m 789m 234s 2s 单骑听 2s（一杯口无，一色三顺……
        // 实际役：一气通贯 123m 456m 789m），荣和宣言牌。
        state.players[1].hand = Hand::from_tiles(&[
            Tile::from_raw(0),
            Tile::from_raw(4),
            Tile::from_raw(8),
            Tile::from_raw(12),
            Tile::from_raw(16),
            Tile::from_raw(20),
            Tile::from_raw(24),
            Tile::from_raw(28),
            Tile::from_raw(32),
            Tile::from_raw(78),
            Tile::from_raw(80),
            Tile::from_raw(84),
            Tile::from_raw(88),
        ]);
        state
            .execute_call(PlayerId(1), ResponseAction::Ron)
            .unwrap();

        // 宣言者未被扣立直棒：支出仅为荣和点数（本局 4 翻 40 符 = 8000）。
        assert!(!state.players[0].is_riichi);
        assert_eq!(25_000 - state.players[0].points, 8_000);
        assert_eq!(state.riichi_sticks, 0);
        assert!(matches!(state.phase, GamePhase::RoundOver));
    }

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
            state.players[0].hand.add(tile).unwrap();
        }
        state.players[0]
            .melds
            .push(Meld::pon(vec![tile; 3], tile, PlayerId(1)));
        state.phase = GamePhase::ActionPhase {
            player: PlayerId(0),
            drawn_tile: Some(tile),
        };

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
                CallKind::Kakan,
                &mut events,
            )
            .unwrap();
        assert_eq!(state.dora.len(), initial_dora_count + 1);
    }

    #[test]
    fn completing_daisangen_records_the_discarder_as_pao_target() {
        let mut state = GameState::new();
        let make_pon = |tile_type: TileType, from_player: PlayerId| {
            let tile = tile_type.with_copy(0);
            Meld::pon(vec![tile; 3], tile, from_player)
        };
        state.players[0]
            .melds
            .push(make_pon(TileType::HATSU, PlayerId(1)));
        state.players[0]
            .melds
            .push(make_pon(TileType::CHUN, PlayerId(1)));

        let called = TileType::HAKU.with_copy(0);
        state.players[0]
            .melds
            .push(Meld::pon(vec![called; 3], called, PlayerId(2)));
        state.update_pao_after_open_call(PlayerId(0), PlayerId(2), called);

        assert_eq!(state.pao_targets[0], Some(2));
    }

    #[test]
    fn response_pass_by_discarder_advances_to_draw_phase() {
        let mut state = GameState::new();
        let mut rng = rand::rngs::StdRng::seed_from_u64(31);
        state.start_round(&mut rng);
        state.draw().unwrap();
        let drawn = state.drawn_tile().unwrap();

        state.execute_action(TurnAction::Discard(drawn)).unwrap();
        assert!(matches!(state.phase, GamePhase::ResponsePhase { .. }));
        state.complete_response_pass().unwrap();
        assert!(matches!(state.phase, GamePhase::DrawPhase { .. }));
    }

    #[test]
    fn riichi_declaration_index_marks_the_first_post_riichi_river_discard() {
        let mut state = GameState::new();
        let mut rng = rand::rngs::StdRng::seed_from_u64(23);
        state.start_round(&mut rng);
        // 摸东(104)后打东能听牌，作为立直宣言牌。
        state.players[0].hand = Hand::from_tiles(&[
            Tile::from_raw(0),
            Tile::from_raw(4),
            Tile::from_raw(8),
            Tile::from_raw(12),
            Tile::from_raw(16),
            Tile::from_raw(20),
            Tile::from_raw(24),
            Tile::from_raw(28),
            Tile::from_raw(32),
            Tile::from_raw(36),
            Tile::from_raw(37),
            Tile::from_raw(40),
            Tile::from_raw(44),
        ]);
        state.phase = GamePhase::ActionPhase {
            player: PlayerId(0),
            drawn_tile: Some(Tile::from_raw(104)),
        };

        state
            .execute_action(TurnAction::RiichiDiscard(Tile::from_raw(104)))
            .unwrap();
        // 宣告本身不扣分：立直棒在宣言牌通过响应窗口后扣除。
        assert!(!state.players[0].is_riichi);
        assert!(state.players[0].riichi_declared);
        assert_eq!(state.players[0].points, 25_000);
        assert_eq!(state.riichi_sticks, 0);
        assert_eq!(state.players[0].riichi_declaration_index, None);

        // 宣言牌未被鸣走，通过响应窗口后受理：扣 1000 点、放置立直棒，
        // 宣言牌进入牌河第 0 位。
        state.complete_response_pass().unwrap();
        assert!(state.players[0].is_riichi);
        assert_eq!(state.players[0].points, 24_000);
        assert_eq!(state.riichi_sticks, 1);
        assert_eq!(state.players[0].riichi_declaration_index, Some(0));
        assert_eq!(state.players[0].discards[0], Tile::from_raw(104));

        // 立直后的第二张入河牌（被鸣后打出的摸牌）不会覆盖宣言下标。
        state.players[0].riichi_declaration_index = Some(0);
        state.players[0].discards.push(Tile::from_raw(0));
        assert_eq!(state.players[0].riichi_declaration_index, Some(0));
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
            player: PlayerId(0),
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
            player: PlayerId(0),
        };

        let options = state.get_call_options();
        assert!(!options.iter().any(|option| {
            option.player == PlayerId(1) && matches!(option.call_type, CallType::Ron)
        }));
    }

    #[test]
    fn tsumo_records_win_details_with_seat_prefix() {
        let mut state = GameState::new();
        let mut rng = rand::rngs::StdRng::seed_from_u64(77);
        state.start_round(&mut rng);
        // 123456789万 + 东东东 + 中，摸中自摸：混一色 + 役牌，必和。
        state.players[0].hand = Hand::from_tiles(&[
            Tile::from_raw(0),
            Tile::from_raw(4),
            Tile::from_raw(8),
            Tile::from_raw(12),
            Tile::from_raw(16),
            Tile::from_raw(20),
            Tile::from_raw(24),
            Tile::from_raw(28),
            Tile::from_raw(32),
            Tile::from_raw(108),
            Tile::from_raw(109),
            Tile::from_raw(110),
            Tile::from_raw(132),
        ]);
        state.phase = GamePhase::ActionPhase {
            player: PlayerId(0),
            drawn_tile: Some(Tile::from_raw(133)),
        };

        state.execute_action(TurnAction::Tsumo).unwrap();

        assert!(!state.round_win_details.is_empty(), "自摸应记录役种明细");
        assert!(
            state
                .round_win_details
                .iter()
                .all(|detail| detail.contains("家：")),
            "明细应带座位前缀: {:?}",
            state.round_win_details
        );
        assert!(state
            .round_win_details
            .iter()
            .any(|detail| detail.contains("翻")));
    }

    #[test]
    fn chankan_on_ankan_is_only_for_kokushi() {
        use riichi_core::hand::Hand;
        use riichi_core::meld::Meld;
        use riichi_core::tile::Tile;

        let mut state = GameState::new();
        // 0 家暗杠 1m，进入抢杠窗口
        state.players[0].melds.push(Meld::ankan(vec![
            Tile::from_raw(0),
            Tile::from_raw(1),
            Tile::from_raw(2),
            Tile::from_raw(3),
        ]));
        state.phase = GamePhase::ChankanResponse {
            player: PlayerId(0),
            kan_tile: Tile::from_raw(0),
            kind: CallKind::Ankan,
        };
        // 1 家国士十三面听牌（13 张单张幺九），暗杠牌 1m 正好补成雀头
        state.players[1].hand = Hand::from_tiles(&[
            Tile::from_raw(0),
            Tile::from_raw(32),
            Tile::from_raw(36),
            Tile::from_raw(68),
            Tile::from_raw(72),
            Tile::from_raw(104),
            Tile::from_raw(108),
            Tile::from_raw(112),
            Tile::from_raw(116),
            Tile::from_raw(120),
            Tile::from_raw(124),
            Tile::from_raw(128),
            Tile::from_raw(132),
        ]);
        // 2 家普通听牌：23m 456p 789p 55s 中中中 听 1m/4m，且有役（中刻）
        state.players[2].hand = Hand::from_tiles(&[
            Tile::from_raw(4),
            Tile::from_raw(8),
            Tile::from_raw(36),
            Tile::from_raw(40),
            Tile::from_raw(44),
            Tile::from_raw(48),
            Tile::from_raw(52),
            Tile::from_raw(56),
            Tile::from_raw(88),
            Tile::from_raw(89),
            Tile::from_raw(132),
            Tile::from_raw(133),
            Tile::from_raw(134),
        ]);

        let options = state.get_call_options();
        assert!(options
            .iter()
            .any(|o| o.player == PlayerId(1) && matches!(o.call_type, CallType::Ron)));
        assert!(!options
            .iter()
            .any(|o| o.player == PlayerId(2) && matches!(o.call_type, CallType::Ron)));
    }

    #[test]
    fn daiminkan_has_no_chankan_window_and_delays_dora_to_discard() {
        use rand::rngs::StdRng;
        use rand::SeedableRng;
        use riichi_core::hand::Hand;
        use riichi_core::wall::Wall;

        let mut state = GameState::new();
        let mut rng = StdRng::seed_from_u64(7);
        state.wall = Wall::new(&mut rng);
        // 1 家手中有 3 张 1m
        state.players[1].hand = Hand::from_tiles(&[
            Tile::from_raw(0),
            Tile::from_raw(1),
            Tile::from_raw(2),
            Tile::from_raw(36),
            Tile::from_raw(40),
            Tile::from_raw(44),
            Tile::from_raw(48),
            Tile::from_raw(52),
            Tile::from_raw(56),
            Tile::from_raw(88),
            Tile::from_raw(89),
            Tile::from_raw(92),
            Tile::from_raw(96),
        ]);
        state.phase = GamePhase::ResponsePhase {
            player: PlayerId(0),
            discarded_tile: Tile::from_raw(3),
        };

        let initial_dora = state.dora.len();
        let options = state.get_call_options();
        let minkan = options
            .iter()
            .find(|o| o.player == PlayerId(1) && matches!(o.call_type, CallType::Minkan { .. }))
            .expect("1 家应能明杠 1m");
        let CallType::Minkan { hand_tiles } = minkan.call_type else {
            unreachable!()
        };
        state
            .execute_call(PlayerId(1), ResponseAction::Minkan { hand_tiles })
            .unwrap();

        // 明杠立即成立：无抢杠窗口，直接进入行动阶段并补摸岭上；
        // 宝牌按 Mortal 延迟到杠家下一次舍牌时翻开。
        assert!(matches!(state.phase, GamePhase::ActionPhase { .. }));
        assert!(state.get_call_options().is_empty());
        assert_eq!(state.dora.len(), initial_dora, "明杠的宝牌不应立即翻开");

        // 杠家摸切岭上牌后宝牌才翻开
        let drawn = state.drawn_tile().expect("岭上补摸后应有自摸牌");
        state.execute_action(TurnAction::Discard(drawn)).unwrap();
        assert_eq!(state.dora.len(), initial_dora + 1);
    }

    #[test]
    fn suucha_riichi_aborts_after_the_fourth_declaration_discard_passes() {
        use rand::rngs::StdRng;
        use rand::SeedableRng;
        use riichi_core::wall::Wall;

        let mut state = GameState::new();
        let mut rng = StdRng::seed_from_u64(11);
        state.wall = Wall::new(&mut rng);
        for pid in 0..4 {
            state.events.push(GameEvent::Riichi {
                player: PlayerId(pid),
            });
        }
        state.phase = GamePhase::ResponsePhase {
            player: PlayerId(3),
            discarded_tile: Tile::from_raw(4),
        };

        state.complete_response_pass().unwrap();
        assert_eq!(state.round_end_reason, Some(RoundEndReason::SuuchaRiichi));
    }

    #[test]
    fn suucha_riichi_does_not_abort_at_the_fourth_declaration() {
        use rand::rngs::StdRng;
        use rand::SeedableRng;
        use riichi_core::hand::Hand;
        use riichi_core::wall::Wall;

        let mut state = GameState::new();
        let mut rng = StdRng::seed_from_u64(23);
        state.wall = Wall::new(&mut rng);
        for pid in 0..3 {
            state.events.push(GameEvent::Riichi {
                player: PlayerId(pid),
            });
        }
        // 第四家摸东后打东能听牌（123456789m 1123p）
        state.players[3].hand = Hand::from_tiles(&[
            Tile::from_raw(0),
            Tile::from_raw(4),
            Tile::from_raw(8),
            Tile::from_raw(12),
            Tile::from_raw(16),
            Tile::from_raw(20),
            Tile::from_raw(24),
            Tile::from_raw(28),
            Tile::from_raw(32),
            Tile::from_raw(36),
            Tile::from_raw(37),
            Tile::from_raw(40),
            Tile::from_raw(44),
        ]);
        state.phase = GamePhase::ActionPhase {
            player: PlayerId(3),
            drawn_tile: Some(Tile::from_raw(104)),
        };

        state
            .execute_action(TurnAction::RiichiDiscard(Tile::from_raw(104)))
            .unwrap();
        // 宣言后仍处于响应阶段：宣言牌可被荣和/鸣牌，立直尚未被受理
        assert!(matches!(state.phase, GamePhase::ResponsePhase { .. }));
        assert_eq!(state.round_end_reason, None);
    }

    #[test]
    fn suufon_renda_triggers_on_a_plain_fourth_wind_discard() {
        use riichi_core::hand::Hand;

        let mut state = GameState::new();
        // 前三家首巡各打东
        for pid in 0..3 {
            state.events.push(GameEvent::Discard {
                player: PlayerId(pid),
                tile: Tile::from_raw(108),
                kind: riichi_core::game::DiscardKind::Tedashi,
            });
        }
        // 第四家普通弃牌（非立直宣言）打出东
        state.players[3].hand = Hand::from_tiles(&[
            Tile::from_raw(0),
            Tile::from_raw(4),
            Tile::from_raw(8),
            Tile::from_raw(12),
            Tile::from_raw(16),
            Tile::from_raw(20),
            Tile::from_raw(24),
            Tile::from_raw(28),
            Tile::from_raw(32),
            Tile::from_raw(36),
            Tile::from_raw(40),
            Tile::from_raw(44),
            Tile::from_raw(48),
        ]);
        state.phase = GamePhase::ActionPhase {
            player: PlayerId(3),
            drawn_tile: Some(Tile::from_raw(108)),
        };

        state
            .execute_action(TurnAction::Discard(Tile::from_raw(108)))
            .unwrap();
        assert_eq!(state.round_end_reason, Some(RoundEndReason::SuufonRenda));
    }

    #[test]
    fn four_kan_aborts_after_the_following_discard_passes() {
        use rand::rngs::StdRng;
        use rand::SeedableRng;
        use riichi_core::meld::Meld;
        use riichi_core::wall::Wall;

        let mut state = GameState::new();
        let mut rng = StdRng::seed_from_u64(13);
        state.wall = Wall::new(&mut rng);
        // 四个杠分布在两家
        let ankan = |t: u8| {
            Meld::ankan(vec![
                Tile::from_raw(t * 4),
                Tile::from_raw(t * 4 + 1),
                Tile::from_raw(t * 4 + 2),
                Tile::from_raw(t * 4 + 3),
            ])
        };
        state.players[0].melds.push(ankan(0));
        state.players[0].melds.push(ankan(1));
        state.players[1].melds.push(ankan(2));
        state.players[1].melds.push(ankan(3));
        state.four_kan_abort_pending = true;
        state.phase = GamePhase::ResponsePhase {
            player: PlayerId(0),
            discarded_tile: Tile::from_raw(8),
        };

        state.complete_response_pass().unwrap();
        assert_eq!(state.round_end_reason, Some(RoundEndReason::SuuKantsu));
    }

    #[test]
    fn four_kan_abort_yields_to_ron() {
        use rand::rngs::StdRng;
        use rand::SeedableRng;
        use riichi_core::hand::Hand;
        use riichi_core::meld::Meld;
        use riichi_core::wall::Wall;

        let mut state = GameState::new();
        let mut rng = StdRng::seed_from_u64(17);
        state.wall = Wall::new(&mut rng);
        let ankan = |t: u8| {
            Meld::ankan(vec![
                Tile::from_raw(t * 4),
                Tile::from_raw(t * 4 + 1),
                Tile::from_raw(t * 4 + 2),
                Tile::from_raw(t * 4 + 3),
            ])
        };
        state.players[0].melds.push(ankan(0));
        state.players[0].melds.push(ankan(1));
        state.players[1].melds.push(ankan(2));
        state.players[1].melds.push(ankan(3));
        state.four_kan_abort_pending = true;
        // 2 家白板碰 + 123456789m 11p，荣和 1p（白板役牌）
        let white = Tile::from_raw(124);
        state.players[2].melds.push(Meld::pon(
            vec![white, Tile::from_raw(125), Tile::from_raw(126)],
            white,
            PlayerId(0),
        ));
        state.players[2].hand = Hand::from_tiles(&[
            Tile::from_raw(0),
            Tile::from_raw(4),
            Tile::from_raw(8),
            Tile::from_raw(12),
            Tile::from_raw(16),
            Tile::from_raw(20),
            Tile::from_raw(24),
            Tile::from_raw(28),
            Tile::from_raw(32),
            Tile::from_raw(36),
        ]);
        state.phase = GamePhase::ResponsePhase {
            player: PlayerId(0),
            discarded_tile: Tile::from_raw(37),
        };

        state
            .execute_call(PlayerId(2), ResponseAction::Ron)
            .unwrap();
        assert!(matches!(
            state.round_end_reason,
            Some(RoundEndReason::Win { .. })
        ));
    }
}
