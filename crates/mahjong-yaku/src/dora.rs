use mahjong_core::tile::{Tile, TileType};

use crate::types::DoraResult;

/// 赤宝牌的 TileType：5m, 5p, 5s
const AKA_DORA_TYPES: [TileType; 3] = [
    TileType(4),  // 5m
    TileType(13), // 5p
    TileType(22), // 5s
];

/// 判断一张牌是否为赤宝牌（副本索引为 0 的 5m/5p/5s）
pub fn is_aka_dora(tile: Tile) -> bool {
    tile.raw() % 4 == 0 && AKA_DORA_TYPES.contains(&tile.tile_type())
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
        mahjong_core::tile::Suit::Man => TileType((rank % 9) as u8),
        mahjong_core::tile::Suit::Pin => TileType(9 + (rank % 9) as u8),
        mahjong_core::tile::Suit::Sou => TileType(18 + (rank % 9) as u8),
        mahjong_core::tile::Suit::Wind => TileType(27 + (rank % 4) as u8),
        mahjong_core::tile::Suit::Dragon => TileType(31 + (rank % 3) as u8),
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
        result.dora += all_tiles.iter().filter(|&&t| t.tile_type() == dora_tile).count() as u8;
    }

    // 里宝牌（仅立直时计算）
    if is_riichi {
        for &indicator in ura_dora_indicators {
            let ura_tile = dora_from_indicator(indicator);
            result.ura_dora += all_tiles.iter().filter(|&&t| t.tile_type() == ura_tile).count() as u8;
        }
    }

    // 赤宝牌：副本索引为 0 的 5m/5p/5s
    result.aka_dora = all_tiles.iter().filter(|&&t| is_aka_dora(t)).count() as u8;

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dora_from_indicator_number() {
        assert_eq!(dora_from_indicator(TileType(0)), TileType(1));  // 1m → 2m
        assert_eq!(dora_from_indicator(TileType(8)), TileType(0));  // 9m → 1m
        assert_eq!(dora_from_indicator(TileType(9)), TileType(10)); // 1p → 2p
        assert_eq!(dora_from_indicator(TileType(17)), TileType(9)); // 9p → 1p
    }

    #[test]
    fn test_dora_from_indicator_wind() {
        assert_eq!(dora_from_indicator(TileType(27)), TileType(28)); // 東 → 南
        assert_eq!(dora_from_indicator(TileType(30)), TileType(27)); // 北 → 東
    }

    #[test]
    fn test_dora_from_indicator_dragon() {
        assert_eq!(dora_from_indicator(TileType(31)), TileType(32)); // 白 → 發
        assert_eq!(dora_from_indicator(TileType(33)), TileType(31)); // 中 → 白
    }

    #[test]
    fn test_is_aka_dora() {
        // 5m copy 0 → 赤宝牌
        assert!(is_aka_dora(Tile::from_raw(16)));
        // 5m copy 1 → 非赤宝牌
        assert!(!is_aka_dora(Tile::from_raw(17)));
        // 5p copy 0 → 赤宝牌
        assert!(is_aka_dora(Tile::from_raw(52)));
        // 5s copy 0 → 赤宝牌
        assert!(is_aka_dora(Tile::from_raw(88)));
        // 1m copy 0 → 非赤宝牌
        assert!(!is_aka_dora(Tile::from_raw(0)));
    }

    #[test]
    fn test_calculate_dora_basic() {
        let tiles: Vec<Tile> = [1u8, 1, 2, 37, 38, 39, 73, 74, 75, 108, 108, 108, 124, 124]
            .iter().map(|&r| Tile::from_raw(r)).collect();
        let indicators = vec![TileType(0)]; // 指示 1m → 宝牌 2m
        let result = calculate_dora(&tiles, &indicators, &[], false);
        assert_eq!(result.dora, 2); // 两张 2m (raw 1, 1 → TileType 0)
    }

    #[test]
    fn test_calculate_aka_dora() {
        // 包含 5m copy 0 (raw=16) 和 5p copy 0 (raw=52)
        let tiles: Vec<Tile> = [16u8, 17, 52, 53, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9]
            .iter().map(|&r| Tile::from_raw(r)).collect();
        let result = calculate_dora(&tiles, &[], &[], false);
        assert_eq!(result.aka_dora, 2); // 5m copy 0 + 5p copy 0
    }
}
