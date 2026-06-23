use riichi_core::game_types::GameEvent;
use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use riichi_logic::analysis::{analyze_wait_tiles, is_standard_win};
use riichi_logic::shanten::ShantenCalculator;
use riichi_logic::types::TileCounts;

use crate::game::{GameError, GameState};

impl GameState {
    /// 获取玩家的听牌列表（手牌 13 张时调用）
    ///
    /// 返回所有能和的牌类型
    pub fn get_waiting_tiles(&self, player: PlayerId) -> Vec<TileType> {
        analyze_wait_tiles(self.players[player.0].hand.tiles())
            .iter()
            .map(|w| w.tile_type)
            .collect()
    }

    /// 检测玩家是否可以宣告立直
    ///
    /// 立直条件：
    /// 1. 尚未立直
    /// 2. 门前清（无副露）
    /// 3. 点数 >= 1000
    /// 4. 剩余牌 >= 4
    /// 5. 打出任意一张后能听牌（向听数 = 0）
    pub fn can_declare_riichi(&self, player: PlayerId) -> bool {
        let p = &self.players[player.0];
        if p.is_riichi {
            return false; // 已经立直
        }
        if !p.is_menzen() {
            return false; // 非门前清
        }
        if p.points < 1000 {
            return false; // 点数不足
        }
        if self.remaining_tiles() < 4 {
            return false; // 剩余牌不足
        }
        // 检查是否存在一张牌打出后能听牌
        let calc = ShantenCalculator::new();
        let mut tiles: Vec<Tile> = self.players[player.0].hand.tiles().to_vec();
        if let Some(t) = self.drawn_tile {
            tiles.push(t);
        }
        let counts = TileCounts::from_tiles(&tiles);
        tiles.iter().any(|tile| {
            let mut after = counts;
            after.dec(tile.tile_type());
            calc.lookup(&after) == 0
        })
    }

    /// 宣告立直（仅宣告，不打牌）
    ///
    /// 扣除 1000 点，设置立直标记，放置立直棒
    pub fn execute_riichi(&mut self, player: PlayerId) -> Result<(), GameError> {
        if !self.can_declare_riichi(player) {
            return Err(GameError::InvalidAction("不满足立直条件".to_string()));
        }
        let p = &mut self.players[player.0];
        p.points -= 1000;
        p.is_riichi = true;
        self.riichi_sticks += 1;
        self.events.push(GameEvent::PlayerDeclaredRiichi { player });
        Ok(())
    }

    /// 立直后可用的暗杠选项
    ///
    /// 立直后暗杠必须满足：
    /// 1. 暗杠的 4 张牌包含摸到的牌
    /// 2. 暗杠后听牌种类不变
    ///
    /// 实现：比较暗杠前（13张手牌）的听牌与暗杠后（10张手牌）的听牌
    pub fn get_riichi_ankan_options(&self, player: PlayerId) -> Vec<Tile> {
        let p = &self.players[player.0];
        if !p.is_riichi {
            return vec![];
        }
        let drawn = match self.drawn_tile {
            Some(t) => t,
            None => return vec![],
        };
        let drawn_tt = drawn.tile_type();

        let hand = &p.hand;
        let hand_count = hand.count_type(drawn_tt.0);

        // 必须手牌 3 张 + drawn_tile 1 张 = 4 张
        if hand_count != 3 {
            return vec![];
        }

        // waits_before：13 张手牌的听牌
        let waits_before: std::collections::HashSet<TileType> = analyze_wait_tiles(hand.tiles())
            .iter()
            .map(|w| w.tile_type)
            .collect();

        if waits_before.is_empty() {
            return vec![];
        }

        // 模拟暗杠后的 10 张手牌
        let mut hand_after = hand.clone();
        let tiles_to_remove: Vec<Tile> = hand
            .tiles()
            .iter()
            .filter(|t| t.tile_type() == drawn_tt)
            .take(3)
            .copied()
            .collect();
        for t in &tiles_to_remove {
            hand_after.remove(*t).ok();
        }

        // waits_after：逐一尝试添加每种牌型，检查是否构成和了形
        let base_counts = TileCounts::from_tiles(hand_after.tiles());
        let waits_after: std::collections::HashSet<TileType> = (0..34u8)
            .map(TileType)
            .filter(|&tt| {
                if base_counts.get(tt) >= 4 {
                    return false;
                }
                let mut counts = base_counts;
                counts.inc(tt);
                is_standard_win(&mut counts)
            })
            .collect();

        // 听牌种类不变才允许暗杠
        if waits_before == waits_after {
            vec![drawn]
        } else {
            vec![]
        }
    }
}
