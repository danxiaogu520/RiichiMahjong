use rand::seq::SliceRandom;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::tile::Tile;

/// 正常摸牌区上界（王牌区从这里开始）
pub const DEAD_WALL_START: usize = 122;

/// 初始宝牌指示牌位置
pub const DORA_INDICATOR_START: usize = 131;

/// 岭上牌起始位置（最大杠数时）
pub const RINSHAN_START: usize = 135;

/// 牌山：136 张牌的固定数组，通过指针索引不同区域。
///
/// ```text
/// 索引:  0                    122  123  124  125  126  127  128  129  130  131  132  133  134  135
///        |←— 正常摸牌区 ——→|←— 王牌区（里宝/宝牌/岭上）——————————————————————→|
/// ```
/// 宝牌指示牌: 131(初始), 130, 129, 128, 127 (杠后追加)
/// 里宝牌指示牌: 126(初始), 125, 124, 123, 122 (杠后追加)
/// 岭上牌: 135, 134, 133, 132 (按杠顺序取用)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wall {
    tiles: Vec<Tile>,
    draw_index: usize,
    /// 正常摸牌区的独占上界。初始为 122，每次成功开杠后减少一张，
    /// 以保持王牌始终包含 14 张牌。
    live_end: usize,
    kan_count: usize,
}

impl Wall {
    /// 创建新牌山并洗牌。
    pub fn new(rng: &mut impl Rng) -> Self {
        let mut tiles = Tile::all_tiles();
        tiles.shuffle(rng);
        Self {
            tiles,
            draw_index: 0,
            live_end: DEAD_WALL_START,
            kan_count: 0,
        }
    }

    /// 创建空牌山（占位，需在 start_round 中替换）。
    pub fn empty() -> Self {
        Self {
            tiles: Vec::new(),
            draw_index: 0,
            live_end: DEAD_WALL_START,
            kan_count: 0,
        }
    }

    /// 从正常摸牌区摸一张牌。耗尽时返回 None。
    pub fn draw(&mut self) -> Option<Tile> {
        if self.draw_index >= self.live_end || self.draw_index >= self.tiles.len() {
            return None;
        }
        let tile = self.tiles[self.draw_index];
        self.draw_index += 1;
        Some(tile)
    }

    /// 正常摸牌区剩余可摸牌数。
    pub fn remaining(&self) -> usize {
        self.live_end.saturating_sub(self.draw_index)
    }

    /// 从岭上区摸一张牌（杠后补摸）。
    pub fn draw_rinshan(&mut self) -> Option<Tile> {
        if self.kan_count >= 4 {
            return None;
        }
        let index = RINSHAN_START - self.kan_count;
        if self.draw_index >= self.live_end || index >= self.tiles.len() {
            return None;
        }
        let tile = self.tiles[index];
        self.kan_count += 1;
        // 牌墙末端的一张正常摸牌牌进入扩展后的王牌区域。由于牌山已
        // 洗牌，逻辑上只需收缩 live_end；该牌不再从正常摸牌区取出。
        self.live_end -= 1;
        Some(tile)
    }

    /// 获取第 i 张宝牌指示牌（0 = 初始 131，1 = 第一次杠后 130，以此类推）。
    pub fn dora_indicator(&self, i: usize) -> Tile {
        self.tiles[DORA_INDICATOR_START - i]
    }

    /// 获取第 i 张里宝牌指示牌（0 = 初始 126，1 = 第一次杠后 125，以此类推）。
    pub fn ura_dora_indicator(&self, i: usize) -> Tile {
        self.tiles[DORA_INDICATOR_START - 5 - i]
    }

    /// 当前已翻开的宝牌指示牌数（初始 1 张 + 每次杠追加 1 张，最多 5 张）。
    pub fn current_dora_count(&self) -> usize {
        (1 + self.kan_count).min(5)
    }

    /// 已开杠次数。
    pub fn kan_count(&self) -> usize {
        self.kan_count
    }

    /// 获取全部牌（用于游戏状态保存）。
    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }

    /// 当前正常摸牌指针。
    pub fn draw_index(&self) -> usize {
        self.draw_index
    }

    /// 当前正常摸牌区的独占上界。
    pub fn live_end(&self) -> usize {
        self.live_end
    }

    /// 判断一张牌是否来自岭上区
    pub fn is_rinshan_tile(&self, tile: Tile) -> bool {
        (RINSHAN_START - 3..=RINSHAN_START).any(|i| self.tiles[i] == tile)
    }
}

#[cfg(test)]
mod tests {
    use super::Wall;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn kan_reduces_live_wall_by_one() {
        let mut rng = StdRng::seed_from_u64(7);
        let mut wall = Wall::new(&mut rng);
        for _ in 0..10 {
            assert!(wall.draw().is_some());
        }
        let before = wall.remaining();
        assert!(wall.draw_rinshan().is_some());
        assert_eq!(wall.remaining(), before - 1);
        assert_eq!(wall.live_end(), 121);
    }

    #[test]
    fn only_four_rinshan_tiles_can_be_drawn() {
        let mut rng = StdRng::seed_from_u64(11);
        let mut wall = Wall::new(&mut rng);
        for _ in 0..4 {
            assert!(wall.draw_rinshan().is_some());
        }
        assert!(wall.draw_rinshan().is_none());
        assert_eq!(wall.kan_count(), 4);
    }
}
