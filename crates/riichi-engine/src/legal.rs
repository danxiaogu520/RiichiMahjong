use riichi_core::game::{CallType, ResponseAction, TurnAction};
use riichi_core::player::PlayerId;

use crate::game::{GamePhase, GameState};

/// 服务器对外暴露的动作候选。
///
/// 这是从“客户端直接提交任意动作”迁移到“服务端生成候选并校验动作”的
/// 第一层兼容接口。后续应将 `TurnAction` / `ResponseAction` 替换为包含
/// 完整实体牌参数的 `LegalAction`。
#[derive(Debug, Clone)]
pub enum LegalAction {
    Turn(TurnAction),
    Response(ResponseAction),
}

impl GameState {
    /// 返回指定玩家当前可以尝试的动作。
    ///
    /// 调用方仍必须在提交时再次验证，候选列表不能替代服务器校验。
    pub fn legal_actions(&self, player: PlayerId) -> Vec<LegalAction> {
        match &self.phase {
            GamePhase::ActionPhase if player == self.current_player => {
                let mut actions = Vec::new();
                for &tile in self.players[player.0].hand.tiles() {
                    actions.push(LegalAction::Turn(TurnAction::Discard(tile)));
                }
                if let Some(tile) = self.drawn_tile {
                    actions.push(LegalAction::Turn(TurnAction::Discard(tile)));
                }
                if self.can_declare_riichi(player) {
                    for &tile in self.players[player.0].hand.tiles() {
                        actions.push(LegalAction::Turn(TurnAction::RiichiDiscard(tile)));
                    }
                    if let Some(tile) = self.drawn_tile {
                        actions.push(LegalAction::Turn(TurnAction::RiichiDiscard(tile)));
                    }
                }
                if self.check_tsumo(player).is_some() {
                    actions.push(LegalAction::Turn(TurnAction::Tsumo));
                }
                for tile in self.get_ankan_options(player) {
                    actions.push(LegalAction::Turn(TurnAction::Ankan(tile)));
                }
                for (index, tile) in self.get_kakan_options(player) {
                    actions.push(LegalAction::Turn(TurnAction::Kakan(index, tile)));
                }
                if self.can_declare_kyuushu(player) {
                    actions.push(LegalAction::Turn(TurnAction::KyuushuKyuuhai));
                }
                actions
            }
            GamePhase::ResponsePhase { .. } | GamePhase::ChankanResponse { .. } => self
                .get_call_options()
                .into_iter()
                .filter(|option| option.player == player)
                .map(|option| {
                    let response = match option.call_type {
                        CallType::Ron => ResponseAction::Ron,
                        CallType::Minkan { hand_tiles } => {
                            ResponseAction::Minkan { hand_tiles }
                        }
                        CallType::Pon { hand_tiles } => ResponseAction::Pon { hand_tiles },
                        CallType::Chi { hand_tiles } => ResponseAction::Chi { hand_tiles },
                    };
                    LegalAction::Response(response)
                })
                .chain(std::iter::once(LegalAction::Response(ResponseAction::Pass)))
                .collect(),
            _ => Vec::new(),
        }
    }
}
