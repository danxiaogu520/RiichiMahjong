use std::collections::HashMap;

use mahjong_core::tile::{Tile, TileType};
use mahjong_yaku::types::TileCounts;

pub struct ShantenCalculator {
    cache: HashMap<TileCounts, i8>,
}

impl ShantenCalculator {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn calculate(&mut self, hand: &[Tile]) -> i8 {
        let counts = TileCounts::from_tiles(hand);
        self.calculate_from_counts(&counts)
    }

    pub fn calculate_from_counts(&mut self, counts: &TileCounts) -> i8 {
        if let Some(&cached) = self.cache.get(counts) {
            return cached;
        }
        let standard = self.standard_shanten(counts);
        let seven_pairs = self.seven_pairs_shanten(counts);
        let kokushi = self.kokushi_shanten(counts);
        let result = standard.min(seven_pairs).min(kokushi);
        self.cache.insert(*counts, result);
        result
    }

    fn standard_shanten(&mut self, counts: &TileCounts) -> i8 {
        let mut work = *counts;
        let mut best = (0usize, false); // (mentsu_count, has_pair)
        self.search(&mut work, 0, 0, false, &mut best);
        8 - 2 * best.0 as i8 - if best.1 { 1 } else { 0 }
    }

    fn search(
        &mut self,
        counts: &mut TileCounts,
        start: u8,
        mentsu: usize,
        has_pair: bool,
        best: &mut (usize, bool),
    ) {
        // 找到第一个有牌的位置
        let idx = match (start..34).find(|&i| counts.get(TileType(i)) > 0) {
            Some(i) => i,
            None => {
                // 所有牌处理完，更新最优
                let score = mentsu * 2 + if has_pair { 1 } else { 0 };
                let best_score = best.0 * 2 + if best.1 { 1 } else { 0 };
                if score > best_score {
                    *best = (mentsu, has_pair);
                }
                return;
            }
        };

        let tt = TileType(idx);

        // 剪枝：即使剩余全组成面子+雀头也无法超过当前最优
        let remaining: u8 = (idx..34).map(|i| counts.get(TileType(i))).sum();
        let _max_possible = mentsu + (remaining as usize) / 3 + if !has_pair && remaining >= 2 { 1 } else { 0 };
        let best_score = best.0 * 2 + if best.1 { 1 } else { 0 };
        if mentsu * 2 + if has_pair { 1 } else { 0 } + (remaining as usize) * 2 / 3 <= best_score {
            return;
        }

        // 尝试雀头
        if !has_pair && counts.get(tt) >= 2 {
            counts.dec(tt);
            counts.dec(tt);
            self.search(counts, idx, mentsu, true, best);
            counts.inc(tt);
            counts.inc(tt);
        }

        // 尝试刻子
        if counts.get(tt) >= 3 {
            counts.dec(tt);
            counts.dec(tt);
            counts.dec(tt);
            self.search(counts, idx, mentsu + 1, has_pair, best);
            counts.inc(tt);
            counts.inc(tt);
            counts.inc(tt);
        }

        // 尝试顺子（数牌，rank <= 7）
        if tt.is_number() && tt.rank().0 <= 7 {
            let tt1 = TileType(idx + 1);
            let tt2 = TileType(idx + 2);
            if tt.suit() == tt1.suit()
                && tt.suit() == tt2.suit()
                && counts.get(tt) >= 1
                && counts.get(tt1) >= 1
                && counts.get(tt2) >= 1
            {
                counts.dec(tt);
                counts.dec(tt1);
                counts.dec(tt2);
                self.search(counts, idx, mentsu + 1, has_pair, best);
                counts.inc(tt);
                counts.inc(tt1);
                counts.inc(tt2);
            }
        }

        // 跳过这张牌
        self.search(counts, idx + 1, mentsu, has_pair, best);
    }

    fn seven_pairs_shanten(&self, counts: &TileCounts) -> i8 {
        let mut pairs = 0i8;
        for i in 0..34u8 {
            let c = counts.get(TileType(i));
            if c >= 2 {
                pairs += 1;
            }
        }
        6 - pairs
    }

    fn kokushi_shanten(&self, counts: &TileCounts) -> i8 {
        let mut types_present = 0i8;
        let mut has_pair = false;

        for &tt in &TileType::YAOCHUUHAI {
            let c = counts.get(tt);
            if c >= 1 {
                types_present += 1;
            }
            if c >= 2 {
                has_pair = true;
            }
        }

        13 - types_present - if has_pair { 1 } else { 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mahjong_core::tile::{Rank, Suit};

    fn make_tiles(spec: &[(Suit, Rank, u8)]) -> Vec<Tile> {
        spec.iter().map(|&(s, r, c)| Tile::new(s, r, c)).collect()
    }

    #[test]
    fn test_tenpai_standard() {
        let mut calc = ShantenCalculator::new();
        // 1m2m3m 4m5m6m 7m8m9m 1p1p 2p3p → 听 1p/4p → shanten=0 (13张)
        let hand = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(2), 0), (Suit::Man, Rank(3), 0),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(5), 0), (Suit::Man, Rank(6), 0),
            (Suit::Man, Rank(7), 0), (Suit::Man, Rank(8), 0), (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(1), 1),
            (Suit::Pin, Rank(2), 0), (Suit::Pin, Rank(3), 0),
        ]);
        assert_eq!(calc.calculate(&hand), 0);
    }

    #[test]
    fn test_tenpai_with_pair_wait() {
        let mut calc = ShantenCalculator::new();
        // 1m2m3m 4m5m6m 7m8m9m 1p2p3p 1z → 听 1z → shanten=0 (13张)
        let hand = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(2), 0), (Suit::Man, Rank(3), 0),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(5), 0), (Suit::Man, Rank(6), 0),
            (Suit::Man, Rank(7), 0), (Suit::Man, Rank(8), 0), (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(2), 0), (Suit::Pin, Rank(3), 0),
            (Suit::Wind, Rank(1), 0),
        ]);
        assert_eq!(calc.calculate(&hand), 0);
    }

    #[test]
    fn test_shanten_one() {
        let mut calc = ShantenCalculator::new();
        // 1m2m3m 4m5m6m 7m8m9m 1p1p 2p 5s → 13张 → 3面子+1雀头 → shanten=1
        let hand = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(2), 0), (Suit::Man, Rank(3), 0),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(5), 0), (Suit::Man, Rank(6), 0),
            (Suit::Man, Rank(7), 0), (Suit::Man, Rank(8), 0), (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(1), 1),
            (Suit::Pin, Rank(2), 0), (Suit::Sou, Rank(5), 0),
        ]);
        assert_eq!(calc.calculate(&hand), 1);
    }

    #[test]
    fn test_seven_pairs_tenpai() {
        let mut calc = ShantenCalculator::new();
        // 1m1m 2m2m 3m3m 4m4m 5m5m 6m6m 7m → 七对子听牌 → shanten=0
        let hand = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(1), 1),
            (Suit::Man, Rank(2), 0), (Suit::Man, Rank(2), 1),
            (Suit::Man, Rank(3), 0), (Suit::Man, Rank(3), 1),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(4), 1),
            (Suit::Man, Rank(5), 0), (Suit::Man, Rank(5), 1),
            (Suit::Man, Rank(6), 0), (Suit::Man, Rank(6), 1),
            (Suit::Man, Rank(7), 0),
        ]);
        assert_eq!(calc.calculate(&hand), 0);
    }

    #[test]
    fn test_kokushi_tenpai() {
        let mut calc = ShantenCalculator::new();
        // 1m 9m 1p 9p 1s 9s 东南西北白發 中 → 国士听牌 → shanten=0
        let hand = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(9), 0),
            (Suit::Sou, Rank(1), 0), (Suit::Sou, Rank(9), 0),
            (Suit::Wind, Rank(1), 0), (Suit::Wind, Rank(2), 0),
            (Suit::Wind, Rank(3), 0), (Suit::Wind, Rank(4), 0),
            (Suit::Dragon, Rank(1), 0), (Suit::Dragon, Rank(2), 0),
            (Suit::Dragon, Rank(3), 0),
        ]);
        assert_eq!(calc.calculate(&hand), 0);
    }

    #[test]
    fn test_empty_hand() {
        let mut calc = ShantenCalculator::new();
        let hand: Vec<Tile> = vec![];
        // 0张牌: 标准形=8, 七对子=6, 国士=13 → min=6
        assert_eq!(calc.calculate(&hand), 6);
    }

    #[test]
    fn test_cache_hit() {
        let mut calc = ShantenCalculator::new();
        let hand = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(2), 0), (Suit::Man, Rank(3), 0),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(5), 0), (Suit::Man, Rank(6), 0),
            (Suit::Man, Rank(7), 0), (Suit::Man, Rank(8), 0), (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(1), 1),
            (Suit::Pin, Rank(2), 0),
        ]);
        let r1 = calc.calculate(&hand);
        let r2 = calc.calculate(&hand);
        assert_eq!(r1, r2);
        assert!(calc.cache.len() > 0);
    }
}
