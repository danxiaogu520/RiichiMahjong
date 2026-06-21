use riichi_core::tile::Tile;
use riichi_logic::acceptance::{analyze_discard, DiscardOption, VisibleTiles};
use riichi_logic::shanten::ShantenCalculator;

/// AI 选择打牌：按向听数/进张/改良排序，选最优
pub fn choose_discard(
    calc: &mut ShantenCalculator,
    hand: &[Tile],
    visible: &VisibleTiles,
) -> Option<DiscardOption> {
    let analysis = analyze_discard(calc, hand, visible);
    analysis.into_iter().next()
}
