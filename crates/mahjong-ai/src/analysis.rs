use std::collections::HashSet;

use mahjong_core::tile::{Tile, TileType};
use mahjong_yaku::types::TileCounts;

use crate::shanten::ShantenCalculator;

#[derive(Debug, Clone)]
pub struct DiscardOption {
    pub tile: Tile,
    pub shanten: i8,
    pub acceptance_types: usize,
    pub acceptance_copies: usize,
    pub improvement_types: usize,
    pub improvement_copies: usize,
}

pub fn analyze_discard(
    calculator: &mut ShantenCalculator,
    hand: &[Tile],
    visible: &VisibleTiles,
) -> Vec<DiscardOption> {
    let current_counts = TileCounts::from_tiles(hand);
    let current_shanten = calculator.lookup(&current_counts);
    let mut results = Vec::new();
    let mut seen_types = HashSet::new();

    for &tile in hand {
        let tt = tile.tile_type();
        if !seen_types.insert(tt) {
            continue;
        }

        let mut after_discard = current_counts;
        after_discard.dec(tt);
        let new_shanten = calculator.lookup(&after_discard);

        let mut acceptance_types = 0usize;
        let mut acceptance_copies = 0usize;
        let acceptance_tiles = find_acceptance(calculator, &after_discard, new_shanten);

        for &att in &acceptance_tiles {
            let remaining = remaining_copies(att, &after_discard, visible);
            if remaining > 0 {
                acceptance_types += 1;
                acceptance_copies += remaining;
            }
        }

        let mut improvement_types = 0usize;
        let mut improvement_copies = 0usize;

        if new_shanten == current_shanten {
            for i in 0..34u8 {
                let draw_tt = TileType(i);
                let rem = remaining_copies(draw_tt, &after_discard, visible);
                if rem == 0 {
                    continue;
                }

                let mut after_draw = after_discard;
                after_draw.inc(draw_tt);

                let new_acceptance = find_acceptance(calculator, &after_draw, new_shanten);
                let old_acceptance_count = acceptance_tiles.len();

                if new_acceptance.len() > old_acceptance_count {
                    improvement_types += 1;
                    improvement_copies += rem;
                }
            }
        }

        results.push(DiscardOption {
            tile,
            shanten: new_shanten,
            acceptance_types,
            acceptance_copies,
            improvement_types,
            improvement_copies,
        });
    }

    results.sort_by(|a, b| {
        a.shanten
            .cmp(&b.shanten)
            .then(b.acceptance_copies.cmp(&a.acceptance_copies))
            .then(b.improvement_copies.cmp(&a.improvement_copies))
    });

    if let Some(min_shanten) = results.first().map(|r| r.shanten) {
        results.retain(|r| r.shanten == min_shanten);
    }

    results
}

#[derive(Debug, Clone)]
pub struct AcceptanceInfo {
    pub tile: TileType,
    pub copies: usize,
    pub new_shanten: i8,
}

pub fn analyze_acceptance(
    calculator: &mut ShantenCalculator,
    hand: &[Tile],
    visible: &VisibleTiles,
) -> (Vec<AcceptanceInfo>, Vec<AcceptanceInfo>, i8) {
    let counts = TileCounts::from_tiles(hand);
    let current_shanten = calculator.lookup(&counts);

    let mut acceptance = Vec::new();
    let mut improvement = Vec::new();

    for i in 0..34u8 {
        let tt = TileType(i);
        let rem = remaining_copies(tt, &counts, visible);
        if rem == 0 {
            continue;
        }

        let mut after = counts;
        after.inc(tt);
        let new_shanten = calculator.lookup(&after);

        if new_shanten < current_shanten {
            acceptance.push(AcceptanceInfo { tile: tt, copies: rem, new_shanten });
        } else if new_shanten == current_shanten {
            let current_acceptance_count = find_acceptance(calculator, &counts, current_shanten).len();
            let new_acceptance_count = find_acceptance(calculator, &after, current_shanten).len();
            if new_acceptance_count > current_acceptance_count {
                improvement.push(AcceptanceInfo { tile: tt, copies: rem, new_shanten });
            }
        }
    }

    (acceptance, improvement, current_shanten)
}

fn find_acceptance(
    calculator: &mut ShantenCalculator,
    counts: &TileCounts,
    current_shanten: i8,
) -> Vec<TileType> {
    let mut result = Vec::new();
    for i in 0..34u8 {
        let tt = TileType(i);
        if counts.get(tt) >= 4 {
            continue;
        }
        let mut after = *counts;
        after.inc(tt);
        if calculator.lookup(&after) < current_shanten {
            result.push(tt);
        }
    }
    result
}

fn remaining_copies(tt: TileType, hand_counts: &TileCounts, visible: &VisibleTiles) -> usize {
    let total = 4usize;
    let used = hand_counts.get(tt) as usize
        + visible.hand_melds.get(tt) as usize
        + visible.all_discards.get(tt) as usize
        + visible.all_melds.get(tt) as usize
        + visible.dora_indicators.get(tt) as usize;
    total.saturating_sub(used)
}

pub struct VisibleTiles {
    pub hand_melds: TileCounts,
    pub all_discards: TileCounts,
    pub all_melds: TileCounts,
    pub dora_indicators: TileCounts,
}

impl VisibleTiles {
    pub fn new() -> Self {
        Self {
            hand_melds: TileCounts::new(),
            all_discards: TileCounts::new(),
            all_melds: TileCounts::new(),
            dora_indicators: TileCounts::new(),
        }
    }
}
