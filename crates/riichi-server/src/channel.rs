use riichi_core::game::CallOption;
use riichi_core::meld::Meld;
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
        pending_discard: Option<(PlayerId, Tile)>,
        drawn_tile: Option<Tile>,
        hand_tiles: Vec<Tile>,
        hand_count: usize,
        hand_counts: [usize; 4],
        points: [i32; 4],
        winds: [TileType; 4],
        is_riichi: [bool; 4],
        discards: [Vec<Tile>; 4],
        melds_count: [usize; 4],
        melds: [Vec<Meld>; 4],
        dora: Vec<TileType>,
        remaining_tiles: usize,
        round: u32,
        honba: u32,
        riichi_sticks: u32,
    },
    ActionRequired {
        can_tsumo: bool,
        can_riichi: bool,
        riichi_options: Vec<Tile>,
        discard_options: Vec<Tile>,
        ankan_options: Vec<Tile>,
        kakan_options: Vec<(usize, Tile)>,
        can_kyuushu: bool,
    },
    CallRequired {
        options: Vec<CallOption>,
    },
    RoundResult {
        reason: String,
        win_details: Vec<String>,
        point_changes: [i32; 4],
        scores: [i32; 4],
    },
    GameOver {
        scores: [i32; 4],
        ranking: [usize; 4],
    },
    Error(String),
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
    /// 明确指定立直宣言时打出的牌。
    RiichiDiscard(Tile),
    /// 兼容旧版 AI 客户端，由服务端选择第一个合法弃牌。
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
