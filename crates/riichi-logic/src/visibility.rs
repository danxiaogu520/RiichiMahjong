use crate::model::TileCounts;
use riichi_core::tile::{Tile, TileType};

/// 从某位玩家视角已经可见的牌。它属于规则事实，不包含弃牌策略。
#[derive(Debug, Clone, Default)]
pub struct VisibleTiles {
    pub hand_melds: TileCounts,
    pub all_discards: TileCounts,
    pub all_melds: TileCounts,
    pub dora_indicators: TileCounts,
}

impl VisibleTiles {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_data(
        player_melds: &[Vec<Tile>],
        other_melds: &[Vec<Tile>],
        all_discards: &[Tile],
        dora_indicator_types: &[TileType],
    ) -> Self {
        let mut visible = Self::new();
        for tile in player_melds.iter().flatten() {
            visible.hand_melds.inc(tile.tile_type());
        }
        for tile in other_melds.iter().flatten() {
            visible.all_melds.inc(tile.tile_type());
        }
        for tile in all_discards {
            visible.all_discards.inc(tile.tile_type());
        }
        for &tile_type in dora_indicator_types {
            visible.dora_indicators.inc(tile_type);
        }
        visible
    }
}

pub fn remaining_copies_for(
    tile_type: TileType,
    hand_counts: &TileCounts,
    visible: &VisibleTiles,
) -> usize {
    let used = hand_counts.get(tile_type) as usize
        + visible.hand_melds.get(tile_type) as usize
        + visible.all_discards.get(tile_type) as usize
        + visible.all_melds.get(tile_type) as usize
        + visible.dora_indicators.get(tile_type) as usize;
    4usize.saturating_sub(used)
}
