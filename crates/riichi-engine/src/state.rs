use std::collections::HashSet;

use crate::game::{TenpaiInfo, WaitInfo};
use riichi_core::game::{CallKind, EventEnvelope, GameEvent, ResponseAction, TurnAction, WinKind};
use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use riichi_logic::model::TileCounts;
use riichi_logic::shape::analyze_wait_tiles_with_open_melds;
use riichi_logic::visibility::remaining_copies_for;

use crate::game::{GamePhase, GameState};

impl GameState {
    /// 构建指定玩家的听牌展示信息。规则判断全部委托给核心逻辑。
    pub fn tenpai_info(&self, player: PlayerId) -> Option<TenpaiInfo> {
        let p = &self.players[player.0];
        let waits = analyze_wait_tiles_with_open_melds(p.hand.tiles(), p.melds.len());
        if waits.is_empty() {
            return None;
        }

        let mut visible = self.build_visible_tiles(player);
        if self.current_player() == Some(player) {
            if let Some(drawn) = self.drawn_tile() {
                visible.all_discards.inc(drawn.tile_type());
            }
        }
        let hand_counts = TileCounts::from_tiles(p.hand.tiles());
        let is_furiten = p.furiten.is_furiten();
        let waits = waits
            .into_iter()
            .map(|wait| {
                let tile_type = wait.tile_type;
                WaitInfo {
                    tile_type,
                    remaining: remaining_copies_for(tile_type, &hand_counts, &visible),
                    is_no_yaku: !self.wait_has_yaku(player, tile_type),
                }
            })
            .collect();

        Some(TenpaiInfo { waits, is_furiten })
    }

    /// 推进到下一个玩家
    pub fn advance_turn(&mut self) {
        let next = self.current_player().unwrap_or(PlayerId(0)).next();
        self.phase = GamePhase::DrawPhase {
            player: next,
            position: riichi_core::game::DrawPosition::LiveWall,
        };
    }

    /// 判断指定的牌是否来自岭上（wall[132..=135]）
    pub fn is_rinshan_tile(&self, tile: Tile) -> bool {
        self.wall.is_rinshan_tile(tile)
    }

    /// 获取玩家的听牌类型集合（内部使用）
    pub(crate) fn get_waiting_tile_types(&self, player: PlayerId) -> HashSet<TileType> {
        let p = &self.players[player.0];
        analyze_wait_tiles_with_open_melds(p.hand.tiles(), p.melds.len())
            .iter()
            .map(|w| w.tile_type)
            .collect()
    }

    /// 更新单个玩家的舍牌振听状态
    ///
    /// 如果玩家听的牌在其舍牌中出现，则进入振听状态
    pub(crate) fn update_discard_furiten(&mut self, player: PlayerId) {
        let waiting = self.get_waiting_tile_types(player);
        let discarded = &self.players[player.0].all_discarded_types;
        self.players[player.0].furiten.discard = waiting.iter().any(|tt| discarded.contains(tt));
    }

    /// 更新所有玩家的舍牌振听状态
    pub(crate) fn update_all_discard_furiten(&mut self) {
        for idx in 0..4 {
            self.update_discard_furiten(PlayerId(idx));
        }
    }

    /// 判断当前局是否已结束
    pub fn is_round_over(&self) -> bool {
        matches!(self.phase, GamePhase::RoundOver) || self.remaining_tiles() == 0
    }

    /// 正常摸牌区剩余可摸牌数
    pub fn remaining_tiles(&self) -> usize {
        self.wall.remaining()
    }

    /// 取出事件列表（消耗性读取）
    pub fn take_events(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.events)
    }

    /// 记录一个同时属于当前局和整场历史的事件。
    pub(crate) fn record_event(&mut self, event: GameEvent) {
        let envelope = EventEnvelope {
            event_id: self.event_log.len() as u64 + 1,
            event: event.clone(),
        };
        self.events.push(event.clone());
        self.history.push(event);
        if !self.replaying {
            self.event_log.push(envelope);
        }
    }

    /// 记录一批同时属于当前局和整场历史的事件。
    pub(crate) fn record_events(&mut self, events: &[GameEvent]) {
        for event in events {
            self.record_event(event.clone());
        }
    }

    /// 获取整场追加式事件历史。
    pub fn event_history(&self) -> &[GameEvent] {
        &self.history
    }

    /// Returns the authoritative sequenced event history.
    pub fn event_log(&self) -> &[EventEnvelope] {
        &self.event_log
    }

    /// Returns events after a Hanchan event id for reconnect catch-up.
    pub fn events_after(&self, event_id: u64) -> impl Iterator<Item = &EventEnvelope> {
        self.event_log
            .iter()
            .filter(move |envelope| envelope.event_id > event_id)
    }

    /// Apply one authoritative action to this state during replay.
    ///
    /// Domain execution is reused for validation and scoring; the replay flag
    /// prevents generated events from being appended a second time.
    pub fn apply_event(&mut self, envelope: &EventEnvelope) -> Result<(), String> {
        let backup = self.clone();
        self.replaying = true;
        if !matches!(envelope.event, GameEvent::Pass { .. }) {
            self.replay_passes.clear();
        }
        let result = match &envelope.event {
            GameEvent::Draw { player, tile } => {
                // 杠成立（明杠/加杠/暗杠通过抢杠窗口后）时引擎已内部补摸岭上并
                // 进入行动阶段；重放时对应的 Draw 事件已被内部消费，幂等跳过。
                let already_consumed = matches!(
                    &self.phase,
                    GamePhase::ActionPhase {
                        drawn_tile: Some(drawn),
                        ..
                    } if *drawn == *tile
                );
                let result = if already_consumed {
                    Ok(())
                } else {
                    match self.draw_position() {
                        Some(riichi_core::game::DrawPosition::Rinshan) => {
                            self.apply_rinshan_draw_event(*player, *tile)
                        }
                        _ => self.apply_draw_event(*player, *tile),
                    }
                    .map_err(|error| error.to_string())
                };
                if result.is_ok() && !already_consumed {
                    self.record_event(envelope.event.clone());
                }
                result
            }
            GameEvent::Discard { player, tile, kind } => {
                let result = self
                    .apply_discard_event(*player, *tile, *kind)
                    .map_err(|error| error.to_string());
                let completion = if result.is_ok()
                    && matches!(self.phase, GamePhase::ResponsePhase { .. })
                    && self.get_call_options().is_empty()
                {
                    self.record_event(envelope.event.clone());
                    self.complete_response_pass()
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                } else {
                    if result.is_ok() {
                        self.record_event(envelope.event.clone());
                    }
                    Ok(())
                };
                match (result, completion) {
                    (Ok(()), Ok(())) => Ok(()),
                    (Err(error), _) | (_, Err(error)) => Err(error),
                }
            }
            GameEvent::Riichi { player } => {
                let result = self
                    .apply_riichi_event(*player)
                    .map_err(|error| error.to_string());
                if result.is_ok() {
                    self.record_event(envelope.event.clone());
                }
                result
            }
            GameEvent::Pass { player } => {
                let eligible: std::collections::HashSet<_> = self
                    .get_call_options()
                    .into_iter()
                    .map(|option| option.player)
                    .collect();
                let result = self
                    .record_response_pass(*player)
                    .map_err(|error| error.to_string());
                let completion = if result.is_ok() {
                    self.replay_passes.insert(*player);
                    if !eligible.is_empty() && eligible.is_subset(&self.replay_passes) {
                        self.complete_response_pass()
                            .map(|_| ())
                            .map_err(|error| error.to_string())
                    } else {
                        Ok(())
                    }
                } else {
                    Ok(())
                };
                match (result, completion) {
                    (Ok(()), Ok(())) => Ok(()),
                    (Err(error), _) | (_, Err(error)) => Err(error),
                }
            }
            GameEvent::Call {
                player,
                kind,
                tiles,
                meld_index,
                ..
            } => {
                let result = match kind {
                    CallKind::Chi if tiles.len() == 2 => self
                        .execute_call(
                            *player,
                            ResponseAction::Chi {
                                hand_tiles: [tiles[0], tiles[1]],
                            },
                        )
                        .map(|_| ())
                        .map_err(|error| error.to_string()),
                    CallKind::Pon if tiles.len() == 2 => self
                        .execute_call(
                            *player,
                            ResponseAction::Pon {
                                hand_tiles: [tiles[0], tiles[1]],
                            },
                        )
                        .map(|_| ())
                        .map_err(|error| error.to_string()),
                    CallKind::Minkan if tiles.len() == 3 => self
                        .execute_call(
                            *player,
                            ResponseAction::Minkan {
                                hand_tiles: [tiles[0], tiles[1], tiles[2]],
                            },
                        )
                        .map(|_| ())
                        .map_err(|error| error.to_string()),
                    CallKind::Ankan if !tiles.is_empty() => self
                        .execute_ankan(*player, tiles[0])
                        .map(|_| ())
                        .map_err(|error| error.to_string()),
                    CallKind::Kakan if meld_index.is_some() && !tiles.is_empty() => self
                        .execute_kakan(*player, meld_index.unwrap(), tiles[0])
                        .map(|_| ())
                        .map_err(|error| error.to_string()),
                    _ => Err("重放鸣牌事件参数不完整".to_string()),
                };
                if result.is_ok() && matches!(kind, CallKind::Ankan | CallKind::Kakan) {
                    self.record_event(envelope.event.clone());
                    // 抢杠窗口无人可抢时（live 流程由 session 直接完成窗口），
                    // 重放时在此自动完成：翻宝牌并补摸岭上。
                    if matches!(self.phase, GamePhase::ChankanResponse { .. })
                        && self.get_call_options().is_empty()
                    {
                        self.complete_response_pass()
                            .map_err(|error| error.to_string())?;
                    }
                }
                result
            }
            GameEvent::Win {
                winners,
                kind: WinKind::Tsumo,
                ..
            } if winners.len() == 1 => self
                .execute_action(TurnAction::Tsumo)
                .map(|_| ())
                .map_err(|error| error.to_string()),
            GameEvent::Win {
                winners,
                kind: WinKind::Ron,
                ..
            } => {
                if winners.len() == 1 && matches!(self.phase, GamePhase::ChankanResponse { .. }) {
                    self.execute_call(winners[0], ResponseAction::Ron)
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                } else {
                    self.execute_multiple_ron(winners)
                        .map(|_| ())
                        .map_err(|error| error.to_string())
                }
            }
            GameEvent::AbortiveDraw { reason, .. } => {
                self.resolve_round_end(reason.clone());
                Ok(())
            }
            _ => Err("该事件不是可重放的动作事件".to_string()),
        };
        self.replaying = false;
        if let Err(error) = result {
            *self = backup;
            Err(error)
        } else {
            Ok(())
        }
    }

    /// 创建可交给重连客户端的权威状态快照。
    pub fn snapshot(&self) -> Self {
        self.clone()
    }

    /// 从权威快照恢复牌局状态。
    pub fn from_snapshot(snapshot: Self) -> Self {
        snapshot
    }

    /// 构建指定玩家视角的 VisibleTiles（用于向听/进张分析）
    ///
    /// 包含：
    /// - 当前玩家的副露牌
    /// - 其他玩家的副露牌
    /// - 所有玩家的舍牌
    /// - 宝牌指示牌
    pub fn build_visible_tiles(&self, player: PlayerId) -> riichi_logic::visibility::VisibleTiles {
        let mut visible = riichi_logic::visibility::VisibleTiles::new();

        // 当前玩家的副露牌
        for meld in &self.players[player.0].melds {
            for t in &meld.tiles {
                visible.hand_melds.inc(t.tile_type());
            }
        }

        // 其他玩家的副露牌
        for i in 0..4 {
            let pid = PlayerId(i);
            if pid == player {
                continue;
            }
            for meld in &self.players[i].melds {
                for t in &meld.tiles {
                    visible.all_melds.inc(t.tile_type());
                }
            }
        }

        // 所有玩家的舍牌
        for i in 0..4 {
            for &t in &self.players[i].discards {
                visible.all_discards.inc(t.tile_type());
            }
        }

        // 宝牌指示牌
        for &tt in &self.dora_indicators {
            visible.dora_indicators.inc(tt);
        }

        visible
    }
}

pub trait HanchanReplay {
    fn state_at(&self, event_id: u64) -> Result<GameState, String>;
}

impl HanchanReplay for riichi_core::game::Hanchan {
    /// Rebuild the materialized state at an authoritative event id.
    fn state_at(&self, event_id: u64) -> Result<GameState, String> {
        let setup = self
            .setup
            .rounds
            .iter()
            .rfind(|setup| setup.event_start_id <= event_id.saturating_add(1))
            .ok_or_else(|| "半庄缺少对应局的初始状态".to_string())?;
        let mut state =
            GameState::from_round_setup(setup, setup.round, setup.honba, setup.riichi_sticks);
        for envelope in self.events.iter().filter(|envelope| {
            envelope.event_id >= setup.event_start_id && envelope.event_id <= event_id
        }) {
            state.apply_event(envelope)?;
        }
        Ok(state)
    }
}

#[cfg(test)]
mod replay_tests {
    use super::GameState;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use riichi_core::game::RoundSetup;

    #[test]
    fn draw_and_discard_events_fold_back_to_the_same_state() {
        let mut original = GameState::new();
        let mut rng = StdRng::seed_from_u64(41);
        original.start_round(&mut rng);
        original.draw().unwrap();
        let drawn = original.drawn_tile().unwrap();
        original.discard(drawn).unwrap();
        let responders: Vec<_> = original
            .get_call_options()
            .into_iter()
            .map(|option| option.player)
            .collect();
        for player in responders {
            original.record_response_pass(player).unwrap();
        }
        original.complete_response_pass().unwrap();
        original.draw().unwrap();

        let setup = RoundSetup {
            round: 1,
            honba: 0,
            riichi_sticks: 0,
            event_start_id: 1,
            initial_points: original.round_start_points,
            wall: original.wall.tiles().to_vec(),
        };
        let events = original.event_log().to_vec();
        let mut replayed = GameState::from_round_setup(&setup, 1, 0, 0);
        for event in &events {
            replayed.apply_event(event).unwrap();
        }

        assert_eq!(replayed.event_log().len(), 0);
        assert_eq!(replayed.remaining_tiles(), original.remaining_tiles());
        assert_eq!(replayed.players[0].discards, original.players[0].discards);
        assert_eq!(
            replayed.players[0].hand.tiles(),
            original.players[0].hand.tiles()
        );
        assert_eq!(
            format!("{:?}", replayed.phase),
            format!("{:?}", original.phase)
        );
    }

    /// 明杠全流程重放：事件流 [Draw, Discard, Call(Minkan), Draw(岭上), Discard]。
    /// 明杠事件执行时引擎内部已补摸岭上，重放对应的 Draw 事件必须幂等跳过。
    #[test]
    fn minkan_round_replays_consistently() {
        use riichi_core::game::{CallType, ResponseAction, TurnAction};
        use riichi_core::tile::{Tile, TileType};
        use riichi_core::wall::Wall;

        // 手动构造牌山：0 家（庄家）第一摸 = 1m 第 4 张（raw 3），
        // 1 家配牌 = 3×1m（raw 0,1,2）+ 其他；保证 live 与重放配牌一致。
        // 1 家配牌位置：4-7, 20-23, 36-39, 49；0 家第一摸 = 52。
        let mut tiles: Vec<Tile> = Tile::all_tiles();
        let target_positions = [4usize, 5, 6, 52];
        let displaced: Vec<Tile> = target_positions.iter().map(|&p| tiles[p]).collect();
        for (i, &p) in target_positions.iter().enumerate() {
            tiles[p] = Tile::from_raw(i as u8); // 0,1,2 → 1 家配牌位；3 → 0 家第一摸位
        }
        for (i, &p) in [0usize, 1, 2, 3].iter().enumerate() {
            tiles[p] = displaced[i];
        }
        let setup = RoundSetup {
            round: 1,
            honba: 0,
            riichi_sticks: 0,
            event_start_id: 1,
            initial_points: [25000; 4],
            wall: tiles.clone(),
        };
        let mut original = GameState::from_round_setup(&setup, 1, 0, 0);
        // 0 家摸 1m 并打出
        original.draw().unwrap();
        let tile = original.drawn_tile().unwrap();
        assert_eq!(tile.tile_type(), TileType(0));
        original.execute_action(TurnAction::Discard(tile)).unwrap();
        // 1 家明杠
        let options = original.get_call_options();
        let minkan = options
            .iter()
            .find(|o| {
                o.player == riichi_core::player::PlayerId(1)
                    && matches!(o.call_type, CallType::Minkan { .. })
            })
            .expect("1 家应能明杠 1m");
        let CallType::Minkan { hand_tiles } = minkan.call_type else {
            unreachable!()
        };
        original
            .execute_call(
                riichi_core::player::PlayerId(1),
                ResponseAction::Minkan { hand_tiles },
            )
            .unwrap();
        // 杠家摸切岭上牌（宝牌此时翻开）
        let drawn = original.drawn_tile().expect("岭上补摸");
        original.execute_action(TurnAction::Discard(drawn)).unwrap();
        // 模拟 session：完成响应窗口（重放侧在无人响应时会自动完成）
        let responders: Vec<_> = original
            .get_call_options()
            .into_iter()
            .map(|option| option.player)
            .collect();
        for player in responders {
            original.record_response_pass(player).unwrap();
        }
        original.complete_response_pass().unwrap();

        let events = original.event_log().to_vec();
        assert!(events.iter().any(|e| matches!(
            e.event,
            riichi_core::game::GameEvent::Call {
                kind: riichi_core::game::CallKind::Minkan,
                ..
            }
        )));
        let mut replayed = GameState::from_round_setup(&setup, 1, 0, 0);
        for event in &events {
            replayed.apply_event(event).unwrap();
        }

        assert_eq!(replayed.event_log().len(), 0);
        assert_eq!(
            format!("{:?}", replayed.phase),
            format!("{:?}", original.phase)
        );
        assert_eq!(replayed.players[1].melds.len(), 1);
        assert_eq!(
            replayed.players[1].hand.tiles(),
            original.players[1].hand.tiles()
        );
        assert_eq!(replayed.remaining_tiles(), original.remaining_tiles());
        assert_eq!(replayed.dora.len(), original.dora.len());
        assert_eq!(
            replayed.dora_indicators, original.dora_indicators,
            "明杠宝牌应在舍牌时翻开，重放保持一致"
        );
    }

    /// 加杠全流程重放：0 家碰 86 → 摸到第 4 张 87 → 加杠 → 抢杠窗口无人可抢
    /// 自动完成 → 摸岭上 → 舍牌。事件流含 Draw(岭上)，重放必须幂等跳过。
    #[test]
    fn kakan_round_replays_consistently() {
        use riichi_core::game::{CallType, ResponseAction, TurnAction};
        use riichi_core::player::PlayerId;
        use riichi_core::tile::{Tile, TileType};

        // round 2 → 庄家 = 1 家，第一摸 = 位置 52。
        // 1 家摸 86 打出 → 0 家碰（配牌 84,85，位置 0,1）→ 0 家打牌 →
        // 1/2/3 家各摸打一张 → 0 家摸（位置 56）= 87 → 加杠。
        let mut tiles: Vec<Tile> = Tile::all_tiles();
        let target_positions = [0usize, 1, 52, 56]; // 0 家配牌 84,85；1 家摸 86；0 家摸 87
        let raws = [84u8, 85, 86, 87];
        let displaced: Vec<Tile> = target_positions.iter().map(|&p| tiles[p]).collect();
        for (i, &p) in target_positions.iter().enumerate() {
            tiles[p] = Tile::from_raw(raws[i]);
        }
        for (i, &p) in raws.iter().enumerate() {
            tiles[p as usize] = displaced[i];
        }
        let setup = RoundSetup {
            round: 2,
            honba: 0,
            riichi_sticks: 0,
            event_start_id: 1,
            initial_points: [25000; 4],
            wall: tiles.clone(),
        };
        let mut original = GameState::from_round_setup(&setup, 2, 0, 0);
        // 辅助：摸切并完成响应窗口
        fn tsumogiri_and_close(state: &mut GameState) {
            let t = state.drawn_tile().expect("行动阶段应有自摸牌");
            state.execute_action(TurnAction::Discard(t)).unwrap();
            let responders: Vec<_> = state
                .get_call_options()
                .into_iter()
                .map(|option| option.player)
                .collect();
            for player in responders {
                state.record_response_pass(player).unwrap();
            }
            state.complete_response_pass().unwrap();
        }
        // 1 家（庄家）摸 86 打出
        original.draw().unwrap();
        let first = original.drawn_tile().unwrap();
        assert_eq!(first.tile_type(), TileType(21));
        original.execute_action(TurnAction::Discard(first)).unwrap();
        // 0 家碰
        let options = original.get_call_options();
        let pon = options
            .iter()
            .find(|o| o.player == PlayerId(0) && matches!(o.call_type, CallType::Pon { .. }))
            .expect("0 家应能碰 4s");
        let CallType::Pon { hand_tiles } = pon.call_type else {
            unreachable!()
        };
        original
            .execute_call(PlayerId(0), ResponseAction::Pon { hand_tiles })
            .unwrap();
        // 0 家打出手牌中的一张
        let x = original.players[0].hand.tiles()[0];
        original.execute_action(TurnAction::Discard(x)).unwrap();
        let responders: Vec<_> = original
            .get_call_options()
            .into_iter()
            .map(|option| option.player)
            .collect();
        for player in responders {
            original.record_response_pass(player).unwrap();
        }
        original.complete_response_pass().unwrap();
        // 1、2、3 家各摸切一张
        for _ in 0..3 {
            original.draw().unwrap();
            tsumogiri_and_close(&mut original);
        }
        // 0 家摸（位置 56 = 87）并加杠
        original.draw().unwrap();
        let fourth = original.drawn_tile().unwrap();
        assert_eq!(fourth.tile_type(), TileType(21));
        original
            .execute_action(TurnAction::Kakan(0, fourth))
            .unwrap();
        // 抢杠窗口无人可抢，直接完成（live 流程与重放自动完成路径一致）
        original.complete_response_pass().unwrap();
        let drawn = original.drawn_tile().expect("岭上补摸");
        original.execute_action(TurnAction::Discard(drawn)).unwrap();
        // 模拟 session：完成响应窗口
        let responders: Vec<_> = original
            .get_call_options()
            .into_iter()
            .map(|option| option.player)
            .collect();
        for player in responders {
            original.record_response_pass(player).unwrap();
        }
        original.complete_response_pass().unwrap();

        let events = original.event_log().to_vec();
        let mut replayed = GameState::from_round_setup(&setup, 2, 0, 0);
        for event in &events {
            replayed.apply_event(event).unwrap();
        }

        assert_eq!(replayed.event_log().len(), 0);
        assert_eq!(
            format!("{:?}", replayed.phase),
            format!("{:?}", original.phase)
        );
        assert_eq!(replayed.players[0].melds.len(), 1);
        assert_eq!(
            replayed.players[0].melds[0].kind,
            riichi_core::meld::MeldKind::Kakan
        );
        assert_eq!(
            replayed.players[0].hand.tiles(),
            original.players[0].hand.tiles()
        );
        assert_eq!(replayed.remaining_tiles(), original.remaining_tiles());
        assert_eq!(replayed.dora.len(), original.dora.len());
        assert_eq!(
            replayed.dora_indicators, original.dora_indicators,
            "加杠宝牌应在窗口关闭后翻开，重放保持一致"
        );
    }
}
