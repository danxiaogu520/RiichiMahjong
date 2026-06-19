use std::collections::HashSet;

use mahjong_core::hand::Hand;
use mahjong_core::meld::Meld;
use mahjong_core::player::PlayerId;
use mahjong_core::tile::{Tile, TileType};

use serde::{Deserialize, Serialize};

// ─── 风位辅助函数 ──────────────────────────────────────────

/// 从玩家索引 (0-3) 获取自风 TileType
pub fn wind_from_index(index: usize) -> TileType {
    match index % 4 {
        0 => TileType::EAST,
        1 => TileType::SOUTH,
        2 => TileType::WEST,
        3 => TileType::NORTH,
        _ => unreachable!(),
    }
}

/// 下一个风位（东→南→西→北→东）
pub fn next_wind(wind: TileType) -> TileType {
    match wind {
        TileType::EAST => TileType::SOUTH,
        TileType::SOUTH => TileType::WEST,
        TileType::WEST => TileType::NORTH,
        TileType::NORTH => TileType::EAST,
        _ => panic!("不是风牌: {:?}", wind),
    }
}

/// 风牌的中文显示（东/南/西/北）
pub fn wind_display(wind: TileType) -> &'static str {
    match wind {
        TileType::EAST => "东",
        TileType::SOUTH => "南",
        TileType::WEST => "西",
        TileType::NORTH => "北",
        _ => "？",
    }
}

// ─── 振听状态 ──────────────────────────────────────────────

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

// ─── 玩家状态 ──────────────────────────────────────────────

/// 玩家状态
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

    /// 是否门清（无明副露）
    pub fn is_menzen(&self) -> bool {
        self.melds.iter().all(|m| m.is_concealed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wind_rotation() {
        assert_eq!(next_wind(TileType::EAST), TileType::SOUTH);
        assert_eq!(next_wind(TileType::SOUTH), TileType::WEST);
        assert_eq!(next_wind(TileType::WEST), TileType::NORTH);
        assert_eq!(next_wind(TileType::NORTH), TileType::EAST);
    }

    #[test]
    fn test_wind_from_index() {
        assert_eq!(wind_from_index(0), TileType::EAST);
        assert_eq!(wind_from_index(1), TileType::SOUTH);
        assert_eq!(wind_from_index(2), TileType::WEST);
        assert_eq!(wind_from_index(3), TileType::NORTH);
        assert_eq!(wind_from_index(4), TileType::EAST); // wrap
    }

    #[test]
    fn test_wind_display() {
        assert_eq!(wind_display(TileType::EAST), "东");
        assert_eq!(wind_display(TileType::SOUTH), "南");
        assert_eq!(wind_display(TileType::WEST), "西");
        assert_eq!(wind_display(TileType::NORTH), "北");
    }
}
