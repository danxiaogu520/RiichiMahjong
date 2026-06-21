use rand::Rng;
use rand::seq::SliceRandom;
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
            kan_count: 0,
        }
    }

    /// 创建空牌山（占位，需在 start_round 中替换）。
    pub fn empty() -> Self {
        Self {
            tiles: Vec::new(),
            draw_index: 0,
            kan_count: 0,
        }
    }

    /// 从正常摸牌区摸一张牌。耗尽时返回 None。
    pub fn draw(&mut self) -> Option<Tile> {
        if self.draw_index >= DEAD_WALL_START {
            return None;
        }
        let tile = self.tiles[self.draw_index];
        self.draw_index += 1;
        Some(tile)
    }

    /// 正常摸牌区剩余可摸牌数。
    pub fn remaining(&self) -> usize {
        DEAD_WALL_START.saturating_sub(self.draw_index)
    }

    /// 从岭上区摸一张牌（杠后补摸）。
    pub fn draw_rinshan(&mut self) -> Option<Tile> {
        if self.kan_count >= 4 {
            return None;
        }
        let index = RINSHAN_START - self.kan_count;
        let tile = self.tiles[index];
        self.kan_count += 1;
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

    /// 判断一张牌是否来自岭上区
    pub fn is_rinshan_tile(&self, tile: Tile) -> bool {
        (RINSHAN_START - 3..=RINSHAN_START).any(|i| self.tiles[i] == tile)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn test_wall_has_136_tiles() {
        let mut rng = StdRng::seed_from_u64(42);
        let wall = Wall::new(&mut rng);
        assert_eq!(wall.tiles.len(), 136);
    }

    #[test]
    fn test_draw() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut wall = Wall::new(&mut rng);
        assert_eq!(wall.remaining(), 122);

        let tile = wall.draw().unwrap();
        assert_eq!(wall.remaining(), 121);
        assert_eq!(wall.draw_index(), 1);
        assert!(tile.raw() < 136);
    }

    #[test]
    fn test_deal_uses_draw() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut wall = Wall::new(&mut rng);

        let mut hands: [Vec<Tile>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        for _ in 0..3 {
            for hand in &mut hands {
                for _ in 0..4 {
                    hand.push(wall.draw().unwrap());
                }
            }
        }
        hands[0].push(wall.draw().unwrap());
        hands[0].push(wall.draw().unwrap());
        for hand in hands.iter_mut().skip(1) {
            hand.push(wall.draw().unwrap());
        }

        assert_eq!(hands[0].len(), 14);
        assert_eq!(hands[1].len(), 13);
        assert_eq!(wall.remaining(), 122 - 53);
    }

    #[test]
    fn test_draw_until_exhausted() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut wall = Wall::new(&mut rng);
        for _ in 0..122 {
            assert!(wall.draw().is_some());
        }
        assert_eq!(wall.remaining(), 0);
        assert!(wall.draw().is_none());
    }

    #[test]
    fn test_dora_indicators() {
        let mut rng = StdRng::seed_from_u64(42);
        let wall = Wall::new(&mut rng);

        assert_eq!(wall.current_dora_count(), 1);
        let d0 = wall.dora_indicator(0);
        assert_eq!(d0, wall.tiles[131]);

        let ud0 = wall.ura_dora_indicator(0);
        assert_eq!(ud0, wall.tiles[126]);
    }

    #[test]
    fn test_draw_rinshan() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut wall = Wall::new(&mut rng);

        let t0 = wall.draw_rinshan().unwrap();
        assert_eq!(t0, wall.tiles()[135]);
        assert_eq!(wall.kan_count(), 1);
        assert_eq!(wall.current_dora_count(), 2);

        let t1 = wall.draw_rinshan().unwrap();
        assert_eq!(t1, wall.tiles()[134]);
        assert_eq!(wall.kan_count(), 2);

        let t2 = wall.draw_rinshan().unwrap();
        assert_eq!(t2, wall.tiles()[133]);
        assert_eq!(wall.kan_count(), 3);

        let t3 = wall.draw_rinshan().unwrap();
        assert_eq!(t3, wall.tiles()[132]);
        assert_eq!(wall.kan_count(), 4);

        assert!(wall.draw_rinshan().is_none());
    }

    #[test]
    fn test_rinshan_does_not_affect_remaining() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut wall = Wall::new(&mut rng);

        let before = wall.remaining();
        wall.draw_rinshan().unwrap();
        assert_eq!(wall.remaining(), before);
    }

    #[test]
    fn test_dora_count_caps_at_5() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut wall = Wall::new(&mut rng);

        for _ in 0..4 {
            wall.draw_rinshan().unwrap();
        }
        assert_eq!(wall.current_dora_count(), 5);
    }

    #[test]
    fn test_deterministic() {
        let mut rng1 = StdRng::seed_from_u64(12345);
        let mut rng2 = StdRng::seed_from_u64(12345);
        let wall1 = Wall::new(&mut rng1);
        let wall2 = Wall::new(&mut rng2);

        for i in 0..136 {
            assert_eq!(wall1.tiles[i], wall2.tiles[i]);
        }
    }
}
