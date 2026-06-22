use rand::rngs::StdRng;
use rand::SeedableRng;
use riichi_core::game_types::{CallType, ResponseAction, TurnAction};
use riichi_core::player::PlayerId;
use riichi_engine::game::{GamePhase, GameState};
use riichi_logic::shanten::ShantenCalculator;
use tokio::sync::mpsc;

use crate::channel::{ActionMsg, CallResponseMsg, PlayerAction, ServerEvent, TurnActionMsg};

pub struct GameLoop {
    pub game: GameState,
    pub calc: ShantenCalculator,
    pub rng: StdRng,
    pub event_txs: [mpsc::Sender<ServerEvent>; 4],
    pub action_rx: mpsc::Receiver<ActionMsg>,
}

impl GameLoop {
    pub fn new(
        event_txs: [mpsc::Sender<ServerEvent>; 4],
        action_rx: mpsc::Receiver<ActionMsg>,
    ) -> Self {
        Self {
            game: GameState::new(),
            calc: ShantenCalculator::new(),
            rng: StdRng::from_entropy(),
            event_txs,
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

                    self.send_to(
                        player,
                        ServerEvent::ActionRequired {
                            can_tsumo,
                            can_riichi,
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

    async fn apply_turn_action(&mut self, player: PlayerId, action: TurnActionMsg) {
        match action {
            TurnActionMsg::Discard(tile) => {
                let _ = self.game.execute_action(TurnAction::Discard(tile));
                self.broadcast_state().await;
                self.handle_after_turn().await;
            }
            TurnActionMsg::Tsumo => {
                let _ = self.game.execute_action(TurnAction::Tsumo);
                self.broadcast_state().await;
                self.handle_round_end().await;
            }
            TurnActionMsg::Riichi => {
                let options = self.game.get_tenpai_discard_options(player);
                if let Some(tile) = options.first() {
                    let _ = self.game.execute_action(TurnAction::RiichiDiscard(*tile));
                    self.broadcast_state().await;
                    self.handle_after_turn().await;
                }
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

        let mut ron_player: Option<PlayerId> = None;

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

            let has_ron = player_options
                .iter()
                .any(|o| matches!(o.call_type, CallType::Ron));

            self.send_to(
                pid,
                ServerEvent::CallRequired {
                    options: player_options,
                },
            )
            .await;

            let response = self.wait_for_call_response(pid).await;

            match response {
                Some(CallResponseMsg::Ron) if has_ron => {
                    ron_player = Some(pid);
                    break;
                }
                _ => {
                    let _ = self.game.execute_call(pid, ResponseAction::Pass);
                }
            }
        }

        if let Some(pid) = ron_player {
            let _ = self.game.execute_call(pid, ResponseAction::Ron);
            self.broadcast_state().await;
            self.handle_round_end().await;
            return;
        }

        let _ = self.game.execute_call(discarder, ResponseAction::Pass);
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
            })
            .await;
        } else {
            self.game.start_round(&mut self.rng);
            self.broadcast_state().await;
        }
    }

    async fn wait_for_turn_action(&mut self, expected: PlayerId) -> Option<TurnActionMsg> {
        while let Some((pid, action)) = self.action_rx.recv().await {
            if pid != expected {
                continue;
            }
            match action {
                PlayerAction::TurnAction(ta) => return Some(ta),
                _ => continue,
            }
        }
        None
    }

    async fn wait_for_call_response(&mut self, expected: PlayerId) -> Option<CallResponseMsg> {
        while let Some((pid, action)) = self.action_rx.recv().await {
            if pid != expected {
                continue;
            }
            match action {
                PlayerAction::CallResponse(cr) => return Some(cr),
                _ => continue,
            }
        }
        None
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
