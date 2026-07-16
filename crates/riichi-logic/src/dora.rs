use riichi_core::tile::{Tile, TileType};

use crate::model::DoraResult;

/// 赤宝牌的 TileType：5m, 5p, 5s
const AKA_DORA_TYPES: [TileType; 3] = [
    TileType(4),  // 5m
    TileType(13), // 5p
    TileType(22), // 5s
];

/// 判断一张牌是否为赤宝牌。
///
/// 固定规则下万、筒、索各有一张赤五，均使用该牌种的 0 号副本。
pub fn is_aka_dora(tile: Tile) -> bool {
    let Some(suit_index) = AKA_DORA_TYPES
        .iter()
        .position(|&tile_type| tile_type == tile.tile_type())
    else {
        return false;
    };
    let _ = suit_index;
    tile.copy_index() == 0
}

/// 从指示牌推导宝牌
///
/// 指示牌 → 宝牌的循环映射：
/// - 数牌：7→8→9→1（同花色内循环）
/// - 风牌：东→南→西→北→东
/// - 三元牌：白→發→中→白
pub fn dora_from_indicator(indicator: TileType) -> TileType {
    let suit = indicator.suit();
    let rank = indicator.rank().0;
    match suit {
        riichi_core::tile::Suit::Man => TileType(rank % 9),
        riichi_core::tile::Suit::Pin => TileType(9 + (rank % 9)),
        riichi_core::tile::Suit::Sou => TileType(18 + (rank % 9)),
        riichi_core::tile::Suit::Wind => TileType(27 + (rank % 4)),
        riichi_core::tile::Suit::Dragon => TileType(31 + (rank % 3)),
    }
}

/// 计算手牌+副露+和了牌中的宝牌数量
///
/// - `all_tiles`: 手牌 + 副露中的所有牌 + 和了牌的实体牌
/// - `dora_indicators`: 宝牌指示牌列表
/// - `ura_dora_indicators`: 里宝牌指示牌列表（仅立直时有效）
/// - `is_riichi`: 是否立直
pub fn calculate_dora(
    all_tiles: &[Tile],
    dora_indicators: &[TileType],
    ura_dora_indicators: &[TileType],
    is_riichi: bool,
) -> DoraResult {
    let mut result = DoraResult::default();

    // 宝牌
    for &indicator in dora_indicators {
        let dora_tile = dora_from_indicator(indicator);
        result.dora += all_tiles
            .iter()
            .filter(|&&t| t.tile_type() == dora_tile)
            .count() as u8;
    }

    // 里宝牌（仅立直时计算）
    if is_riichi {
        for &indicator in ura_dora_indicators {
            let ura_tile = dora_from_indicator(indicator);
            result.ura_dora += all_tiles
                .iter()
                .filter(|&&t| t.tile_type() == ura_tile)
                .count() as u8;
        }
    }

    // 固定规则：5m/5p/5s 各有一枚赤五。
    result.aka_dora = all_tiles.iter().filter(|&&t| is_aka_dora(t)).count() as u8;

    result
}

#[cfg(test)]
mod tests {
    use super::{calculate_dora, is_aka_dora};
    use riichi_core::tile::Tile;

    #[test]
    fn only_zero_copy_fives_are_red() {
        let five_man_copies = [
            Tile::from_raw(16),
            Tile::from_raw(17),
            Tile::from_raw(18),
            Tile::from_raw(19),
        ];
        assert!(is_aka_dora(five_man_copies[0]));
        assert!(!is_aka_dora(five_man_copies[1]));
    }

    #[test]
    fn fixed_rule_has_one_red_five_per_suit() {
        let tiles = vec![Tile::from_raw(16), Tile::from_raw(17)];
        let result = calculate_dora(&tiles, &[], &[], false);
        assert_eq!(result.aka_dora, 1);
    }
}
