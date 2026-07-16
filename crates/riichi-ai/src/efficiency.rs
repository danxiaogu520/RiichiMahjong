use std::collections::HashSet;

use riichi_core::tile::{Tile, TileType};
use riichi_logic::model::TileCounts;
use riichi_logic::visibility::{remaining_copies_for, VisibleTiles};

use riichi_logic::shanten::ShantenCalculator;

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
    calculator: &ShantenCalculator,
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
            let old_acceptance_copies =
                count_acceptance_copies(calculator, &after_discard, visible, new_shanten);

            for i in 0..34u8 {
                let draw_tt = TileType(i);
                let draw_rem = remaining_copies(draw_tt, &after_discard, visible);
                if draw_rem == 0 {
                    continue;
                }

                let mut after_draw = after_discard;
                after_draw.inc(draw_tt);

                for j in 0..34u8 {
                    let discard_tt = TileType(j);
                    if after_draw.get(discard_tt) == 0 {
                        continue;
                    }
                    let mut after_discard2 = after_draw;
                    after_discard2.dec(discard_tt);

                    let new_acceptance_copies = count_acceptance_copies_with_extra(
                        calculator,
                        &after_discard2,
                        visible,
                        new_shanten,
                        draw_tt,
                    );

                    if new_acceptance_copies > old_acceptance_copies {
                        improvement_types += 1;
                        improvement_copies += draw_rem;
                        break;
                    }
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
    calculator: &ShantenCalculator,
    hand: &[Tile],
    visible: &VisibleTiles,
) -> (Vec<AcceptanceInfo>, Vec<AcceptanceInfo>, i8) {
    let counts = TileCounts::from_tiles(hand);
    let current_shanten = calculator.lookup(&counts);

    let mut acceptance = Vec::new();

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
            acceptance.push(AcceptanceInfo {
                tile: tt,
                copies: rem,
                new_shanten,
            });
        }
    }

    let base_acceptance = count_acceptance_copies(calculator, &counts, visible, current_shanten);
    let mut improvement = Vec::new();

    for i in 0..34u8 {
        let draw_tt = TileType(i);
        let draw_rem = remaining_copies(draw_tt, &counts, visible);
        if draw_rem == 0 {
            continue;
        }

        let mut after_draw = counts;
        after_draw.inc(draw_tt);

        for j in 0..34u8 {
            let discard_tt = TileType(j);
            if after_draw.get(discard_tt) == 0 {
                continue;
            }
            let mut after_discard = after_draw;
            after_discard.dec(discard_tt);

            let new_acceptance = count_acceptance_copies_with_extra(
                calculator,
                &after_discard,
                visible,
                current_shanten,
                draw_tt,
            );
            if new_acceptance > base_acceptance {
                improvement.push(AcceptanceInfo {
                    tile: draw_tt,
                    copies: draw_rem,
                    new_shanten: current_shanten,
                });
                break;
            }
        }
    }

    (acceptance, improvement, current_shanten)
}

fn count_acceptance_copies(
    calculator: &ShantenCalculator,
    counts: &TileCounts,
    visible: &VisibleTiles,
    shanten: i8,
) -> usize {
    let mut total = 0usize;
    for i in 0..34u8 {
        let tt = TileType(i);
        if counts.get(tt) >= 4 {
            continue;
        }
        let mut after = *counts;
        after.inc(tt);
        if calculator.lookup(&after) < shanten {
            total += remaining_copies(tt, counts, visible);
        }
    }
    total
}

fn count_acceptance_copies_with_extra(
    calculator: &ShantenCalculator,
    counts: &TileCounts,
    visible: &VisibleTiles,
    shanten: i8,
    extra_tile: TileType,
) -> usize {
    let mut total = 0usize;
    for i in 0..34u8 {
        let tt = TileType(i);
        if counts.get(tt) >= 4 {
            continue;
        }
        let mut after = *counts;
        after.inc(tt);
        if calculator.lookup(&after) < shanten {
            let mut rem = remaining_copies(tt, counts, visible);
            if tt == extra_tile {
                rem = rem.saturating_sub(1);
            }
            total += rem;
        }
    }
    total
}

fn find_acceptance(
    calculator: &ShantenCalculator,
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
    remaining_copies_for(tt, hand_counts, visible)
}
