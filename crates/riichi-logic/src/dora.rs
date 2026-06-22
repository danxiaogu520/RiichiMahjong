use riichi_core::tile::{Tile, TileType};

use crate::types::DoraResult;

/// 赤宝牌的 TileType：5m, 5p, 5s
const AKA_DORA_TYPES: [TileType; 3] = [
    TileType(4),  // 5m
    TileType(13), // 5p
    TileType(22), // 5s
];

/// 判断一张牌是否为赤宝牌（副本索引为 0 的 5m/5p/5s）
pub fn is_aka_dora(tile: Tile) -> bool {
    tile.raw().is_multiple_of(4) && AKA_DORA_TYPES.contains(&tile.tile_type())
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

    // 赤宝牌：副本索引为 0 的 5m/5p/5s
    result.aka_dora = all_tiles.iter().filter(|&&t| is_aka_dora(t)).count() as u8;

    result
}
