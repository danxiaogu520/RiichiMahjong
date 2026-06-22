use riichi_core::player::PlayerId;
use riichi_core::tile::Tile;

/// AI 决策：是否立直 + 选择宣言牌
/// 当前简单实现：如果有可立直的打牌选项，选第一个
pub fn decide_riichi(_player: PlayerId, tenpai_discards: &[Tile]) -> Option<Tile> {
    tenpai_discards.first().copied()
}
