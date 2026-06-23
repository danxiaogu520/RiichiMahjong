use riichi_core::game_types::GameEvent;
use riichi_core::player::PlayerId;
use riichi_core::player_state::Player;
use riichi_core::tile::{Tile, TileType};
use riichi_core::wall::Wall;
use serde::{Deserialize, Serialize};

pub use riichi_core::game_types::{extract_kuikae_tiles, GameError, GamePhase};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    pub players: [Player; 4],
    pub wind: TileType,
    pub round: u32,
    pub honba: u32,
    pub riichi_sticks: u32,
    pub current_player: PlayerId,
    pub drawn_tile: Option<Tile>,
    pub wall: Wall,
    pub dora: Vec<TileType>,
    pub dora_indicators: Vec<TileType>,
    pub ura_dora_indicators: Vec<TileType>,
    pub events: Vec<GameEvent>,
    pub phase: GamePhase,
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}
