use riichi_core::meld::Meld;
use riichi_core::tile::{Tile, TileType};
use riichi_logic::shanten::ShantenCalculator;
use riichi_logic::types::TileCounts;

#[derive(Debug, Clone)]
pub struct DiscardAnalysis {
    pub tile: Tile,
    pub shanten: i8,
    pub effective_tiles: usize,
    pub effective_types: usize,
}

pub fn analyze_discards(
    hand_tiles: &[Tile],
    discards: &[Vec<Tile>; 4],
    melds: &[Vec<Meld>; 4],
    dora: &[TileType],
    pending_discard: Option<(riichi_core::player::PlayerId, Tile)>,
) -> Vec<DiscardAnalysis> {
    if hand_tiles.len() < 2 {
        return Vec::new();
    }
    let calculator = ShantenCalculator::new();
    let visible = visible_counts(hand_tiles, discards, melds, dora, pending_discard);
    let candidates: Vec<(Tile, Vec<Tile>, i8)> = hand_tiles
        .iter()
        .copied()
        .enumerate()
        .map(|(index, tile)| {
            let mut after = hand_tiles.to_vec();
            after.remove(index);
            let shanten = calculator.calculate(&after);
            (tile, after, shanten)
        })
        .collect();
    let Some(min_shanten) = candidates.iter().map(|(_, _, shanten)| *shanten).min() else {
        return Vec::new();
    };

    let mut result: Vec<_> = candidates
        .into_iter()
        .filter(|(_, _, shanten)| *shanten == min_shanten)
        .map(|(tile, after, shanten)| {
            let after_counts = TileCounts::from_tiles(&after);
            let mut effective_tiles = 0usize;
            let mut effective_types = 0usize;
            for index in 0..34u8 {
                let tile_type = TileType(index);
                if after_counts.get(tile_type) >= 4 {
                    continue;
                }
                let mut improved = after.clone();
                improved.push(tile_type.with_copy(0));
                if calculator.calculate(&improved) < shanten {
                    effective_types += 1;
                    effective_tiles += 4usize.saturating_sub(visible[index as usize] as usize);
                }
            }
            DiscardAnalysis {
                tile,
                shanten,
                effective_tiles,
                effective_types,
            }
        })
        .collect();
    result.sort_by_key(|analysis| {
        (
            std::cmp::Reverse(analysis.effective_tiles),
            std::cmp::Reverse(analysis.effective_types),
            analysis.tile.tile_type().0,
            analysis.tile.raw(),
        )
    });
    result
}

fn visible_counts(
    hand_tiles: &[Tile],
    discards: &[Vec<Tile>; 4],
    melds: &[Vec<Meld>; 4],
    dora: &[TileType],
    pending_discard: Option<(riichi_core::player::PlayerId, Tile)>,
) -> [u8; 34] {
    let mut counts = [0u8; 34];
    let add_tile = |counts: &mut [u8; 34], tile: Tile| {
        let index = tile.tile_type().0 as usize;
        counts[index] = counts[index].saturating_add(1);
    };
    for &tile in hand_tiles {
        add_tile(&mut counts, tile);
    }
    for player_discards in discards {
        for &tile in player_discards {
            add_tile(&mut counts, tile);
        }
    }
    for player_melds in melds {
        for meld in player_melds {
            for &tile in &meld.tiles {
                add_tile(&mut counts, tile);
            }
        }
    }
    for &tile_type in dora {
        counts[tile_type.0 as usize] = counts[tile_type.0 as usize].saturating_add(1);
    }
    if let Some((_, tile)) = pending_discard {
        add_tile(&mut counts, tile);
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::analyze_discards;
    use riichi_core::tile::Tile;

    #[test]
    fn keeps_only_minimum_shanten_discards_and_sorts_effective_tiles() {
        let hand = [0, 4, 8, 12, 16, 36, 40, 44, 72, 76, 80, 84, 88, 92]
            .into_iter()
            .map(Tile::from_raw)
            .collect::<Vec<_>>();
        let empty_discards = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let empty_melds = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let result = analyze_discards(&hand, &empty_discards, &empty_melds, &[], None);

        assert!(!result.is_empty());
        let minimum = result[0].shanten;
        assert!(result.iter().all(|analysis| analysis.shanten == minimum));
        assert!(result.windows(2).all(|pair| {
            (pair[0].effective_tiles, pair[0].effective_types)
                >= (pair[1].effective_tiles, pair[1].effective_types)
        }));
    }
}
