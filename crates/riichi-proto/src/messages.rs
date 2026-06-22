use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    JoinRoom { room_id: String },
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
    Event(GameEventView),
    ActionRequired(ActionRequest),
    CallRequired(CallRequest),
    RoundResult(RoundResultView),
    GameOver {
        scores: [i32; 4],
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
    pub current_player: PlayerId,
    pub drawn_tile: Option<Tile>,
    pub dora: Vec<TileType>,
    pub remaining_tiles: usize,
    pub phase: GamePhaseView,
    pub recent_events: Vec<GameEventView>,
    pub analysis: Option<AnalysisInfo>,
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
    DrawPhase,
    ActionPhase,
    ResponsePhase,
    ChankanResponse,
    RoundOver,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameEventView {
    GameStarted {
        dealer: PlayerId,
    },
    RoundStarted {
        round_number: u32,
        dealer: PlayerId,
    },
    PlayerDrew {
        player: PlayerId,
    },
    PlayerDiscarded {
        player: PlayerId,
        tile: Tile,
    },
    PlayerCalledPon {
        player: PlayerId,
        tiles: Vec<Tile>,
        from_player: PlayerId,
    },
    PlayerCalledChi {
        player: PlayerId,
        tiles: Vec<Tile>,
        from_player: PlayerId,
    },
    PlayerCalledMinkan {
        player: PlayerId,
        tiles: Vec<Tile>,
        from_player: PlayerId,
    },
    PlayerCalledAnkan {
        player: PlayerId,
        tiles: Vec<Tile>,
    },
    PlayerCalledKakan {
        player: PlayerId,
        tile: Tile,
    },
    PlayerDeclaredRiichi {
        player: PlayerId,
    },
    PlayerWon {
        player: PlayerId,
        is_tsumo: bool,
        points: i32,
        yaku_names: Vec<String>,
    },
    RoundEnded {
        reason: RoundEndReasonView,
    },
    ExhaustiveDrawResult {
        tenpai: [bool; 4],
        payments: [i32; 4],
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RoundEndReasonView {
    ExhaustiveDraw,
    Win { winner: PlayerId, is_tsumo: bool },
    KyuushuKyuuhai,
    SuufonRenda,
    SuuchaRiichi,
    SuuKantsu,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRequest {
    pub player: PlayerId,
    pub can_tsumo: bool,
    pub can_riichi: bool,
    pub ankan_options: Vec<Tile>,
    pub kakan_options: Vec<(usize, Tile)>,
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
