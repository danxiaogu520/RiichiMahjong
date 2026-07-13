use riichi_core::player::PlayerId;
use riichi_core::tile::Tile;
use riichi_logic::acceptance::VisibleTiles;
use riichi_logic::shanten::ShantenCalculator;

use crate::discard::choose_riichi_discard;

/// AI 决策：在合法立直宣言牌中按牌效选择一张。
pub fn decide_riichi(
    _player: PlayerId,
    calc: &mut ShantenCalculator,
    hand: &[Tile],
    visible: &VisibleTiles,
    tenpai_discards: &[Tile],
) -> Option<Tile> {
    choose_riichi_discard(calc, hand, visible, tenpai_discards)
}
