use riichi_core::game::CallOption;
use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use riichi_engine::game::GamePhase;
use tokio::sync::mpsc;

// ═══════════════════════════════════════════════════════════════
//  Server → Client 事件
// ═══════════════════════════════════════════════════════════════

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum ServerEvent {
    StateUpdate {
        phase: GamePhase,
        current_player: PlayerId,
        drawn_tile: Option<Tile>,
        hand_tiles: Vec<Tile>,
        hand_count: usize,
        points: [i32; 4],
        discards: [Vec<Tile>; 4],
        melds_count: [usize; 4],
        dora: Vec<TileType>,
        remaining_tiles: usize,
        round: u32,
        honba: u32,
        riichi_sticks: u32,
    },
    ActionRequired {
        can_tsumo: bool,
        can_riichi: bool,
    },
    CallRequired {
        options: Vec<CallOption>,
    },
    GameOver {
        scores: [i32; 4],
    },
}

// ═══════════════════════════════════════════════════════════════
//  Client → Server 动作
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub enum PlayerAction {
    TurnAction(TurnActionMsg),
    CallResponse(CallResponseMsg),
}

#[derive(Debug, Clone)]
pub enum TurnActionMsg {
    Discard(Tile),
    Tsumo,
    Riichi,
    Ankan(Tile),
    Kakan(usize, Tile),
    KyuushuKyuuhai,
}

#[derive(Debug, Clone)]
pub enum CallResponseMsg {
    Pass,
    Ron,
    Pon { hand_tiles: [Tile; 2] },
    Chi { hand_tiles: [Tile; 2] },
    Minkan { hand_tiles: [Tile; 3] },
}

// ═══════════════════════════════════════════════════════════════
//  PlayerHandle / ClientHandle
// ═══════════════════════════════════════════════════════════════

pub type ActionMsg = (PlayerId, PlayerAction);

pub struct PlayerHandle {
    pub id: PlayerId,
    pub event_tx: mpsc::Sender<ServerEvent>,
    pub action_rx: mpsc::Receiver<ActionMsg>,
}

pub struct ClientHandle {
    pub id: PlayerId,
    pub event_rx: mpsc::Receiver<ServerEvent>,
    pub action_tx: mpsc::Sender<ActionMsg>,
}

pub fn create_player_pair(id: PlayerId) -> (PlayerHandle, ClientHandle) {
    let (event_tx, event_rx) = mpsc::channel(64);
    let (action_tx, action_rx) = mpsc::channel(64);
    (
        PlayerHandle {
            id,
            event_tx,
            action_rx,
        },
        ClientHandle {
            id,
            event_rx,
            action_tx,
        },
    )
}
