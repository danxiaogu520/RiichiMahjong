use riichi_core::meld::Meld;
use riichi_core::tile::{Tile, TileType};
use riichi_logic::acceptance::{analyze_discard, DiscardOption, VisibleTiles};
use riichi_logic::shanten::ShantenCalculator;

pub type DiscardAnalysis = DiscardOption;

pub fn analyze_discards(
    hand_tiles: &[Tile],
    discards: &[Vec<Tile>; 4],
    melds: &[Vec<Meld>; 4],
    dora: &[TileType],
    pending_discard: Option<(riichi_core::player::PlayerId, Tile)>,
) -> Vec<DiscardAnalysis> {
    if hand_tiles.len() != 14 {
        return Vec::new();
    }

    let player_melds = vec![melds[0]
        .iter()
        .flat_map(|meld| meld.tiles.clone())
        .collect()];
    let other_melds = melds[1..]
        .iter()
        .map(|melds| melds.iter().flat_map(|meld| meld.tiles.clone()).collect())
        .collect::<Vec<Vec<Tile>>>();
    let mut all_discards: Vec<Tile> = discards.iter().flatten().copied().collect();
    if let Some((_, tile)) = pending_discard {
        all_discards.push(tile);
    }
    let visible = VisibleTiles::from_data(&player_melds, &other_melds, &all_discards, dora);
    let mut calculator = ShantenCalculator::new();
    analyze_discard(&mut calculator, hand_tiles, &visible)
}

#[cfg(test)]
mod tests {
    use super::analyze_discards;
    use riichi_core::tile::Tile;

    #[test]
    fn keeps_minimum_shanten_and_sorts_by_acceptance_then_improvement() {
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
            (pair[0].acceptance_copies, pair[0].improvement_copies)
                >= (pair[1].acceptance_copies, pair[1].improvement_copies)
        }));
    }
}
