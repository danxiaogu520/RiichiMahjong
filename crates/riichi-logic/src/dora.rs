use riichi_core::tile::{Tile, TileType};

use crate::types::DoraResult;

/// 赤宝牌的 TileType：5m, 5p, 5s
const AKA_DORA_TYPES: [TileType; 3] = [
    TileType(4),  // 5m
    TileType(13), // 5p
    TileType(22), // 5s
];

/// 判断一张牌是否为赤宝牌。
///
/// `red_fives` 按万、筒、索分别指定赤五数量。当前四张牌编码中，
/// 5 的副本索引 0..3 对应同一种牌，因此支持标准规则下的 0 或 1 张，
/// 同时保留扩展到多个赤五的能力。
pub fn is_aka_dora(tile: Tile, red_fives: [u8; 3]) -> bool {
    let Some(suit_index) = AKA_DORA_TYPES
        .iter()
        .position(|&tile_type| tile_type == tile.tile_type())
    else {
        return false;
    };
    (tile.raw() % 4) < red_fives[suit_index].min(4)
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
/// - `red_fives`: 万、筒、索三种赤五数量
pub fn calculate_dora(
    all_tiles: &[Tile],
    dora_indicators: &[TileType],
    ura_dora_indicators: &[TileType],
    is_riichi: bool,
    red_fives: [u8; 3],
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

    // 赤宝牌：按规则配置统计 5m/5p/5s 的赤色副本。
    result.aka_dora = all_tiles
        .iter()
        .filter(|&&t| is_aka_dora(t, red_fives))
        .count() as u8;

    result
}

#[cfg(test)]
mod tests {
    use super::{calculate_dora, is_aka_dora};
    use riichi_core::tile::Tile;

    #[test]
    fn red_five_count_follows_rule_config() {
        let five_man_copies = [
            Tile::from_raw(16),
            Tile::from_raw(17),
            Tile::from_raw(18),
            Tile::from_raw(19),
        ];
        assert!(is_aka_dora(five_man_copies[0], [1, 0, 0]));
        assert!(!is_aka_dora(five_man_copies[1], [1, 0, 0]));
        assert!(is_aka_dora(five_man_copies[1], [2, 0, 0]));
        assert!(!is_aka_dora(five_man_copies[0], [0, 0, 0]));
    }

    #[test]
    fn dora_result_uses_configured_red_fives() {
        let tiles = vec![Tile::from_raw(16), Tile::from_raw(17)];
        let no_red = calculate_dora(&tiles, &[], &[], false, [0, 0, 0]);
        let two_red = calculate_dora(&tiles, &[], &[], false, [2, 0, 0]);
        assert_eq!(no_red.aka_dora, 0);
        assert_eq!(two_red.aka_dora, 2);
    }
}
