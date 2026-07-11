use rand::rngs::StdRng;
use rand::SeedableRng;
use riichi_core::game::{CallType, ResponseAction, TurnAction};
use riichi_core::player::PlayerId;
use riichi_core::tile::Tile;
use riichi_engine::game::{GamePhase, GameState};
use riichi_logic::shanten::ShantenCalculator;
use std::collections::HashSet;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration, Instant};

use crate::channel::{ActionMsg, CallResponseMsg, PlayerAction, ServerEvent, TurnActionMsg};

pub struct GameLoop {
    pub game: GameState,
    pub calc: ShantenCalculator,
    pub rng: StdRng,
    pub event_txs: [mpsc::Sender<ServerEvent>; 4],
    pub action_tx: mpsc::Sender<ActionMsg>,
    pub action_rx: mpsc::Receiver<ActionMsg>,
}

impl GameLoop {
    pub fn new(
        event_txs: [mpsc::Sender<ServerEvent>; 4],
        action_tx: mpsc::Sender<ActionMsg>,
        action_rx: mpsc::Receiver<ActionMsg>,
    ) -> Self {
        Self {
            game: GameState::new(),
            calc: ShantenCalculator::new(),
            rng: StdRng::from_entropy(),
            event_txs,
            action_tx,
            action_rx,
        }
    }

    pub async fn run(&mut self) {
        self.game.start_round(&mut self.rng);
        self.broadcast_state().await;

        loop {
            if self.game.is_game_over() {
                self.broadcast(ServerEvent::GameOver {
                    scores: self.scores(),
                    ranking: self.game.final_ranking(),
                })
                .await;
                return;
            }

            match self.game.phase {
                GamePhase::DrawPhase => {
                    if self.game.draw().is_err() {
                        self.handle_round_end().await;
                        continue;
                    }
                    self.broadcast_state().await;
                }
                GamePhase::ActionPhase => {
                    let player = self.game.current_player;
                    let can_tsumo = self.game.check_tsumo(player).is_some();
                    let can_riichi = self.game.can_declare_riichi(player);
                    let riichi_options = self.riichi_options(player);
                    let discard_options = self.game.players[player.0].hand.tiles().to_vec();
                    let ankan_options = self.game.get_ankan_options(player);
                    let kakan_options = self.game.get_kakan_options(player);
                    let can_kyuushu = self.game.can_declare_kyuushu(player);

                    self.send_to(
                        player,
                        ServerEvent::ActionRequired {
                            can_tsumo,
                            can_riichi,
                            riichi_options,
                            discard_options,
                            ankan_options,
                            kakan_options,
                            can_kyuushu,
                        },
                    )
                    .await;

                    match self.wait_for_turn_action(player).await {
                        Some(action) => {
                            self.apply_turn_action(player, action).await;
                        }
                        None => return,
                    }
                }
                GamePhase::ResponsePhase { .. } | GamePhase::ChankanResponse { .. } => {
                    self.handle_response_phase().await;
                }
                GamePhase::RoundOver => {
                    self.handle_round_end().await;
                }
            }
        }
    }

    /// 替换指定座位的事件连接，并立即发送该玩家视角的完整状态快照。
    ///
    /// 玩家身份由座位保持，重连客户端不会获得其他玩家的手牌；状态广播
    /// 仍然按照现有的逐玩家视角逻辑生成。
    pub async fn reconnect_player(
        &mut self,
        player: PlayerId,
        event_tx: mpsc::Sender<ServerEvent>,
        mut action_rx: mpsc::Receiver<ActionMsg>,
    ) {
        self.event_txs[player.0] = event_tx;
        let action_tx = self.action_tx.clone();
        tokio::spawn(async move {
            while let Some(message) = action_rx.recv().await {
                if message.0 == player {
                    if action_tx.send(message).await.is_err() {
                        break;
                    }
                }
            }
        });
        self.broadcast_state().await;
        self.send_current_action_prompt(player).await;
    }

    /// 向重连玩家重新发送当前阶段的操作提示。
    async fn send_current_action_prompt(&self, player: PlayerId) {
        match self.game.phase {
            GamePhase::ActionPhase if self.game.current_player == player => {
                let _ = self.event_txs[player.0]
                    .send(ServerEvent::ActionRequired {
                        can_tsumo: self.game.check_tsumo(player).is_some(),
                        can_riichi: self.game.can_declare_riichi(player),
                        riichi_options: self.riichi_options(player),
                        discard_options: self.game.players[player.0].hand.tiles().to_vec(),
                        ankan_options: self.game.get_ankan_options(player),
                        kakan_options: self.game.get_kakan_options(player),
                        can_kyuushu: self.game.can_declare_kyuushu(player),
                    })
                    .await;
            }
            GamePhase::ResponsePhase { .. } | GamePhase::ChankanResponse { .. } => {
                let options: Vec<_> = self
                    .game
                    .get_call_options()
                    .into_iter()
                    .filter(|option| option.player == player)
                    .collect();
                if !options.is_empty() {
                    let _ = self.event_txs[player.0]
                        .send(ServerEvent::CallRequired { options })
                        .await;
                }
            }
            _ => {}
        }
    }

    async fn apply_turn_action(&mut self, player: PlayerId, action: TurnActionMsg) {
        match action {
            TurnActionMsg::Discard(tile) => {
                if let Err(error) = self.game.execute_action(TurnAction::Discard(tile)) {
                    self.send_to(player, ServerEvent::Error(error.to_string()))
                        .await;
                    return;
                }
                self.broadcast_state().await;
                self.handle_after_turn().await;
            }
            TurnActionMsg::Tsumo => {
                if let Err(error) = self.game.execute_action(TurnAction::Tsumo) {
                    self.send_to(player, ServerEvent::Error(error.to_string()))
                        .await;
                    return;
                }
                self.broadcast_state().await;
                self.handle_round_end().await;
            }
            TurnActionMsg::RiichiDiscard(tile) => {
                if let Err(error) = self.game.execute_action(TurnAction::RiichiDiscard(tile)) {
                    self.send_to(player, ServerEvent::Error(error.to_string()))
                        .await;
                    return;
                }
                self.broadcast_state().await;
                self.handle_after_turn().await;
            }
            TurnActionMsg::Riichi => {
                if let Some(tile) = self.riichi_options(player).first().copied() {
                    if let Err(error) = self.game.execute_action(TurnAction::RiichiDiscard(tile)) {
                        self.send_to(player, ServerEvent::Error(error.to_string()))
                            .await;
                        return;
                    }
                    self.broadcast_state().await;
                    self.handle_after_turn().await;
                } else {
                    self.send_to(player, ServerEvent::Error("当前没有合法的立直弃牌".into()))
                        .await;
                }
            }
            TurnActionMsg::Ankan(tile) => {
                if let Err(error) = self.game.execute_action(TurnAction::Ankan(tile)) {
                    self.send_to(player, ServerEvent::Error(error.to_string()))
                        .await;
                    return;
                }
                self.broadcast_state().await;
            }
            TurnActionMsg::Kakan(index, tile) => {
                if let Err(error) = self.game.execute_action(TurnAction::Kakan(index, tile)) {
                    self.send_to(player, ServerEvent::Error(error.to_string()))
                        .await;
                    return;
                }
                self.broadcast_state().await;
                self.handle_after_turn().await;
            }
            TurnActionMsg::KyuushuKyuuhai => {
                if let Err(error) = self.game.execute_action(TurnAction::KyuushuKyuuhai) {
                    self.send_to(player, ServerEvent::Error(error.to_string()))
                        .await;
                    return;
                }
                self.broadcast_state().await;
                self.handle_round_end().await;
            }
        }
    }

    async fn handle_after_turn(&mut self) {
        loop {
            let phase = self.game.phase.clone();
            match phase {
                GamePhase::ResponsePhase { discarder, .. }
                | GamePhase::ChankanResponse {
                    kakan_player: discarder,
                    ..
                } => {
                    self.handle_response_phase_with_discarder(discarder).await;
                    if matches!(self.game.phase, GamePhase::DrawPhase) {
                        if self.game.draw().is_err() {
                            self.handle_round_end().await;
                        }
                        self.broadcast_state().await;
                        return;
                    }
                    if matches!(self.game.phase, GamePhase::RoundOver) {
                        self.handle_round_end().await;
                        return;
                    }
                }
                _ => return,
            }
        }
    }

    async fn handle_response_phase(&mut self) {
        let phase = self.game.phase.clone();
        let discarder = match phase {
            GamePhase::ResponsePhase { discarder, .. } => discarder,
            GamePhase::ChankanResponse { kakan_player, .. } => kakan_player,
            _ => return,
        };
        self.handle_response_phase_with_discarder(discarder).await;
    }

    async fn handle_response_phase_with_discarder(&mut self, discarder: PlayerId) {
        let call_options = self.game.get_call_options();

        let mut accepted_call: Option<(PlayerId, ResponseAction)> = None;
        let mut ron_players = Vec::new();

        let mut eligible = Vec::new();
        for idx in 0..4u8 {
            let pid = PlayerId(idx as usize);
            if pid == discarder {
                continue;
            }

            let player_options: Vec<_> = call_options
                .iter()
                .filter(|o| o.player == pid)
                .cloned()
                .collect();

            if player_options.is_empty() {
                continue;
            }

            eligible.push(pid);
            self.send_to(
                pid,
                ServerEvent::CallRequired {
                    options: player_options,
                },
            )
            .await;
        }

        let responses = self.wait_for_call_responses(&eligible).await;
        for (pid, response) in responses {
            let player_options: Vec<_> = call_options
                .iter()
                .filter(|o| o.player == pid)
                .cloned()
                .collect();

            let has_ron = player_options
                .iter()
                .any(|o| matches!(o.call_type, CallType::Ron));

            match response {
                CallResponseMsg::Ron if has_ron => {
                    ron_players.push(pid);
                }
                CallResponseMsg::Pon { hand_tiles } => {
                    let candidate = ResponseAction::Pon { hand_tiles };
                    if should_replace_call(accepted_call.as_ref(), pid, &candidate, discarder) {
                        accepted_call = Some((pid, candidate));
                    }
                }
                CallResponseMsg::Chi { hand_tiles } => {
                    let candidate = ResponseAction::Chi { hand_tiles };
                    if should_replace_call(accepted_call.as_ref(), pid, &candidate, discarder) {
                        accepted_call = Some((pid, candidate));
                    }
                }
                CallResponseMsg::Minkan { hand_tiles } => {
                    let candidate = ResponseAction::Minkan { hand_tiles };
                    if should_replace_call(accepted_call.as_ref(), pid, &candidate, discarder) {
                        accepted_call = Some((pid, candidate));
                    }
                }
                _ => {
                    let _ = self.game.record_response_pass(pid);
                }
            }
        }

        if !ron_players.is_empty() {
            let mut ordered_winners = Vec::new();
            for offset in 1..4 {
                let candidate = PlayerId((discarder.0 + offset) % 4);
                if ron_players.contains(&candidate) {
                    ordered_winners.push(candidate);
                }
            }
            let max_winners = if self.game.rules.allow_triple_ron {
                3
            } else if self.game.rules.allow_double_ron {
                2
            } else {
                1
            };
            ordered_winners.truncate(max_winners);
            if ordered_winners.len() > 1 {
                if let Err(error) = self.game.execute_multiple_ron(&ordered_winners) {
                    for winner in &ordered_winners {
                        self.send_to(*winner, ServerEvent::Error(error.to_string()))
                            .await;
                    }
                    return;
                }
            } else if let Some(&pid) = ordered_winners.first() {
                if let Err(error) = self.game.execute_call(pid, ResponseAction::Ron) {
                    self.send_to(pid, ServerEvent::Error(error.to_string()))
                        .await;
                    return;
                }
            }
            self.broadcast_state().await;
            if matches!(self.game.phase, GamePhase::RoundOver) {
                self.handle_round_end().await;
            }
            return;
        }

        if let Some((pid, action)) = accepted_call {
            if let Err(error) = self.game.execute_call(pid, action) {
                self.send_to(pid, ServerEvent::Error(error.to_string()))
                    .await;
                return;
            }
            self.broadcast_state().await;
            if matches!(self.game.phase, GamePhase::RoundOver) {
                self.handle_round_end().await;
            }
            return;
        }

        if let Err(error) = self.game.complete_response_pass() {
            self.broadcast(ServerEvent::Error(error.to_string())).await;
            return;
        }
        self.broadcast_state().await;

        if matches!(self.game.phase, GamePhase::DrawPhase) {
            if self.game.draw().is_err() {
                self.handle_round_end().await;
            }
            self.broadcast_state().await;
        } else if matches!(self.game.phase, GamePhase::RoundOver) {
            self.handle_round_end().await;
        }
    }

    async fn handle_round_end(&mut self) {
        self.broadcast_state().await;

        if self.game.is_game_over() {
            self.broadcast(ServerEvent::GameOver {
                scores: self.scores(),
                ranking: self.game.final_ranking(),
            })
            .await;
        } else {
            self.game.start_round(&mut self.rng);
            self.broadcast_state().await;
        }
    }

    async fn wait_for_turn_action(&mut self, expected: PlayerId) -> Option<TurnActionMsg> {
        let deadline = Instant::now() + Duration::from_millis(self.game.rules.turn_timeout_ms);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let received = timeout(remaining, self.action_rx.recv()).await;
            match received {
                Ok(Some((pid, PlayerAction::TurnAction(action)))) if pid == expected => {
                    return Some(action);
                }
                Ok(Some(_)) => continue,
                Ok(None) | Err(_) => break,
            }
        }

        // 超时默认摸切；若状态异常没有摸牌，则选择当前手牌最后一张，
        // 后续仍由规则引擎进行最终合法性校验。
        self.game
            .drawn_tile
            .map(TurnActionMsg::Discard)
            .or_else(|| {
                self.game.players[expected.0]
                    .hand
                    .tiles()
                    .last()
                    .copied()
                    .map(TurnActionMsg::Discard)
            })
    }

    async fn wait_for_call_responses(
        &mut self,
        eligible: &[PlayerId],
    ) -> Vec<(PlayerId, CallResponseMsg)> {
        if eligible.is_empty() {
            return Vec::new();
        }

        let deadline = Instant::now() + Duration::from_millis(self.game.rules.response_timeout_ms);
        let eligible_set: HashSet<PlayerId> = eligible.iter().copied().collect();
        let mut received_players = HashSet::new();
        let mut responses = Vec::with_capacity(eligible.len());
        loop {
            if received_players.len() == eligible_set.len() {
                break;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            let received = timeout(remaining, self.action_rx.recv()).await;
            match received {
                Ok(Some((pid, PlayerAction::CallResponse(response))))
                    if eligible_set.contains(&pid) && received_players.insert(pid) =>
                {
                    responses.push((pid, response));
                }
                Ok(Some(_)) => continue,
                Ok(None) | Err(_) => break,
            }
        }

        for &player in eligible {
            if received_players.insert(player) {
                responses.push((player, CallResponseMsg::Pass));
            }
        }
        responses
    }

    fn riichi_options(&self, player: PlayerId) -> Vec<Tile> {
        self.game.get_riichi_discard_options(player)
    }

    async fn broadcast_state(&self) {
        let players = &self.game.players;
        let points = [
            players[0].points,
            players[1].points,
            players[2].points,
            players[3].points,
        ];
        let discards = [
            players[0].discards.clone(),
            players[1].discards.clone(),
            players[2].discards.clone(),
            players[3].discards.clone(),
        ];
        let melds_count = [
            players[0].melds.len(),
            players[1].melds.len(),
            players[2].melds.len(),
            players[3].melds.len(),
        ];

        #[allow(clippy::needless_range_loop)]
        for idx in 0..4 {
            let pid = PlayerId(idx);
            let my_hand = {
                let mut tiles = players[idx].hand.tiles().to_vec();
                if self.game.current_player == pid {
                    if let Some(drawn) = self.game.drawn_tile {
                        tiles.push(drawn);
                    }
                }
                tiles
            };
            let event = ServerEvent::StateUpdate {
                phase: self.game.phase.clone(),
                current_player: self.game.current_player,
                drawn_tile: if self.game.current_player == pid {
                    self.game.drawn_tile
                } else {
                    None
                },
                hand_tiles: my_hand,
                hand_count: players[idx].hand.len(),
                points,
                discards: discards.clone(),
                melds_count,
                dora: self.game.dora.clone(),
                remaining_tiles: self.game.remaining_tiles(),
                round: self.game.round,
                honba: self.game.honba,
                riichi_sticks: self.game.riichi_sticks,
            };
            let _ = self.event_txs[idx].send(event).await;
        }
    }

    async fn broadcast(&self, event: ServerEvent) {
        for tx in &self.event_txs {
            let _ = tx.send(event.clone()).await;
        }
    }

    async fn send_to(&self, player: PlayerId, event: ServerEvent) {
        let _ = self.event_txs[player.0].send(event).await;
    }

    fn scores(&self) -> [i32; 4] {
        [
            self.game.players[0].points,
            self.game.players[1].points,
            self.game.players[2].points,
            self.game.players[3].points,
        ]
    }
}

fn should_replace_call(
    current: Option<&(PlayerId, ResponseAction)>,
    candidate_player: PlayerId,
    candidate: &ResponseAction,
    discarder: PlayerId,
) -> bool {
    let candidate_key = call_priority_key(candidate_player, candidate, discarder);
    current
        .map(|(player, action)| candidate_key > call_priority_key(*player, action, discarder))
        .unwrap_or(true)
}

fn call_priority_key(player: PlayerId, action: &ResponseAction, discarder: PlayerId) -> (u8, u8) {
    let priority = match action {
        ResponseAction::Pon { .. } | ResponseAction::Minkan { .. } => 2,
        ResponseAction::Chi { .. } => 1,
        ResponseAction::Pass | ResponseAction::Ron => 0,
    };
    let distance = ((player.0 + 4 - discarder.0) % 4) as u8;
    (priority, 4 - distance)
}

#[cfg(test)]
mod tests {
    use super::{call_priority_key, should_replace_call};
    use riichi_core::game::ResponseAction;
    use riichi_core::player::PlayerId;
    use riichi_core::tile::Tile;

    #[test]
    fn pon_beats_chi_and_nearer_call_wins_same_priority() {
        let chi = ResponseAction::Chi {
            hand_tiles: [Tile::from_raw(0), Tile::from_raw(4)],
        };
        let pon = ResponseAction::Pon {
            hand_tiles: [Tile::from_raw(8), Tile::from_raw(12)],
        };
        let discarder = PlayerId(0);
        assert!(should_replace_call(
            Some(&(PlayerId(1), chi.clone())),
            PlayerId(2),
            &pon,
            discarder,
        ));

        let farther_pon = ResponseAction::Pon {
            hand_tiles: [Tile::from_raw(16), Tile::from_raw(20)],
        };
        assert!(!should_replace_call(
            Some(&(PlayerId(1), pon)),
            PlayerId(2),
            &farther_pon,
            discarder,
        ));
        assert_eq!(call_priority_key(PlayerId(1), &chi, discarder).0, 1);
    }
}
