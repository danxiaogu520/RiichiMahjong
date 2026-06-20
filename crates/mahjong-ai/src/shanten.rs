use std::collections::HashMap;

use mahjong_core::tile::{Tile, TileType};
use mahjong_yaku::types::TileCounts;

pub struct ShantenCalculator {
    cache: HashMap<[u8; 34], i8>,
}

impl ShantenCalculator {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn calculate(&mut self, hand: &[Tile]) -> i8 {
        let counts = TileCounts::from_tiles(hand);
        self.lookup(&counts)
    }

    pub fn lookup(&mut self, counts: &TileCounts) -> i8 {
        let key = *counts.inner();
        if let Some(&cached) = self.cache.get(&key) {
            return cached;
        }
        let result = self.compute_shanten(counts);
        self.cache.insert(key, result);
        result
    }

    fn compute_shanten(&self, counts: &TileCounts) -> i8 {
        let standard = self.standard_shanten(counts);
        let seven_pairs = self.seven_pairs_shanten(counts);
        let kokushi = self.kokushi_shanten(counts);
        standard.min(seven_pairs).min(kokushi)
    }

    fn standard_shanten(&self, counts: &TileCounts) -> i8 {
        let mut work = *counts;
        let mut best_score = 0usize;
        self.search(&mut work, 0, 0, 0, &mut best_score);
        8 - best_score as i8
    }

    fn search(
        &self,
        counts: &mut TileCounts,
        start: u8,
        mentsu: usize,
        pairs: usize,
        best_score: &mut usize,
    ) {
        let idx = match (start..34).find(|&i| counts.get(TileType(i)) > 0) {
            Some(i) => i,
            None => {
                // All tiles consumed — check for partial meld in leftover
                let has_partial = self.has_partial_meld(counts);
                let score = mentsu * 2 + pairs + if has_partial { 1 } else { 0 };
                if score > *best_score {
                    *best_score = score;
                }
                return;
            }
        };

        let remaining: u8 = (idx..34).map(|i| counts.get(TileType(i))).sum();
        let cur_score = mentsu * 2 + pairs;
        if cur_score + (remaining as usize) <= *best_score {
            return;
        }

        let tt = TileType(idx);

        // Try pair (only one pair allowed)
        if pairs == 0 && counts.get(tt) >= 2 {
            counts.dec(tt);
            counts.dec(tt);
            self.search(counts, idx, mentsu, 1, best_score);
            counts.inc(tt);
            counts.inc(tt);
        }

        // Try triplet
        if counts.get(tt) >= 3 {
            counts.dec(tt);
            counts.dec(tt);
            counts.dec(tt);
            self.search(counts, idx, mentsu + 1, pairs, best_score);
            counts.inc(tt);
            counts.inc(tt);
            counts.inc(tt);
        }

        // Try sequence (number tiles, rank <= 7)
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
                self.search(counts, idx, mentsu + 1, pairs, best_score);
                counts.inc(tt);
                counts.inc(tt1);
                counts.inc(tt2);
            }
        }

        // Skip this tile
        self.search(counts, idx + 1, mentsu, pairs, best_score);
    }

    fn has_partial_meld(&self, counts: &TileCounts) -> bool {
        for i in 0..34u8 {
            let tt = TileType(i);
            if counts.get(tt) == 0 {
                continue;
            }
            // Pair wait
            if counts.get(tt) >= 2 {
                return true;
            }
            // Two-sided or middle wait with adjacent tile
            if tt.is_number() && i + 1 < 34 {
                let tt1 = TileType(i + 1);
                if tt.suit() == tt1.suit() && counts.get(tt1) > 0 {
                    return true;
                }
            }
            // Gap wait (e.g., 3p and 5p waiting for 4p)
            if tt.is_number() && tt.rank().0 <= 7 && i + 2 < 34 {
                let tt2 = TileType(i + 2);
                if tt.suit() == tt2.suit() && counts.get(tt2) > 0 {
                    return true;
                }
            }
        }
        false
    }

    fn seven_pairs_shanten(&self, counts: &TileCounts) -> i8 {
        let pairs = (0..34).filter(|&i| counts.get(TileType(i)) >= 2).count();
        6 - pairs as i8
    }

    fn kokushi_shanten(&self, counts: &TileCounts) -> i8 {
        let mut types_present = 0i8;
        let mut has_pair = false;
        for &tt in &TileType::YAOCHUUHAI {
            let c = counts.get(tt);
            if c >= 1 { types_present += 1; }
            if c >= 2 { has_pair = true; }
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
    fn test_user_hand_tenpai() {
        let mut calc = ShantenCalculator::new();
        let hand = make_tiles(&[
            (Suit::Man, Rank(9), 0), (Suit::Man, Rank(9), 1),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(2), 0),
            (Suit::Pin, Rank(3), 0), (Suit::Pin, Rank(3), 1),
            (Suit::Pin, Rank(4), 0),
            (Suit::Pin, Rank(7), 0), (Suit::Pin, Rank(8), 0), (Suit::Pin, Rank(9), 0),
            (Suit::Sou, Rank(3), 0), (Suit::Sou, Rank(4), 0), (Suit::Sou, Rank(5), 0),
        ]);
        let s = calc.calculate(&hand);
        assert_eq!(s, 0, "should be tenpai (shanten=0), got {}", s);
    }

    #[test]
    fn test_tenpai_standard() {
        let mut calc = ShantenCalculator::new();
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
        assert_eq!(calc.calculate(&hand), 6);
    }

    #[test]
    fn test_cache_works() {
        let mut calc = ShantenCalculator::new();
        let hand = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(2), 0), (Suit::Man, Rank(3), 0),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(5), 0), (Suit::Man, Rank(6), 0),
            (Suit::Man, Rank(7), 0), (Suit::Man, Rank(8), 0), (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(1), 1),
            (Suit::Pin, Rank(2), 0), (Suit::Pin, Rank(3), 0),
        ]);
        let r1 = calc.calculate(&hand);
        assert!(!calc.cache.is_empty());
        let r2 = calc.calculate(&hand);
        assert_eq!(r1, r2);
    }
}
