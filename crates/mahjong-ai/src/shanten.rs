use mahjong_core::tile::Tile;
use mahjong_yaku::types::TileCounts;

/// 数牌（9种）查找表：每条目10字节，存储0~4面子无雀头/有雀头的部分置换数
static SUIT_TABLE: &[u8] = include_bytes!("../data/index_s.bin");
/// 字牌（7种）查找表
static HONOR_TABLE: &[u8] = include_bytes!("../data/index_h.bin");

const ENTRY_LEN: usize = 10;

/// Base-5 编码：将 N 种牌的计数数组映射为查找表索引
fn encode_base5<const N: usize>(counts: &[u8]) -> usize {
    counts.iter().take(N).fold(0, |acc, &c| acc * 5 + (c.min(4)) as usize)
}

fn lookup_suit(counts: &[u8]) -> [u8; 10] {
    let idx = encode_base5::<9>(counts) * ENTRY_LEN;
    SUIT_TABLE.get(idx..idx + ENTRY_LEN)
        .map(|s| <[u8; 10]>::try_from(s).unwrap())
        .unwrap_or([14u8; 10])
}

fn lookup_honor(counts: &[u8]) -> [u8; 10] {
    let idx = encode_base5::<7>(counts) * ENTRY_LEN;
    HONOR_TABLE.get(idx..idx + ENTRY_LEN)
        .map(|s| <[u8; 10]>::try_from(s).unwrap())
        .unwrap_or([14u8; 10])
}

/// 合并两组花色的部分置换数（雀头可来自任一方）
///
/// acc[0..5]: u_0..u_4（无雀头）
/// acc[5..10]: t_0..t_4（有雀头）
fn merge_with_pair(acc: &mut [u8; 10], other: &[u8; 10], num_melds: usize) {
    let max_j = (num_melds + 5).min(9);
    for j in (5..=max_j).rev() {
        let mut best = std::cmp::min(acc[j] as i32 + other[0] as i32, acc[0] as i32 + other[j] as i32);
        for k in 5..j {
            best = std::cmp::min(best, std::cmp::min(
                acc[k] as i32 + other[j - k] as i32,
                acc[j - k] as i32 + other[k] as i32,
            ));
        }
        acc[j] = best as u8;
    }
    for j in (0..=num_melds.min(4)).rev() {
        let mut best = acc[j] as i32 + other[0] as i32;
        for k in 0..j {
            best = std::cmp::min(best, acc[k] as i32 + other[j - k] as i32);
        }
        acc[j] = best as u8;
    }
}

/// 合并最后一组花色（雀头已在前面确定，只更新有雀头部分）
fn merge_final(acc: &mut [u8; 10], other: &[u8; 10], num_melds: usize) {
    let j = (num_melds + 5).min(9);
    let mut best = std::cmp::min(acc[j] as i32 + other[0] as i32, acc[0] as i32 + other[j] as i32);
    for k in 5..j {
        best = std::cmp::min(best, std::cmp::min(
            acc[k] as i32 + other[j - k] as i32,
            acc[j - k] as i32 + other[k] as i32,
        ));
    }
    acc[j] = best as u8;
}

/// 标准形（四面子一雀头）向听数
fn calc_standard(counts: &[u8; 34], num_melds: usize) -> i8 {
    let m = num_melds.min(4);
    let mut dist = lookup_honor(&counts[27..34]);
    merge_with_pair(&mut dist, &lookup_suit(&counts[18..27]), m);
    merge_with_pair(&mut dist, &lookup_suit(&counts[9..18]), m);
    merge_final(&mut dist, &lookup_suit(&counts[0..9]), m);
    dist[m + 5] as i8 - 1
}

/// 七对子向听数
fn calc_seven_pairs(counts: &[u8; 34]) -> i8 {
    let pairs = counts.iter().filter(|&&c| c >= 2).count() as i8;
    let kinds = counts.iter().filter(|&&c| c > 0).count() as i8;
    7 - pairs + if kinds < 7 { 7 - kinds } else { 0 } - 1
}

/// 国士无双向听数
fn calc_thirteen_orphans(counts: &[u8; 34]) -> i8 {
    let yaochuuhai = [0, 8, 9, 17, 18, 26, 27, 28, 29, 30, 31, 32, 33];
    let kinds = yaochuuhai.iter().filter(|&&i| counts[i] > 0).count() as i8;
    let has_pair = yaochuuhai.iter().any(|&i| counts[i] >= 2);
    14 - kinds - if has_pair { 1 } else { 0 } - 1
}

pub struct ShantenCalculator;

impl ShantenCalculator {
    pub fn new() -> Self {
        Self
    }

    pub fn calculate(&self, hand: &[Tile]) -> i8 {
        let counts = TileCounts::from_tiles(hand);
        self.lookup(&counts)
    }

    pub fn lookup(&self, counts: &TileCounts) -> i8 {
        let c = counts.inner();
        let num_melds = c.iter().sum::<u8>() as usize / 3;

        let standard = calc_standard(c, num_melds);
        let seven_pairs = if num_melds == 4 { calc_seven_pairs(c) } else { i8::MAX };
        let thirteen_orphans = if num_melds == 4 { calc_thirteen_orphans(c) } else { i8::MAX };

        standard.min(seven_pairs).min(thirteen_orphans)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::seq::SliceRandom;
    use rand::SeedableRng;

    fn run_distribution_test(num_tiles: usize, num_hands: usize, expected_ev: f64) {
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);

        let mut wall: Vec<usize> = Vec::with_capacity(136);
        for i in 0..34 {
            for _ in 0..4 {
                wall.push(i);
            }
        }

        let mut dist = [0i64; 8];
        let mut ev = 0.0f64;

        for _ in 0..num_hands {
            wall.shuffle(&mut rng);
            let mut counts = [0u8; 34];
            for j in 0..num_tiles {
                counts[wall[j]] += 1;
            }
            let s = calc_standard(&counts, num_tiles / 3);
            let sp = calc_seven_pairs(&counts);
            let to = calc_thirteen_orphans(&counts);
            let result = s.min(sp).min(to);
            let idx = (result + 1) as usize;
            if idx < dist.len() {
                dist[idx] += 1;
            }
            ev += result as f64;
        }

        ev /= num_hands as f64;

        println!("\n=== {} tiles, {} hands ===", num_tiles, num_hands);
        for i in 0..8 {
            let sht = i as i8 - 1;
            let count = dist[i];
            let pct = count as f64 * 100.0 / num_hands as f64;
            println!("{:4}{:12}{:12.6}", sht, count, pct);
        }
        println!("Expected: {:.5}, Actual: {:.5}", expected_ev, ev);

        assert!(
            (ev - expected_ev).abs() < 0.005,
            "expected {:.5}, got {:.5}",
            expected_ev,
            ev
        );
    }

    #[test]
    fn test_distribution_14tiles() {
        run_distribution_test(14, 1_000_000, 3.15593);
    }

    #[test]
    fn test_distribution_13tiles() {
        run_distribution_test(13, 1_000_000, 3.57967);
    }

    #[test]
    fn test_specific_hands() {
        let calc = ShantenCalculator::new();

        let hand = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(2), 0), (Suit::Man, Rank(3), 0),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(5), 0), (Suit::Man, Rank(6), 0),
            (Suit::Man, Rank(7), 0), (Suit::Man, Rank(8), 0), (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(2), 0), (Suit::Pin, Rank(3), 0),
            (Suit::Pin, Rank(4), 0),
        ]);
        assert_eq!(calc.calculate(&hand), 0, "tenpai hand");

        let hand = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(2), 0), (Suit::Man, Rank(3), 0),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(5), 0), (Suit::Man, Rank(6), 0),
            (Suit::Man, Rank(7), 0), (Suit::Man, Rank(8), 0), (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(1), 1),
            (Suit::Pin, Rank(2), 0), (Suit::Pin, Rank(3), 0),
        ]);
        assert_eq!(calc.calculate(&hand), 0, "tenpai ryanmen");

        let hand = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(1), 1),
            (Suit::Man, Rank(2), 0), (Suit::Man, Rank(2), 1),
            (Suit::Man, Rank(3), 0), (Suit::Man, Rank(3), 1),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(4), 1),
            (Suit::Man, Rank(5), 0), (Suit::Man, Rank(5), 1),
            (Suit::Man, Rank(6), 0), (Suit::Man, Rank(6), 1),
            (Suit::Man, Rank(7), 0),
        ]);
        assert_eq!(calc.calculate(&hand), 0, "7 pairs tenpai");

        let hand = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(9), 0),
            (Suit::Sou, Rank(1), 0), (Suit::Sou, Rank(9), 0),
            (Suit::Wind, Rank(1), 0), (Suit::Wind, Rank(2), 0),
            (Suit::Wind, Rank(3), 0), (Suit::Wind, Rank(4), 0),
            (Suit::Dragon, Rank(1), 0), (Suit::Dragon, Rank(2), 0),
            (Suit::Dragon, Rank(3), 0),
        ]);
        assert_eq!(calc.calculate(&hand), 0, "kokushi tenpai");

        let hand = make_tiles(&[
            (Suit::Man, Rank(9), 0), (Suit::Man, Rank(9), 1),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(2), 0),
            (Suit::Pin, Rank(3), 0), (Suit::Pin, Rank(3), 1), (Suit::Pin, Rank(4), 0),
            (Suit::Pin, Rank(7), 0), (Suit::Pin, Rank(8), 0), (Suit::Pin, Rank(9), 0),
            (Suit::Sou, Rank(3), 0), (Suit::Sou, Rank(4), 0), (Suit::Sou, Rank(5), 0),
        ]);
        assert_eq!(calc.calculate(&hand), 0, "taatsu tenpai");

        let hand = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(4), 0), (Suit::Man, Rank(7), 0),
            (Suit::Pin, Rank(2), 0), (Suit::Pin, Rank(5), 0), (Suit::Pin, Rank(8), 0),
            (Suit::Sou, Rank(3), 0), (Suit::Sou, Rank(6), 0), (Suit::Sou, Rank(9), 0),
            (Suit::Wind, Rank(1), 0), (Suit::Wind, Rank(3), 0),
            (Suit::Dragon, Rank(1), 0), (Suit::Dragon, Rank(3), 0),
        ]);
        let sht = calc.calculate(&hand);
        assert!(sht >= 5, "scattered hand should be 5+ shanten, got {}", sht);
    }

    use mahjong_core::tile::{Rank, Suit};

    fn make_tiles(spec: &[(Suit, Rank, u8)]) -> Vec<Tile> {
        spec.iter().map(|&(s, r, c)| Tile::new(s, r, c)).collect()
    }
}
