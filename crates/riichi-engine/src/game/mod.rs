mod abort;
mod action;
mod init;
mod query;
mod riichi;
mod round;
mod settlement;
mod state;
mod win;

use std::collections::HashSet;

use rand::Rng;
use riichi_core::hand::Hand;
use riichi_core::meld::{Meld, MeldKind};
use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use riichi_core::wall::Wall;
use riichi_logic::analysis::{analyze_wait_tiles, is_standard_win};
use riichi_logic::shanten::ShantenCalculator;
use riichi_logic::types::{TileCounts, WinContext};
use riichi_logic::win_check;
use serde::{Deserialize, Serialize};

use crate::action::{CallOption, CallType, GameEvent, ResponseAction, RoundEndReason, TurnAction};
use crate::player::{wind_from_index, FuritenState, Player};

use riichi_core::game_types::GameError::{InvalidAction, WallExhausted};
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
