use riichi_core::game_types::{CallOption, GameEvent};
use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use riichi_engine::game::GamePhase;

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
        recent_events: Vec<GameEvent>,
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
}

#[derive(Debug, Clone)]
pub enum CallResponseMsg {
    Pass,
    Ron,
}
