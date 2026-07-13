use riichi_core::tile::Tile;
use riichi_logic::acceptance::{analyze_discard, DiscardOption, VisibleTiles};
use riichi_logic::shanten::ShantenCalculator;

/// AI 选择打牌：按向听数/进张/改良排序，平手时优先打出孤立牌。
pub fn choose_discard(
    calc: &mut ShantenCalculator,
    hand: &[Tile],
    visible: &VisibleTiles,
) -> Option<DiscardOption> {
    choose_from_analysis(&analyze_discard(calc, hand, visible))
}

/// 从合法立直宣言牌中按同样的牌效规则选择一张。
pub fn choose_riichi_discard(
    calc: &mut ShantenCalculator,
    hand: &[Tile],
    visible: &VisibleTiles,
    legal_discards: &[Tile],
) -> Option<Tile> {
    let analysis = analyze_discard(calc, hand, visible);
    let best = analysis
        .iter()
        .filter(|option| {
            legal_discards
                .iter()
                .any(|tile| tile.tile_type() == option.tile.tile_type())
        })
        .cloned()
        .collect::<Vec<_>>();

    choose_from_analysis(&best).and_then(|option| {
        legal_discards
            .iter()
            .copied()
            .find(|tile| tile.tile_type() == option.tile.tile_type())
    })
}

fn choose_from_analysis(analysis: &[DiscardOption]) -> Option<DiscardOption> {
    analysis.iter().cloned().max_by(|a, b| {
        b.shanten
            .cmp(&a.shanten)
            .then(a.acceptance_copies.cmp(&b.acceptance_copies))
            .then(a.improvement_copies.cmp(&b.improvement_copies))
            .then(discard_priority(a.tile).cmp(&discard_priority(b.tile)))
    })
}

/// 平手时的舍牌优先级：字牌 > 1/9 > 2/8 > 3～7。
fn discard_priority(tile: Tile) -> u8 {
    let tile_type = tile.tile_type();
    if tile_type.is_honor() {
        3
    } else if tile_type.is_terminal() {
        2
    } else if matches!(tile_type.rank().0, 2 | 8) {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::discard_priority;
    use riichi_core::tile::{Tile, TileType};

    #[test]
    fn discard_priority_matches_requested_order() {
        assert!(
            discard_priority(Tile::from_type_index(27, 0))
                > discard_priority(Tile::from_type_index(0, 0))
        );
        assert!(
            discard_priority(Tile::from_type_index(0, 0))
                > discard_priority(Tile::from_type_index(1, 0))
        );
        assert!(
            discard_priority(Tile::from_type_index(1, 0))
                > discard_priority(Tile::from_type_index(2, 0))
        );
        assert_eq!(TileType::EAST, Tile::from_type_index(27, 0).tile_type());
    }
}
