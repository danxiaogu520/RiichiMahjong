use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use serde::{Deserialize, Serialize};

/// 当前客户端与服务端共同支持的线协议版本。
pub const PROTOCOL_VERSION: u16 = 1;

/// 客户端发送给服务端的统一消息外壳。
///
/// `command_id` 用于识别重复提交，`expected_seq` 用于让服务端发现
/// 客户端基于过期状态发起的行动。身份不放在外壳中，服务端应从连接
/// 会话中取得经过认证的座位。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientEnvelope {
    pub protocol_version: u16,
    pub command_id: u64,
    pub expected_seq: u64,
    pub body: ClientMessage,
}

/// 服务端发送给客户端的统一消息外壳。
///
/// 同一连接上的 `seq` 必须单调递增；客户端可以用它检测丢失或乱序
/// 的事件，并在需要时请求完整状态快照。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerEnvelope {
    pub protocol_version: u16,
    pub seq: u64,
    pub body: ServerMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    JoinRoom { room_id: String },
    RequestSnapshot,
    TurnAction { action: TurnActionPayload },
    CallResponse { action: CallResponsePayload },
    Ready,
    LeaveRoom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TurnActionPayload {
    Discard(Tile),
    RiichiDiscard(Tile),
    Tsumo,
    Ankan(Tile),
    Kakan(usize, Tile),
    KyuushuKyuuhai,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CallResponsePayload {
    Pass,
    Ron,
    Pon { hand_tiles: [Tile; 2] },
    Chi { hand_tiles: [Tile; 2] },
    Minkan { hand_tiles: [Tile; 3] },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    RoomJoined {
        room_id: String,
        player_id: PlayerId,
    },
    StateUpdate(Box<GameStateView>),
    StateSnapshot(Box<GameStateView>),
    Event {
        event_id: u64,
        event: GameEventView,
    },
    CommandAccepted {
        command_id: u64,
        seq: u64,
    },
    CommandRejected {
        command_id: u64,
        expected_seq: u64,
        actual_seq: u64,
        reason: String,
    },
    ActionRequired(ActionRequest),
    CallRequired(CallRequest),
    RoundResult(RoundResultView),
    GameOver {
        scores: [i32; 4],
        ranking: [usize; 4],
    },
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameStateView {
    pub players: [PlayerView; 4],
    pub wind: TileType,
    pub round: u32,
    pub honba: u32,
    pub riichi_sticks: u32,
    pub dora: Vec<TileType>,
    pub remaining_tiles: usize,
    pub phase: GamePhaseView,
    pub recent_events: Vec<GameEventView>,
    pub analysis: Option<AnalysisInfo>,
    pub tenpai_info: Option<TenpaiInfoView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenpaiInfoView {
    pub waits: Vec<WaitInfoView>,
    pub is_furiten: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitInfoView {
    pub tile_type: TileType,
    pub remaining: usize,
    pub is_no_yaku: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerView {
    pub id: PlayerId,
    pub hand: Option<Vec<Tile>>,
    pub hand_count: usize,
    pub points: i32,
    pub wind: TileType,
    pub discards: Vec<Tile>,
    pub melds: Vec<MeldView>,
    pub is_riichi: bool,
    pub riichi_declaration_tile: Option<Tile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeldView {
    pub kind: MeldKindView,
    pub tiles: Vec<Tile>,
    pub from_player: Option<PlayerId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MeldKindView {
    Chi,
    Pon,
    Ankan,
    Minkan,
    Kakan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GamePhaseView {
    DrawPhase {
        player: PlayerId,
        position: DrawPositionView,
    },
    ActionPhase {
        player: PlayerId,
        drawn_tile: Option<Tile>,
    },
    ResponsePhase {
        player: PlayerId,
        discarded_tile: Tile,
    },
    ChankanResponse {
        player: PlayerId,
        kan_tile: Tile,
    },
    RoundOver,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DrawPositionView {
    LiveWall,
    Rinshan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameEventView {
    Draw {
        player: PlayerId,
        tile: Option<Tile>,
    },
    Discard {
        player: PlayerId,
        tile: Tile,
        kind: DiscardKindView,
    },
    Call {
        player: PlayerId,
        kind: CallKindView,
        tiles: Vec<Tile>,
        called_tile: Option<Tile>,
        from_player: Option<PlayerId>,
        meld_index: Option<usize>,
    },
    Pass {
        player: PlayerId,
    },
    Riichi {
        player: PlayerId,
    },
    Win {
        winners: Vec<PlayerId>,
        tile: Tile,
        kind: WinKindView,
        loser: Option<PlayerId>,
    },
    AbortiveDraw {
        player: Option<PlayerId>,
        reason: RoundEndReasonView,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DiscardKindView {
    Tsumogiri,
    Tedashi,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CallKindView {
    Chi,
    Pon,
    Minkan,
    Ankan,
    Kakan,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WinKindView {
    Ron,
    Tsumo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoundEndReasonView {
    ExhaustiveDraw,
    Win { winner: PlayerId, is_tsumo: bool },
    MultiWin { winners: Vec<PlayerId> },
    KyuushuKyuuhai,
    SuufonRenda,
    SuuchaRiichi,
    SuuKantsu,
    Unknown(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRequest {
    pub player: PlayerId,
    pub can_tsumo: bool,
    pub can_riichi: bool,
    pub riichi_options: Vec<Tile>,
    pub discard_options: Vec<Tile>,
    pub ankan_options: Vec<Tile>,
    pub kakan_options: Vec<(usize, Tile)>,
    pub can_kyuushu: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallRequest {
    pub player: PlayerId,
    pub options: Vec<CallOptionView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallOptionView {
    pub player: PlayerId,
    pub call_type: CallTypeView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CallTypeView {
    Ron,
    Minkan { hand_tiles: [Tile; 3] },
    Pon { hand_tiles: [Tile; 2] },
    Chi { hand_tiles: [Tile; 2] },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundResultView {
    pub reason: RoundEndReasonView,
    pub point_changes: [i32; 4],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisInfo {
    pub discard_options: Vec<DiscardOptionView>,
    pub acceptance: Vec<AcceptanceInfoView>,
    pub improvement: Vec<AcceptanceInfoView>,
    pub current_shanten: i8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscardOptionView {
    pub tile: Tile,
    pub shanten: i8,
    pub acceptance_count: usize,
    pub improvement_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceInfoView {
    pub tile_type: TileType,
    pub copies: usize,
}
