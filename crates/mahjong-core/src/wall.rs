use rand::Rng;
use rand::seq::SliceRandom;

use crate::tile::Tile;

/// 正常摸牌区上界（王牌区从这里开始）
pub const DEAD_WALL_START: usize = 122;

/// 宝牌指示牌索引（倒数第5,7,9,11,13张）
pub const DORA_INDICATOR_INDEX: [usize; 5] = [130, 128, 126, 124, 122];

/// 里宝牌指示牌索引（倒数第6,8,10,12,14张）
pub const URA_DORA_INDICATOR_INDEX: [usize; 5] = [131, 129, 127, 125, 123];

/// 岭上牌索引（按开杠顺序取用）
pub const KAN_DRAW_INDEX: [usize; 4] = [134, 135, 132, 133];

/// 牌山：136 张牌的固定数组，通过指针索引不同区域。
///
/// ```text
/// 索引:  0                    122  124  126  128  130  132  134  135
///        |←— 正常摸牌区 ——→|←— 宝牌指示牌区 ——→|← 岭上牌 →|
///        draw_index →        123  125  127  129  131  133
///                            |←— 里宝牌指示牌 ——→|
/// ```
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
    /// 按 `kan_count` 从 `KAN_DRAW_INDEX` 取牌，然后 `kan_count += 1`。
    pub fn draw_rinshan(&mut self) -> Option<Tile> {
        if self.kan_count >= 4 {
            return None;
        }
        let index = KAN_DRAW_INDEX[self.kan_count];
        let tile = self.tiles[index];
        self.kan_count += 1;
        Some(tile)
    }

    /// 获取第 i 张宝牌指示牌（0 = 初始，1 = 第一次杠后翻开，以此类推）。
    pub fn dora_indicator(&self, i: usize) -> Tile {
        self.tiles[DORA_INDICATOR_INDEX[i]]
    }

    /// 获取第 i 张里宝牌指示牌。
    pub fn ura_dora_indicator(&self, i: usize) -> Tile {
        self.tiles[URA_DORA_INDICATOR_INDEX[i]]
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

        // 手动模拟配牌：庄家14张，其余13张
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
        // 14 + 13*3 = 53 张配出
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

        // 初始翻开 1 张宝牌指示牌
        assert_eq!(wall.current_dora_count(), 1);
        let d0 = wall.dora_indicator(0);
        assert_eq!(d0, wall.tiles[130]); // 倒数第5张

        // 里宝牌指示牌
        let ud0 = wall.ura_dora_indicator(0);
        assert_eq!(ud0, wall.tiles[131]); // 倒数第6张
    }

    #[test]
    fn test_draw_rinshan() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut wall = Wall::new(&mut rng);

        // 第1次杠：从 index 134 取岭上牌
        let t0 = wall.draw_rinshan().unwrap();
        assert_eq!(t0, wall.tiles()[134]);
        assert_eq!(wall.kan_count(), 1);
        assert_eq!(wall.current_dora_count(), 2);

        // 第2次杠：从 index 135
        let t1 = wall.draw_rinshan().unwrap();
        assert_eq!(t1, wall.tiles()[135]);
        assert_eq!(wall.kan_count(), 2);

        // 第3次杠：从 index 132
        let t2 = wall.draw_rinshan().unwrap();
        assert_eq!(t2, wall.tiles()[132]);
        assert_eq!(wall.kan_count(), 3);

        // 第4次杠：从 index 133
        let t3 = wall.draw_rinshan().unwrap();
        assert_eq!(t3, wall.tiles()[133]);
        assert_eq!(wall.kan_count(), 4);

        // 第5次杠：无岭上牌
        assert!(wall.draw_rinshan().is_none());
    }

    #[test]
    fn test_rinshan_does_not_affect_remaining() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut wall = Wall::new(&mut rng);

        let before = wall.remaining();
        wall.draw_rinshan().unwrap();
        // 岭上牌取自王牌区，不影响正常摸牌区剩余数
        assert_eq!(wall.remaining(), before);
    }

    #[test]
    fn test_dora_count_caps_at_5() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut wall = Wall::new(&mut rng);

        for _ in 0..4 {
            wall.draw_rinshan().unwrap();
        }
        // 1 + 4 = 5，不超过 5
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
