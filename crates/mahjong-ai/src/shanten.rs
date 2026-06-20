use mahjong_core::tile::Tile;
use mahjong_yaku::types::TileCounts;

static INDEX_S: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/index_s.bin"));
static INDEX_H: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/index_h.bin"));

const ENTRY_SIZE: usize = 10;

fn hash_base5<const N: usize>(counts: &[u8]) -> usize {
    let mut h: usize = 0;
    for i in 0..N {
        h = h * 5 + counts[i] as usize;
    }
    h
}

fn lookup_s(counts: &[u8]) -> [u8; 10] {
    let idx = hash_base5::<9>(counts) * ENTRY_SIZE;
    let mut entry = [0u8; 10];
    entry.copy_from_slice(&INDEX_S[idx..idx + ENTRY_SIZE]);
    entry
}

fn lookup_h(counts: &[u8]) -> [u8; 10] {
    let idx = hash_base5::<7>(counts) * ENTRY_SIZE;
    let mut entry = [0u8; 10];
    entry.copy_from_slice(&INDEX_H[idx..idx + ENTRY_SIZE]);
    entry
}

fn add1(lhs: &mut [u8; 10], rhs: &[u8; 10], m: usize) {
    for j in (5..=m + 5).rev() {
        let mut sht = std::cmp::min(
            lhs[j] as i32 + rhs[0] as i32,
            lhs[0] as i32 + rhs[j] as i32,
        );
        for k in 5..j {
            sht = std::cmp::min(
                sht,
                std::cmp::min(
                    lhs[k] as i32 + rhs[j - k] as i32,
                    lhs[j - k] as i32 + rhs[k] as i32,
                ),
            );
        }
        lhs[j] = sht as u8;
    }
    for j in (0..=m).rev() {
        let mut sht = lhs[j] as i32 + rhs[0] as i32;
        for k in 0..j {
            sht = std::cmp::min(sht, lhs[k] as i32 + rhs[j - k] as i32);
        }
        lhs[j] = sht as u8;
    }
}

fn add2(lhs: &mut [u8; 10], rhs: &[u8; 10], m: usize) {
    let j = m + 5;
    let mut sht = std::cmp::min(
        lhs[j] as i32 + rhs[0] as i32,
        lhs[0] as i32 + rhs[j] as i32,
    );
    for k in 5..j {
        sht = std::cmp::min(
            sht,
            std::cmp::min(
                lhs[k] as i32 + rhs[j - k] as i32,
                lhs[j - k] as i32 + rhs[k] as i32,
            ),
        );
    }
    lhs[j] = sht as u8;
}

fn calc_lh(counts: &[u8; 34], m: usize) -> i8 {
    let mut ret = lookup_h(&counts[27..34]);
    add1(&mut ret, &lookup_s(&counts[18..27]), m);
    add1(&mut ret, &lookup_s(&counts[9..18]), m);
    add2(&mut ret, &lookup_s(&counts[0..9]), m);
    ret[m + 5] as i8 - 1
}

fn calc_sp(counts: &[u8; 34]) -> i8 {
    let mut pair = 0i8;
    let mut kind = 0i8;
    for i in 0..34 {
        if counts[i] > 0 {
            kind += 1;
            if counts[i] >= 2 {
                pair += 1;
            }
        }
    }
    7 - pair + if kind < 7 { 7 - kind } else { 0 } - 1
}

fn calc_to(counts: &[u8; 34]) -> i8 {
    let yaochuuhai = [0, 8, 9, 17, 18, 26, 27, 28, 29, 30, 31, 32, 33];
    let mut pair = 0i8;
    let mut kind = 0i8;
    for &i in &yaochuuhai {
        if counts[i] > 0 {
            kind += 1;
            if counts[i] >= 2 {
                pair += 1;
            }
        }
    }
    14 - kind - if pair > 0 { 1 } else { 0 } - 1
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
        let total: u8 = c.iter().sum();
        let m = total as usize / 3;

        let lh = calc_lh(c, m);
        let sp = if m == 4 { calc_sp(c) } else { i8::MAX };
        let to = if m == 4 { calc_to(c) } else { i8::MAX };

        lh.min(sp).min(to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::seq::SliceRandom;
    use rand::SeedableRng;

    #[test]
    fn test_shanten_distribution() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(0);
        let num_tiles = 14usize;
        let num_hands = 1_000_000usize;

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
            let sht = calc_lh(&counts, num_tiles / 3);
            let sp = calc_sp(&counts);
            let to = calc_to(&counts);
            let result = sht.min(sp).min(to);
            let idx = (result + 1) as usize;
            if idx < dist.len() {
                dist[idx] += 1;
            }
            ev += result as f64;
        }

        ev /= num_hands as f64;

        println!("Shanten distribution ({} hands):", num_hands);
        for i in 0..8 {
            let sht = i as i8 - 1;
            let count = dist[i];
            let pct = count as f64 * 100.0 / num_hands as f64;
            println!("{:4}{:12}{:12.6}", sht, count, pct);
        }
        println!("Expected value: {:.5}", ev);

        // Verify against reference values (tolerance ±0.5%)
        let ref_pcts = [0.0003, 0.07, 2.33, 19.50, 43.93, 28.52, 5.50, 0.16];
        for i in 0..8 {
            let actual = dist[i] as f64 * 100.0 / num_hands as f64;
            let expected = ref_pcts[i];
            if expected > 0.01 {
                assert!(
                    (actual - expected).abs() < 0.5,
                    "shanten {}: expected ~{:.2}%, got {:.4}%",
                    i as i8 - 1,
                    expected,
                    actual
                );
            }
        }
        assert!(
            (ev - 3.156).abs() < 0.01,
            "expected value ~3.156, got {:.5}",
            ev
        );
    }

    #[test]
    fn test_specific_hands() {
        let calc = ShantenCalculator::new();

        // 1m2m3m 4m5m6m 7m8m9m 1p2p3p 4p → tenpai
        let hand = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(2), 0), (Suit::Man, Rank(3), 0),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(5), 0), (Suit::Man, Rank(6), 0),
            (Suit::Man, Rank(7), 0), (Suit::Man, Rank(8), 0), (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(2), 0), (Suit::Pin, Rank(3), 0),
            (Suit::Pin, Rank(4), 0),
        ]);
        assert_eq!(calc.calculate(&hand), 0, "tenpai hand");

        // 1m2m3m 4m5m6m 7m8m9m 1p1p 2p3p → tenpai (ryanmen)
        let hand = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(2), 0), (Suit::Man, Rank(3), 0),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(5), 0), (Suit::Man, Rank(6), 0),
            (Suit::Man, Rank(7), 0), (Suit::Man, Rank(8), 0), (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(1), 1),
            (Suit::Pin, Rank(2), 0), (Suit::Pin, Rank(3), 0),
        ]);
        assert_eq!(calc.calculate(&hand), 0, "tenpai ryanmen");

        // 1m1m 2m2m 3m3m 4m4m 5m5m 6m6m 7m → tenpai (7 pairs)
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

        // 1m 9m 1p 9p 1s 9s 1z2z3z4z 5z6z7z → tenpai (kokushi)
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

        // 9m9m 1p2p 3p3p4p 7p8p9p 3s4s5s → tenpai (3p4p taatsu)
        let hand = make_tiles(&[
            (Suit::Man, Rank(9), 0), (Suit::Man, Rank(9), 1),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(2), 0),
            (Suit::Pin, Rank(3), 0), (Suit::Pin, Rank(3), 1), (Suit::Pin, Rank(4), 0),
            (Suit::Pin, Rank(7), 0), (Suit::Pin, Rank(8), 0), (Suit::Pin, Rank(9), 0),
            (Suit::Sou, Rank(3), 0), (Suit::Sou, Rank(4), 0), (Suit::Sou, Rank(5), 0),
        ]);
        assert_eq!(calc.calculate(&hand), 0, "taatsu tenpai");

        // 1m 4m 7m 2p 5p 8p 3s 6s 9s 1z 3z 5z 7z → high shanten
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
