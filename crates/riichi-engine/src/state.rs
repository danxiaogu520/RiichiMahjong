use std::collections::HashSet;

use crate::game::{TenpaiInfo, WaitInfo};
use riichi_core::game::GameEvent;
use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use riichi_logic::acceptance::remaining_copies_for;
use riichi_logic::analysis::analyze_wait_tiles_with_open_melds;
use riichi_logic::types::TileCounts;

use crate::game::{GamePhase, GameState};

impl GameState {
    /// 构建指定玩家的听牌展示信息。规则判断全部委托给核心逻辑。
    pub fn tenpai_info(&self, player: PlayerId) -> Option<TenpaiInfo> {
        let p = &self.players[player.0];
        let waits = analyze_wait_tiles_with_open_melds(p.hand.tiles(), p.melds.len());
        if waits.is_empty() {
            return None;
        }

        let mut visible = self.build_visible_tiles(player);
        if self.current_player == player {
            if let Some(drawn) = self.drawn_tile {
                visible.all_discards.inc(drawn.tile_type());
            }
        }
        let hand_counts = TileCounts::from_tiles(p.hand.tiles());
        let is_furiten = p.furiten.is_furiten();
        let waits = waits
            .into_iter()
            .map(|wait| {
                let tile_type = wait.tile_type;
                WaitInfo {
                    tile_type,
                    remaining: remaining_copies_for(tile_type, &hand_counts, &visible),
                    is_no_yaku: !self.wait_has_yaku(player, tile_type),
                }
            })
            .collect();

        Some(TenpaiInfo { waits, is_furiten })
    }

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
        let p = &self.players[player.0];
        analyze_wait_tiles_with_open_melds(p.hand.tiles(), p.melds.len())
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

    /// 记录一个同时属于当前局和整场历史的事件。
    pub(crate) fn record_event(&mut self, event: GameEvent) {
        self.events.push(event.clone());
        self.history.push(event);
    }

    /// 记录一批同时属于当前局和整场历史的事件。
    pub(crate) fn record_events(&mut self, events: &[GameEvent]) {
        self.events.extend(events.iter().cloned());
        self.history.extend(events.iter().cloned());
    }

    /// 获取整场追加式事件历史。
    pub fn event_history(&self) -> &[GameEvent] {
        &self.history
    }

    /// 创建可交给重连客户端的权威状态快照。
    pub fn snapshot(&self) -> Self {
        self.clone()
    }

    /// 从权威快照恢复牌局状态。
    pub fn from_snapshot(snapshot: Self) -> Self {
        snapshot
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
