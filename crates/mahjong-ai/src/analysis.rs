use mahjong_core::tile::{Tile, TileType};
use mahjong_yaku::types::TileCounts;

use crate::shanten::ShantenCalculator;

#[derive(Debug, Clone)]
pub struct DiscardAnalysis {
    pub tile: Tile,
    pub acceptance: usize,
    pub improvement: usize,
    pub shanten: i8,
}

pub fn analyze_discard(calculator: &mut ShantenCalculator, hand: &[Tile]) -> Vec<DiscardAnalysis> {
    let mut results = Vec::new();
    let current_counts = TileCounts::from_tiles(hand);
    let current_shanten = calculator.calculate_from_counts(&current_counts);

    // 去重：同类型牌只分析一次
    let mut seen_types = std::collections::HashSet::new();

    for &tile in hand {
        let tt = tile.tile_type();
        if !seen_types.insert(tt) {
            continue;
        }

        // 模拟打出这张牌
        let mut after_discard = current_counts;
        after_discard.dec(tt);

        let new_shanten = calculator.calculate_from_counts(&after_discard);

        // 计算进张：遍历 34 种牌，看哪些能减少向听数
        let mut acceptance = 0usize;
        let mut acceptance_tiles = Vec::new();

        for i in 0..34u8 {
            let draw_tt = TileType(i);
            if after_discard.get(draw_tt) >= 4 {
                continue;
            }

            let mut after_draw = after_discard;
            after_draw.inc(draw_tt);

            let draw_shanten = calculator.calculate_from_counts(&after_draw);
            if draw_shanten < new_shanten {
                let copies = 4 - after_discard.get(draw_tt) as usize;
                acceptance += copies;
                acceptance_tiles.push(draw_tt);
            }
        }

        // 计算改良：进张数比打出前更多的打牌选择
        // 这里简化处理：如果打出后向听数不变，计算进张数作为改良参考
        let improvement = if new_shanten == current_shanten {
            // 向听数不变，看进张是否比平均更多（简化：直接用进张数）
            acceptance
        } else {
            0
        };

        results.push(DiscardAnalysis {
            tile,
            acceptance,
            improvement,
            shanten: new_shanten,
        });
    }

    results
}

pub fn count_acceptance(
    calculator: &mut ShantenCalculator,
    hand: &[Tile],
) -> (usize, Vec<TileType>) {
    let counts = TileCounts::from_tiles(hand);
    let current_shanten = calculator.calculate_from_counts(&counts);

    let mut total = 0usize;
    let mut tiles = Vec::new();

    for i in 0..34u8 {
        let tt = TileType(i);
        if counts.get(tt) >= 4 {
            continue;
        }

        let mut after_draw = counts;
        after_draw.inc(tt);

        let draw_shanten = calculator.calculate_from_counts(&after_draw);
        if draw_shanten < current_shanten {
            total += 4 - counts.get(tt) as usize;
            tiles.push(tt);
        }
    }

    (total, tiles)
}
