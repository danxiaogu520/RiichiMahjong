use std::collections::HashSet;

use riichi_core::game::GameEvent;
use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use riichi_logic::analysis::analyze_wait_tiles;

use crate::game::{GamePhase, GameState};

impl GameState {
    /// 推进到下一个玩家
    pub fn advance_turn(&mut self) {
        self.current_player = self.current_player.next();
    }

    /// 判断指定的牌是否来自岭上（wall[132..=135]）
    pub fn is_rinshan_tile(&self, tile: Tile) -> bool {
        self.wall.is_rinshan_tile(tile)
    }

    /// 获取玩家的听牌类型集合（内部使用）
    pub(crate) fn get_waiting_tile_types(&self, player: PlayerId) -> HashSet<TileType> {
        analyze_wait_tiles(self.players[player.0].hand.tiles())
            .iter()
            .map(|w| w.tile_type)
            .collect()
    }

    /// 更新单个玩家的舍牌振听状态
    ///
    /// 如果玩家听的牌在其舍牌中出现，则进入振听状态
    pub(crate) fn update_discard_furiten(&mut self, player: PlayerId) {
        let waiting = self.get_waiting_tile_types(player);
        let discarded = &self.players[player.0].all_discarded_types;
        self.players[player.0].furiten.discard = waiting.iter().any(|tt| discarded.contains(tt));
    }

    /// 更新所有玩家的舍牌振听状态
    pub(crate) fn update_all_discard_furiten(&mut self) {
        for idx in 0..4 {
            self.update_discard_furiten(PlayerId(idx));
        }
    }

    /// 判断当前局是否已结束
    pub fn is_round_over(&self) -> bool {
        matches!(self.phase, GamePhase::RoundOver) || self.remaining_tiles() == 0
    }

    /// 正常摸牌区剩余可摸牌数
    pub fn remaining_tiles(&self) -> usize {
        self.wall.remaining()
    }

    /// 取出事件列表（消耗性读取）
    pub fn take_events(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.events)
    }

    /// 构建指定玩家视角的 VisibleTiles（用于向听/进张分析）
    ///
    /// 包含：
    /// - 当前玩家的副露牌
    /// - 其他玩家的副露牌
    /// - 所有玩家的舍牌
    /// - 宝牌指示牌
    pub fn build_visible_tiles(&self, player: PlayerId) -> riichi_logic::acceptance::VisibleTiles {
        let mut visible = riichi_logic::acceptance::VisibleTiles::new();

        // 当前玩家的副露牌
        for meld in &self.players[player.0].melds {
            for t in &meld.tiles {
                visible.hand_melds.inc(t.tile_type());
            }
        }

        // 其他玩家的副露牌
        for i in 0..4 {
            let pid = PlayerId(i);
            if pid == player {
                continue;
            }
            for meld in &self.players[i].melds {
                for t in &meld.tiles {
                    visible.all_melds.inc(t.tile_type());
                }
            }
        }

        // 所有玩家的舍牌
        for i in 0..4 {
            for &t in &self.players[i].discards {
                visible.all_discards.inc(t.tile_type());
            }
        }

        // 宝牌指示牌
        for &tt in &self.dora_indicators {
            visible.dora_indicators.inc(tt);
        }

        visible
    }
}
