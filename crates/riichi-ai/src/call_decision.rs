use riichi_core::game_types::{CallOption, ResponseAction};
use riichi_core::player::PlayerId;

/// AI 决策：是否响应他人的打出牌
/// 当前简单实现：一律 Pass（不副露）
pub fn decide_call(
    _player: PlayerId,
    _options: &[CallOption],
) -> Option<ResponseAction> {
    Some(ResponseAction::Pass)
}
