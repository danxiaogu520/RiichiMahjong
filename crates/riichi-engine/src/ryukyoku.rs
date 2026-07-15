use std::collections::HashSet;

use riichi_core::game::{CallKind, GameEvent};
use riichi_core::player::PlayerId;
use riichi_core::tile::TileType;

use crate::game::GameState;

impl GameState {
    /// 检查是否满足九种九牌的条件
    ///
    /// 条件：
    /// 1. 本局没有任何鸣牌（吃/碰/杠）
    /// 2. 玩家处于首巡状态
    /// 3. 手牌中有九种不同的幺九牌（包括手牌和摸到的牌）
    pub fn can_declare_kyuushu(&self, player: PlayerId) -> bool {
        // 检查当前是否是该玩家的回合
        if self.current_player() != Some(player) {
            return false;
        }
        // 检查是否有任何鸣牌
        if self
            .events
            .iter()
            .any(|e| matches!(e, GameEvent::Call { .. }))
        {
            return false;
        }
        // 检查是否是首巡状态
        if self.events.iter().any(|e| {
            matches!(
                e,
                GameEvent::Discard {
                    player: discarded_player,
                    ..
                } if *discarded_player == player
            )
        }) {
            return false;
        }
        // 检查手牌中是否有九种不同的幺九牌
        let hand = &self.players[player.0].hand;
        let tile_count = hand.len() + if self.drawn_tile().is_some() { 1 } else { 0 };
        if tile_count != 14 {
            return false;
        }
        let mut types = HashSet::new();
        for &tile in hand.tiles() {
            if tile.is_yaochuuhai() {
                types.insert(tile.tile_type());
            }
        }
        if let Some(drawn) = self.drawn_tile() {
            if drawn.is_yaochuuhai() {
                types.insert(drawn.tile_type());
            }
        }
        types.len() >= 9
    }

    /// 检查是否满足四风连打的条件
    ///
    /// 条件：
    /// 1. 首巡状态下，四位玩家的首巡弃牌都是同一风牌
    /// 2. 没有任何鸣牌
    pub fn check_suufon_renda(&self) -> bool {
        // 检查是否有任何鸣牌
        if self
            .events
            .iter()
            .any(|e| matches!(e, GameEvent::Call { .. }))
        {
            return false;
        }
        // 检查是否是首巡状态
        if self
            .events
            .iter()
            .filter(|e| matches!(e, GameEvent::Discard { .. }))
            .count()
            != 4
        {
            return false;
        }
        // 检查四位玩家的首巡弃牌是否都是同一风牌
        let mut first = [None::<TileType>; 4];
        for e in &self.events {
            if let GameEvent::Discard { player, tile, .. } = e {
                if first[player.0].is_none() {
                    first[player.0] = Some(tile.tile_type());
                }
            }
        }
        let discards: Vec<TileType> = first.iter().filter_map(|o| *o).collect();
        if discards.len() < 4 {
            return false;
        }
        let first = discards[0];
        if !first.is_wind() {
            return false;
        }
        discards.iter().all(|&d| d == first)
    }

    /// 检查是否满足四家立直的条件
    ///
    /// 条件：至少四位玩家宣布立直
    pub fn check_suucha_riichi(&self) -> bool {
        self.events
            .iter()
            .filter(|e| matches!(e, GameEvent::Riichi { .. }))
            .count()
            >= 4
    }

    /// 检查是否满足四杠散了的条件
    ///
    /// 条件：
    /// 1. 总杠数 >= 4
    /// 2. 至少有两位玩家开杠
    pub fn check_four_kan_abort(&self) -> bool {
        // 检查总开杠数是否达到四个
        let total = self.get_kan_count();
        if total < 4 {
            return false;
        }
        // 检查是否至少有两位玩家开杠
        self.events
            .iter()
            .filter(|e| {
                matches!(
                    e,
                    GameEvent::Call {
                        kind: CallKind::Minkan | CallKind::Ankan | CallKind::Kakan,
                        ..
                    }
                )
            })
            .map(|e| match e {
                GameEvent::Call {
                    player,
                    kind: CallKind::Minkan | CallKind::Ankan | CallKind::Kakan,
                    ..
                } => player,
                _ => unreachable!(),
            })
            .collect::<HashSet<&PlayerId>>()
            .len()
            >= 2
    }
}

#[cfg(test)]
mod tests {
    use super::GameState;
    use riichi_core::game::GameEvent;
    use riichi_core::player::PlayerId;
    use riichi_core::tile::Tile;

    #[test]
    fn kyuushu_is_available_before_this_players_first_discard() {
        let mut state = GameState::new();
        state.phase = riichi_core::game::GamePhase::ActionPhase {
            player: PlayerId(1),
            drawn_tile: None,
        };
        state.players[1].hand = riichi_core::hand::Hand::from_tiles(&[
            Tile::from_raw(0),
            Tile::from_raw(32),
            Tile::from_raw(36),
            Tile::from_raw(68),
            Tile::from_raw(72),
            Tile::from_raw(104),
            Tile::from_raw(108),
            Tile::from_raw(112),
            Tile::from_raw(116),
            Tile::from_raw(120),
            Tile::from_raw(124),
            Tile::from_raw(128),
            Tile::from_raw(132),
        ]);
        state.phase = riichi_core::game::GamePhase::ActionPhase {
            player: PlayerId(1),
            drawn_tile: Some(Tile::from_raw(4)),
        };
        state.events.push(GameEvent::Discard {
            player: PlayerId(0),
            tile: Tile::from_raw(4),
            kind: riichi_core::game::DiscardKind::Tedashi,
        });

        assert!(state.can_declare_kyuushu(PlayerId(1)));
    }

    #[test]
    fn kyuushu_is_unavailable_after_this_players_first_discard() {
        let mut state = GameState::new();
        state.phase = riichi_core::game::GamePhase::ActionPhase {
            player: PlayerId(1),
            drawn_tile: None,
        };
        state.players[1].hand = riichi_core::hand::Hand::from_tiles(&[
            Tile::from_raw(0),
            Tile::from_raw(32),
            Tile::from_raw(36),
            Tile::from_raw(68),
            Tile::from_raw(72),
            Tile::from_raw(104),
            Tile::from_raw(108),
            Tile::from_raw(112),
            Tile::from_raw(116),
            Tile::from_raw(120),
            Tile::from_raw(124),
            Tile::from_raw(128),
            Tile::from_raw(132),
        ]);
        state.phase = riichi_core::game::GamePhase::ActionPhase {
            player: PlayerId(1),
            drawn_tile: Some(Tile::from_raw(4)),
        };
        state.events.push(GameEvent::Discard {
            player: PlayerId(1),
            tile: Tile::from_raw(4),
            kind: riichi_core::game::DiscardKind::Tedashi,
        });

        assert!(!state.can_declare_kyuushu(PlayerId(1)));
    }
}
