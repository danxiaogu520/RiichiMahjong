use riichi_core::game::CallOption;
use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use riichi_engine::game::GamePhase;
use tokio::sync::mpsc;

use riichi_server::channel::{
    ActionMsg, CallResponseMsg, ClientHandle, PlayerAction, ServerEvent, TurnActionMsg,
};

pub struct App {
    pub event_rx: mpsc::Receiver<ServerEvent>,
    pub action_tx: mpsc::Sender<ActionMsg>,

    pub phase: GamePhase,
    pub current_player: PlayerId,
    pub hand_tiles: Vec<Tile>,
    pub hand_count: usize,
    pub points: [i32; 4],
    pub discards: [Vec<Tile>; 4],
    pub melds_count: [usize; 4],
    pub dora: Vec<TileType>,
    pub remaining_tiles: usize,
    pub round: u32,
    pub honba: u32,
    pub riichi_sticks: u32,

    pub can_tsumo: bool,
    pub can_riichi: bool,
    pub riichi_options: Vec<Tile>,
    pub ankan_options: Vec<Tile>,
    pub kakan_options: Vec<(usize, Tile)>,
    pub can_kyuushu: bool,
    pub call_options: Vec<CallOption>,

    pub messages: Vec<String>,
    pub selected: usize,
    pub call_selected: usize,
    pub should_quit: bool,
    pub show_result: bool,
    pub game_over: bool,
    pub scores: [i32; 4],
    pub ranking: [usize; 4],
}

impl App {
    pub fn new(handle: ClientHandle) -> Self {
        Self {
            event_rx: handle.event_rx,
            action_tx: handle.action_tx,
            phase: GamePhase::ActionPhase,
            current_player: PlayerId(0),
            hand_tiles: Vec::new(),
            hand_count: 0,
            points: [25000; 4],
            discards: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            melds_count: [0; 4],
            dora: Vec::new(),
            remaining_tiles: 0,
            round: 1,
            honba: 0,
            riichi_sticks: 0,
            can_tsumo: false,
            can_riichi: false,
            riichi_options: Vec::new(),
            ankan_options: Vec::new(),
            kakan_options: Vec::new(),
            can_kyuushu: false,
            call_options: Vec::new(),
            messages: Vec::new(),
            selected: 0,
            call_selected: 0,
            should_quit: false,
            show_result: false,
            game_over: false,
            scores: [0; 4],
            ranking: [0, 1, 2, 3],
        }
    }

    pub async fn process_server_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                ServerEvent::StateUpdate {
                    phase,
                    current_player,
                    hand_tiles,
                    hand_count,
                    points,
                    discards,
                    melds_count,
                    dora,
                    remaining_tiles,
                    round,
                    honba,
                    riichi_sticks,
                    ..
                } => {
                    self.phase = phase;
                    self.current_player = current_player;
                    self.hand_tiles = hand_tiles;
                    self.hand_count = hand_count;
                    self.points = points;
                    self.discards = discards;
                    self.melds_count = melds_count;
                    self.dora = dora;
                    self.remaining_tiles = remaining_tiles;
                    self.round = round;
                    self.honba = honba;
                    self.riichi_sticks = riichi_sticks;
                    if self.selected >= self.hand_tiles.len() {
                        self.selected = 0;
                    }
                }
                ServerEvent::ActionRequired {
                    can_tsumo,
                    can_riichi,
                    riichi_options,
                    ankan_options,
                    kakan_options,
                    can_kyuushu,
                    ..
                } => {
                    self.can_tsumo = can_tsumo;
                    self.can_riichi = can_riichi;
                    self.riichi_options = riichi_options;
                    self.ankan_options = ankan_options;
                    self.kakan_options = kakan_options;
                    self.can_kyuushu = can_kyuushu;
                    self.call_options.clear();
                }
                ServerEvent::CallRequired { options } => {
                    self.call_options = options;
                    self.call_selected = 0;
                }
                ServerEvent::GameOver { scores, ranking } => {
                    self.scores = scores;
                    self.ranking = ranking;
                    self.game_over = true;
                    self.show_result = true;
                }
                ServerEvent::Error(message) => {
                    self.messages.push(message);
                }
            }
        }
    }

    pub fn is_human_turn(&self) -> bool {
        self.current_player == PlayerId(0)
            && matches!(self.phase, GamePhase::ActionPhase)
            && self.call_options.is_empty()
    }

    pub fn needs_human_response(&self) -> bool {
        !self.call_options.is_empty()
    }

    pub fn send_discard(&self, tile: Tile) {
        let _ = self.action_tx.try_send((
            PlayerId(0),
            PlayerAction::TurnAction(TurnActionMsg::Discard(tile)),
        ));
    }

    pub fn send_tsumo(&self) {
        let _ = self
            .action_tx
            .try_send((PlayerId(0), PlayerAction::TurnAction(TurnActionMsg::Tsumo)));
    }

    pub fn send_riichi(&self) {
        let Some(tile) = self.riichi_options.first().copied() else {
            return;
        };
        let _ = self
            .action_tx
            .try_send((PlayerId(0), PlayerAction::TurnAction(TurnActionMsg::RiichiDiscard(tile))));
    }

    pub fn send_ankan(&self, tile: Tile) {
        let _ = self.action_tx.try_send((
            PlayerId(0),
            PlayerAction::TurnAction(TurnActionMsg::Ankan(tile)),
        ));
    }

    pub fn send_kakan(&self, meld_index: usize, tile: Tile) {
        let _ = self.action_tx.try_send((
            PlayerId(0),
            PlayerAction::TurnAction(TurnActionMsg::Kakan(meld_index, tile)),
        ));
    }

    pub fn send_kyuushu(&self) {
        let _ = self.action_tx.try_send((
            PlayerId(0),
            PlayerAction::TurnAction(TurnActionMsg::KyuushuKyuuhai),
        ));
    }

    pub fn send_call_ron(&self) {
        let _ = self.action_tx.try_send((
            PlayerId(0),
            PlayerAction::CallResponse(CallResponseMsg::Ron),
        ));
    }

    pub fn send_call_pon(&self, hand_tiles: [Tile; 2]) {
        let _ = self.action_tx.try_send((
            PlayerId(0),
            PlayerAction::CallResponse(CallResponseMsg::Pon { hand_tiles }),
        ));
    }

    pub fn send_call_chi(&self, hand_tiles: [Tile; 2]) {
        let _ = self.action_tx.try_send((
            PlayerId(0),
            PlayerAction::CallResponse(CallResponseMsg::Chi { hand_tiles }),
        ));
    }

    pub fn send_call_minkan(&self, hand_tiles: [Tile; 3]) {
        let _ = self.action_tx.try_send((
            PlayerId(0),
            PlayerAction::CallResponse(CallResponseMsg::Minkan { hand_tiles }),
        ));
    }

    pub fn send_call_pass(&self) {
        let _ = self.action_tx.try_send((
            PlayerId(0),
            PlayerAction::CallResponse(CallResponseMsg::Pass),
        ));
    }

    pub fn player_name(&self, idx: usize) -> &str {
        match idx {
            0 => "你",
            1 => "AI-南",
            2 => "AI-西",
            3 => "AI-北",
            _ => "?",
        }
    }

    pub fn round_display(&self) -> String {
        let wind = if self.round <= 4 { "东" } else { "南" };
        let num = (self.round.saturating_sub(1) % 4) + 1;
        format!("{}{}局", wind, num)
    }
}
