use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::hand::Hand;
use crate::meld::Meld;
use crate::player::PlayerId;
use crate::tile::{Tile, TileType};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FuritenState {
    pub discard: bool,
    pub round: bool,
    pub riichi: bool,
}

impl FuritenState {
    pub fn is_furiten(&self) -> bool {
        self.discard || self.round || self.riichi
    }
    pub fn clear_round(&mut self) {
        self.round = false;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Player {
    pub id: PlayerId,
    pub hand: Hand,
    pub points: i32,
    pub wind: TileType,
    pub discards: Vec<Tile>,
    pub melds: Vec<Meld>,
    pub is_riichi: bool,
    pub is_ippatsu: bool,
    pub forbidden: Vec<TileType>,
    pub riichi_declaration_tile: Option<Tile>,
    pub has_made_first_action: bool,
    pub is_double_riichi: bool,
    pub furiten: FuritenState,
    pub all_discarded_types: HashSet<TileType>,
}

impl Player {
    pub fn new(id: PlayerId, wind: TileType) -> Self {
        Self {
            id,
            hand: Hand::new(),
            wind,
            discards: Vec::new(),
            melds: Vec::new(),
            points: 25000,
            is_riichi: false,
            forbidden: Vec::new(),
            riichi_declaration_tile: None,
            is_ippatsu: false,
            has_made_first_action: false,
            is_double_riichi: false,
            furiten: FuritenState::default(),
            all_discarded_types: HashSet::new(),
        }
    }

    pub fn is_menzen(&self) -> bool {
        self.melds.iter().all(|m| m.is_concealed())
    }
}
