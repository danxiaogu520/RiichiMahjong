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

    /// 校验动作是否仍然适用于当前状态。
    ///
    /// 候选动作只用于展示和决策，网络层提交动作时必须再次调用本方法。
    pub fn validate_action(
        &self,
        player: PlayerId,
        action: &LegalAction,
    ) -> Result<(), riichi_core::game::GameError> {
        match action {
            LegalAction::Turn(turn) => {
                if !matches!(&self.phase, GamePhase::ActionPhase)
                    || player != self.current_player
                {
                    return Err(riichi_core::game::GameError::InvalidAction(
                        "当前不是该玩家的行动阶段".to_string(),
                    ));
                }
                match turn {
                    TurnAction::Discard(tile) => {
                        if self.players[player.0].is_riichi
                            && self.drawn_tile != Some(*tile)
                        {
                            return Err(riichi_core::game::GameError::InvalidAction(
                                "立直后只能摸切".to_string(),
                            ));
                        }
                        if self.drawn_tile != Some(*tile)
                            && !self.players[player.0].hand.contains(*tile)
                        {
                            return Err(riichi_core::game::GameError::TileNotInHand(*tile));
                        }
                    }
                    TurnAction::RiichiDiscard(tile) => {
                        if !self.can_declare_riichi(player)
                            || (self.drawn_tile != Some(*tile)
                                && !self.players[player.0].hand.contains(*tile))
                        {
                            return Err(riichi_core::game::GameError::InvalidAction(
                                "不满足立直宣言条件".to_string(),
                            ));
                        }
                    }
                    TurnAction::Tsumo => {
                        if self.check_tsumo(player).is_none() {
                            return Err(riichi_core::game::GameError::InvalidAction(
                                "当前不能自摸".to_string(),
                            ));
                        }
                    }
                    TurnAction::Ankan(tile) => {
                        if !self
                            .get_ankan_options(player)
                            .iter()
                            .any(|candidate| candidate.tile_type() == tile.tile_type())
                        {
                            return Err(riichi_core::game::GameError::InvalidAction(
                                "当前不能暗杠该牌".to_string(),
                            ));
                        }
                    }
                    TurnAction::Kakan(index, tile) => {
                        if !self
                            .get_kakan_options(player)
                            .contains(&(*index, *tile))
                        {
                            return Err(riichi_core::game::GameError::InvalidAction(
                                "当前不能加杠该牌".to_string(),
                            ));
                        }
                    }
                    TurnAction::KyuushuKyuuhai => {
                        if !self.can_declare_kyuushu(player) {
                            return Err(riichi_core::game::GameError::InvalidAction(
                                "当前不能宣告九种九牌".to_string(),
                            ));
                        }
                    }
                }
            }
            LegalAction::Response(response) => {
                let discarder = match &self.phase {
                    GamePhase::ResponsePhase {
                        discarder,
                        ..
                    } => *discarder,
                    GamePhase::ChankanResponse {
                        kakan_player,
                        ..
                    } => *kakan_player,
                    _ => {
                        return Err(riichi_core::game::GameError::InvalidAction(
                            "当前不在响应阶段".to_string(),
                        ));
                    }
                };
                if player == discarder {
                    return Err(riichi_core::game::GameError::InvalidAction(
                        "不能响应自己的牌".to_string(),
                    ));
                }
                if matches!(response, ResponseAction::Pass) {
                    return Ok(());
                }
                let options = self.get_call_options();
                let valid = options.iter().any(|option| {
                    if option.player != player {
                        return false;
                    }
                    match (&option.call_type, response) {
                        (CallType::Ron, ResponseAction::Ron) => true,
                        (
                            CallType::Minkan { hand_tiles: expected },
                            ResponseAction::Minkan { hand_tiles },
                        ) => expected == hand_tiles,
                        (
                            CallType::Pon { hand_tiles: expected },
                            ResponseAction::Pon { hand_tiles },
                        ) => expected == hand_tiles,
                        (
                            CallType::Chi { hand_tiles: expected },
                            ResponseAction::Chi { hand_tiles },
                        ) => expected == hand_tiles,
                        _ => false,
                    }
                });
                if !valid {
                    return Err(riichi_core::game::GameError::InvalidAction(
                        format!("玩家 {} 当前不能执行该响应", player),
                    ));
                }
            }
        }
        Ok(())
    }
}
