use std::collections::HashSet;

use riichi_core::game_types::GameEvent;
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
        if self.current_player.0 != player.0 {
            return false;
        }
        // 检查是否有任何鸣牌
        if self.events.iter().any(|e| {
            matches!(
                e,
                GameEvent::PlayerCalledPon { .. }
                    | GameEvent::PlayerCalledChi { .. }
                    | GameEvent::PlayerCalledMinkan { .. }
                    | GameEvent::PlayerCalledAnkan { .. }
                    | GameEvent::PlayerCalledKakan { .. }
            )
        }) {
            return false;
        }
        // 检查是否是首巡状态
        if self
            .events
            .iter()
            .any(|e| matches!(e, GameEvent::PlayerDiscarded { player:p, .. }if p.0 != player.0))
        {
            return false;
        }
        // 检查手牌中是否有九种不同的幺九牌
        let hand = &self.players[player.0].hand;
        let tile_count = hand.len() + if self.drawn_tile.is_some() { 1 } else { 0 };
        if tile_count != 14 {
            return false;
        }
        let mut types = HashSet::new();
        for &tile in hand.tiles() {
            if tile.is_yaochuuhai() {
                types.insert(tile.tile_type());
            }
        }
        if let Some(drawn) = self.drawn_tile {
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
        if self.events.iter().any(|e| {
            matches!(
                e,
                GameEvent::PlayerCalledPon { .. }
                    | GameEvent::PlayerCalledChi { .. }
                    | GameEvent::PlayerCalledMinkan { .. }
                    | GameEvent::PlayerCalledAnkan { .. }
                    | GameEvent::PlayerCalledKakan { .. }
            )
        }) {
            return false;
        }
        // 检查是否是首巡状态
        if self
            .events
            .iter()
            .filter(|e| matches!(e, GameEvent::PlayerDiscarded { .. }))
            .count()
            != 4
        {
            return false;
        }
        // 检查四位玩家的首巡弃牌是否都是同一风牌
        let mut first = [None::<TileType>; 4];
        for e in &self.events {
            if let GameEvent::PlayerDiscarded { player, tile } = e {
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
            .filter(|e| matches!(e, GameEvent::PlayerDeclaredRiichi { .. }))
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
                    GameEvent::PlayerCalledMinkan { .. }
                        | GameEvent::PlayerCalledAnkan { .. }
                        | GameEvent::PlayerCalledKakan { .. }
                )
            })
            .map(|e| match e {
                GameEvent::PlayerCalledMinkan { player, .. }
                | GameEvent::PlayerCalledAnkan { player, .. }
                | GameEvent::PlayerCalledKakan { player, .. } => player,
                _ => unreachable!(),
            })
            .collect::<HashSet<&PlayerId>>()
            .len()
            >= 2
    }
}
