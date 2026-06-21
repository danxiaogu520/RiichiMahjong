use riichi_core::game_types::{CallOption, CallType, GameEvent, ResponseAction, TurnAction};
use riichi_core::player::PlayerId;
use riichi_core::tile::Tile;
use riichi_engine::game::{GamePhase, GameState};
use riichi_logic::acceptance::analyze_discard;
use riichi_logic::analysis::analyze_wait_tiles;
use riichi_logic::shanten::ShantenCalculator;
use rand::rngs::StdRng;
use rand::SeedableRng;
use tokio::sync::mpsc;

use crate::channel::{PlayerAction, ServerEvent, TurnActionMsg, CallResponseMsg};

pub struct GameLoop {
    pub game: GameState,
    pub calc: ShantenCalculator,
    pub rng: StdRng,
    pub event_tx: mpsc::Sender<ServerEvent>,
    pub action_rx: mpsc::Receiver<PlayerAction>,
}

impl GameLoop {
    pub fn new(
        event_tx: mpsc::Sender<ServerEvent>,
        action_rx: mpsc::Receiver<PlayerAction>,
    ) -> Self {
        Self {
            game: GameState::new(),
            calc: ShantenCalculator::new(),
            rng: StdRng::from_entropy(),
            event_tx,
            action_rx,
        }
    }

    pub async fn run(&mut self) {
        self.game.start_round(&mut self.rng);
        self.send_state().await;

        loop {
            if self.game.is_game_over() {
                let _ = self.event_tx.send(ServerEvent::GameOver {
                    scores: self.scores(),
                }).await;
                return;
            }

            match self.game.phase {
                GamePhase::DrawPhase => {
                    if self.game.draw().is_err() {
                        self.handle_round_end().await;
                        continue;
                    }
                    self.send_state().await;
                }
                GamePhase::ActionPhase => {
                    if self.game.current_player == PlayerId(0) {
                        self.send_action_required().await;
                        match self.wait_for_action().await {
                            Some(action) => self.apply_human_turn(action).await,
                            None => return,
                        }
                    } else {
                        self.run_ai_turn().await;
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

    async fn run_ai_turn(&mut self) {
        let player = self.game.current_player;

        if self.game.check_tsumo(player).is_some() {
            if self.game.execute_action(TurnAction::Tsumo).is_ok() {
                self.send_state().await;
                self.handle_round_end().await;
                return;
            }
        }

        if self.game.can_declare_riichi(player) {
            let options = self.game.get_tenpai_discard_options(player);
            if let Some(tile) = self.choose_best_riichi_tile(player, &options) {
                if self.game.execute_action(TurnAction::RiichiDiscard(tile)).is_ok() {
                    self.send_state().await;
                    self.process_after_ai_action().await;
                    return;
                }
            }
        }

        let tile = self.ai_choose_discard(player);
        let _ = self.game.execute_action(TurnAction::Discard(tile));
        self.send_state().await;
        self.process_after_ai_action().await;
    }

    fn ai_choose_discard(&mut self, player: PlayerId) -> Tile {
        let visible = self.game.build_visible_tiles(player);
        let hand = &self.game.players[player.0].hand;
        let analysis = analyze_discard(&mut self.calc, hand.tiles(), &visible);
        if let Some(best) = analysis.first() {
            best.tile
        } else {
            *hand.tiles().first().unwrap()
        }
    }

    fn choose_best_riichi_tile(&mut self, player: PlayerId, options: &[Tile]) -> Option<Tile> {
        if options.is_empty() {
            return None;
        }
        let hand = &self.game.players[player.0].hand;
        let mut full_hand = hand.clone();
        if let Some(drawn) = self.game.drawn_tile {
            full_hand.add(drawn);
        }

        let mut best_tile = options[0];
        let mut best_count = 0usize;
        for &tile in options {
            let mut sim = full_hand.clone();
            if sim.remove(tile).is_ok() {
                let waits = analyze_wait_tiles(sim.tiles());
                if waits.len() > best_count {
                    best_count = waits.len();
                    best_tile = tile;
                }
            }
        }
        Some(best_tile)
    }

    async fn handle_response_phase(&mut self) {
        let call_options = self.game.get_call_options();
        let human_options: Vec<CallOption> = call_options
            .iter()
            .filter(|o| o.player == PlayerId(0))
            .cloned()
            .collect();

        if !human_options.is_empty() {
            let _ = self.event_tx.send(ServerEvent::CallRequired {
                options: human_options.clone(),
            }).await;

            match self.wait_for_call().await {
                Some(PlayerAction::CallResponse(CallResponseMsg::Ron)) => {
                    let _ = self.game.execute_call(PlayerId(0), ResponseAction::Ron);
                    self.send_state().await;
                    self.handle_round_end().await;
                    return;
                }
                _ => {
                    let _ = self.game.execute_call(PlayerId(0), ResponseAction::Pass);
                }
            }
        }

        let phase = self.game.phase.clone();
        let discarder = match phase {
            GamePhase::ResponsePhase { discarder, .. } => discarder,
            GamePhase::ChankanResponse { kakan_player, .. } => kakan_player,
            _ => return,
        };

        let call_options = self.game.get_call_options();
        let ai_ron = call_options.iter().find(|o| {
            o.player != discarder
                && o.player != PlayerId(0)
                && matches!(o.call_type, CallType::Ron)
        });
        if let Some(r) = ai_ron {
            let pid = r.player;
            let _ = self.game.execute_call(pid, ResponseAction::Ron);
            self.send_state().await;
            self.handle_round_end().await;
            return;
        }

        let _ = self.game.execute_call(discarder, ResponseAction::Pass);
        self.send_state().await;

        if matches!(self.game.phase, GamePhase::DrawPhase) {
            if self.game.draw().is_err() {
                self.handle_round_end().await;
            }
            self.send_state().await;
        } else if matches!(self.game.phase, GamePhase::RoundOver) {
            self.handle_round_end().await;
        }
    }

    async fn process_after_ai_action(&mut self) {
        loop {
            let phase = self.game.phase.clone();
            match phase {
                GamePhase::ResponsePhase { discarder, .. }
                | GamePhase::ChankanResponse { kakan_player: discarder, .. } => {
                    let call_options = self.game.get_call_options();
                    let human_options: Vec<CallOption> = call_options
                        .iter()
                        .filter(|o| o.player == PlayerId(0))
                        .cloned()
                        .collect();

                    if !human_options.is_empty() {
                        let _ = self.event_tx.send(ServerEvent::CallRequired {
                            options: human_options,
                        }).await;

                        match self.wait_for_call().await {
                            Some(PlayerAction::CallResponse(CallResponseMsg::Ron)) => {
                                let _ = self.game.execute_call(PlayerId(0), ResponseAction::Ron);
                                self.send_state().await;
                                self.handle_round_end().await;
                                return;
                            }
                            _ => {
                                let _ = self.game.execute_call(PlayerId(0), ResponseAction::Pass);
                            }
                        }
                    }

                    let call_options = self.game.get_call_options();
                    let ai_ron = call_options.iter().find(|o| {
                        o.player != discarder
                            && o.player != PlayerId(0)
                            && matches!(o.call_type, CallType::Ron)
                    });
                    if let Some(r) = ai_ron {
                        let pid = r.player;
                        let _ = self.game.execute_call(pid, ResponseAction::Ron);
                        self.send_state().await;
                        self.handle_round_end().await;
                        return;
                    }

                    let _ = self.game.execute_call(discarder, ResponseAction::Pass);
                    self.send_state().await;

                    if matches!(self.game.phase, GamePhase::DrawPhase) {
                        if self.game.draw().is_err() {
                            self.handle_round_end().await;
                        }
                        self.send_state().await;
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

    async fn apply_human_turn(&mut self, action: PlayerAction) {
        match action {
            PlayerAction::TurnAction(TurnActionMsg::Discard(tile)) => {
                let _ = self.game.execute_action(TurnAction::Discard(tile));
                self.send_state().await;
                self.process_after_ai_action().await;
            }
            PlayerAction::TurnAction(TurnActionMsg::Tsumo) => {
                let _ = self.game.execute_action(TurnAction::Tsumo);
                self.send_state().await;
                self.handle_round_end().await;
            }
            PlayerAction::TurnAction(TurnActionMsg::Riichi) => {
                let options = self.game.get_tenpai_discard_options(PlayerId(0));
                if let Some(tile) = options.first() {
                    let _ = self.game.execute_action(TurnAction::RiichiDiscard(*tile));
                    self.send_state().await;
                    self.process_after_ai_action().await;
                }
            }
            _ => {}
        }
    }

    async fn handle_round_end(&mut self) {
        self.send_state().await;

        if self.game.is_game_over() {
            let _ = self.event_tx.send(ServerEvent::GameOver {
                scores: self.scores(),
            }).await;
        } else {
            self.game.start_round(&mut self.rng);
            self.send_state().await;
        }
    }

    async fn send_state(&self) {
        let players = &self.game.players;
        let discards = [
            players[0].discards.clone(),
            players[1].discards.clone(),
            players[2].discards.clone(),
            players[3].discards.clone(),
        ];
        let mut melds_count = [0usize; 4];
        for i in 0..4 {
            melds_count[i] = players[i].melds.len();
        }

        let hand_tiles = {
            let mut tiles = players[0].hand.tiles().to_vec();
            if let Some(drawn) = self.game.drawn_tile {
                tiles.push(drawn);
            }
            tiles
        };

        let points = [
            players[0].points,
            players[1].points,
            players[2].points,
            players[3].points,
        ];

        let dora = self.game.dora.clone();
        let recent_events: Vec<GameEvent> = self.game.events.iter().rev().take(5).cloned().collect();

        let _ = self.event_tx.send(ServerEvent::StateUpdate {
            phase: self.game.phase.clone(),
            current_player: self.game.current_player,
            drawn_tile: self.game.drawn_tile,
            hand_tiles,
            hand_count: players[0].hand.len(),
            points,
            discards,
            melds_count,
            dora,
            remaining_tiles: self.game.remaining_tiles(),
            round: self.game.round,
            honba: self.game.honba,
            riichi_sticks: self.game.riichi_sticks,
            recent_events,
        }).await;
    }

    async fn send_action_required(&self) {
        let can_tsumo = self.game.check_tsumo(PlayerId(0)).is_some();
        let can_riichi = self.game.can_declare_riichi(PlayerId(0));
        let _ = self.event_tx.send(ServerEvent::ActionRequired {
            can_tsumo,
            can_riichi,
        }).await;
    }

    async fn wait_for_action(&mut self) -> Option<PlayerAction> {
        while let Some(msg) = self.action_rx.recv().await {
            match &msg {
                PlayerAction::TurnAction(_) => return Some(msg),
                _ => {}
            }
        }
        None
    }

    async fn wait_for_call(&mut self) -> Option<PlayerAction> {
        while let Some(msg) = self.action_rx.recv().await {
            match &msg {
                PlayerAction::CallResponse(_) => return Some(msg),
                _ => {}
            }
        }
        None
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
