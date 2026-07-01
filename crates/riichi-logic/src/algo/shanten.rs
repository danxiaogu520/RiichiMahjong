use std::io::prelude::*;
use std::sync::LazyLock;

use flate2::read::GzDecoder;
use riichi_core::tile_index;

const JIHAI_TABLE_SIZE: usize = 78_032;
const SUHAI_TABLE_SIZE: usize = 1_940_777;

static JIHAI_TABLE: LazyLock<Vec<[u8; 10]>> = LazyLock::new(|| {
    read_table(
        include_bytes!("data/shanten_jihai.bin.gz"),
        JIHAI_TABLE_SIZE,
    )
});

static SUHAI_TABLE: LazyLock<Vec<[u8; 10]>> = LazyLock::new(|| {
    read_table(
        include_bytes!("data/shanten_suhai.bin.gz"),
        SUHAI_TABLE_SIZE,
    )
});

fn read_table(gzipped: &[u8], length: usize) -> Vec<[u8; 10]> {
    let mut gz = GzDecoder::new(gzipped);
    let mut raw = vec![];
    gz.read_to_end(&mut raw).unwrap();

    let mut ret = Vec::with_capacity(length);
    let mut entry = [0; 10];
    for (i, b) in raw.into_iter().enumerate() {
        entry[i * 2 % 10] = b & 0b1111;
        entry[i * 2 % 10 + 1] = (b >> 4) & 0b1111;
        if (i + 1) % 5 == 0 {
            ret.push(entry);
        }
    }
    assert_eq!(ret.len(), length);

    ret
}

pub fn ensure_init() {
    assert_eq!(JIHAI_TABLE.len(), JIHAI_TABLE_SIZE);
    assert_eq!(SUHAI_TABLE.len(), SUHAI_TABLE_SIZE);
}

fn add_suit(lhs: &mut [u8; 10], index: usize, m: usize) {
    let tab = SUHAI_TABLE.get(index).copied().unwrap_or_default();

    for j in (5..=(5 + m)).rev() {
        let mut sht = (lhs[j] + tab[0]).min(lhs[0] + tab[j]);
        for k in 5..j {
            sht = sht.min(lhs[k] + tab[j - k]).min(lhs[j - k] + tab[k]);
        }
        lhs[j] = sht;
    }

    for j in (0..=m).rev() {
        let mut sht = lhs[j] + tab[0];
        for k in 0..j {
            sht = sht.min(lhs[k] + tab[j - k]);
        }
        lhs[j] = sht;
    }
}

fn add_honour(lhs: &mut [u8; 10], index: usize, m: usize) {
    let tab = JIHAI_TABLE.get(index).copied().unwrap_or_default();

    let j = m + 5;
    let mut sht = (lhs[j] + tab[0]).min(lhs[0] + tab[j]);
    for k in 5..j {
        sht = sht.min(lhs[k] + tab[j - k]).min(lhs[j - k] + tab[k]);
    }
    lhs[j] = sht;
}

fn sum_tiles(tiles: &[u8]) -> usize {
    tiles.iter().fold(0, |acc, &x| acc * 5 + x as usize)
}

#[must_use]
pub fn calculate_normal(tiles: &[u8; 34], len_div3: u8) -> i8 {
    let len_div3 = len_div3 as usize;

    let mut ret = SUHAI_TABLE
        .get(sum_tiles(&tiles[..9]))
        .copied()
        .unwrap_or_default();
    add_suit(&mut ret, sum_tiles(&tiles[9..2 * 9]), len_div3);
    add_suit(&mut ret, sum_tiles(&tiles[2 * 9..3 * 9]), len_div3);
    add_honour(&mut ret, sum_tiles(&tiles[3 * 9..]), len_div3);

    (ret[5 + len_div3] as i8) - 1
}

#[must_use]
pub fn calculate_seven_pairs(tiles: &[u8; 34]) -> i8 {
    let mut pairs = 0;
    let mut kinds = 0;
    tiles.iter().filter(|&&c| c > 0).for_each(|&c| {
        kinds += 1;
        if c >= 2 {
            pairs += 1;
        }
    });

    let redunct = 7_u8.saturating_sub(kinds) as i8;
    7 - pairs + redunct - 1
}

#[must_use]
pub fn calculate_thirteen_orphans(tiles: &[u8; 34]) -> i8 {
    let mut pairs = 0;
    let mut kinds = 0;

    tile_index![1m, 9m, 1p, 9p, 1s, 9s, E, S, W, N, P, F, C]
        .iter()
        .map(|&i| tiles[i])
        .filter(|&c| c > 0)
        .for_each(|c| {
            kinds += 1;
            if c >= 2 {
                pairs += 1;
            }
        });

    let has_pair = (pairs > 0) as i8;
    14 - kinds - has_pair - 1
}

#[must_use]
pub fn calculate(tiles: &[u8; 34], len_div3: u8) -> i8 {
    let mut shanten = calculate_normal(tiles, len_div3);
    if shanten <= 0 || len_div3 < 4 {
        return shanten;
    }

    shanten = shanten.min(calculate_seven_pairs(tiles));
    if shanten > 0 {
        shanten.min(calculate_thirteen_orphans(tiles))
    } else {
        shanten
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(s: &str) -> [u8; 34] {
        let mut ret = [0; 34];
        let mut digits: Vec<usize> = Vec::new();
        for b in s.as_bytes() {
            match b {
                b'0'..=b'9' => digits.push((b - b'0') as usize),
                b'm' | b'p' | b's' | b'z' => {
                    let kind = match b {
                        b'm' => 0,
                        b'p' => 1,
                        b's' => 2,
                        b'z' => 3,
                        _ => unreachable!(),
                    };
                    for &t in &digits {
                        let idx = if t == 0 {
                            match b {
                                b'm' => tile_index!(5mr),
                                b'p' => tile_index!(5pr),
                                b's' => tile_index!(5sr),
                                _ => panic!("unexpected 0 with {b}"),
                            }
                        } else {
                            kind * 9 + t - 1
                        };
                        if idx < 34 {
                            ret[idx] += 1;
                        }
                    }
                    digits.clear();
                }
                b' ' | b'\t' | b'\n' => {}
                _ => panic!("unexpected byte: {b}"),
            }
        }
        ret
    }

    #[test]
    fn test_3n_plus_1() {
        assert_eq!(calculate(&counts("1111m 333p 222s 444z"), 4), 1);
        assert_eq!(calculate(&counts("147m 258p 369s 1234z"), 4), 6);
        assert_eq!(calculate(&counts("468m 33346p 7s"), 3), 2);
        assert_eq!(calculate(&counts("147m 258p 3s"), 2), 4);
        assert_eq!(calculate(&counts("4455s"), 1), 0);
        assert_eq!(calculate(&counts("7z"), 0), 0);
        assert_eq!(calculate(&counts("15559m 19p 19s 1234z"), 4), 3);
        assert_eq!(calculate(&counts("9999m 6677p 88s 355z"), 4), 2);
        assert_eq!(calculate(&counts("19m 19p 159s 123456z"), 4), 1);
    }

    #[test]
    fn test_3n_plus_2() {
        assert_eq!(calculate(&counts("2344456m 14p 127s 2z 7p"), 4), 3);
        assert_eq!(calculate(&counts("2344456m 14p 127s 2z 5p"), 4), 2);
        assert_eq!(calculate(&counts("344455667p 1139s 9m"), 4), 2);
        assert_eq!(calculate(&counts("344455667p 1139s 9p"), 4), 1);
        assert_eq!(calculate(&counts("122334m 678p 37s 22z 5s"), 4), 0);
        assert_eq!(calculate(&counts("122334m 678p 12s 22z 4s"), 4), 0);
        assert_eq!(calculate(&counts("12223456m 78889p 2m"), 4), -1);
        assert_eq!(calculate(&counts("34778p"), 1), 0);
        assert_eq!(calculate(&counts("34s"), 0), 0);
        assert_eq!(calculate(&counts("55m"), 0), -1);
    }

    #[test]
    fn test_seven_pairs() {
        assert_eq!(
            calculate_seven_pairs(&counts("11m 33p 55s 11z 77z 55m 22p")),
            -1
        );
        assert_eq!(calculate_seven_pairs(&counts("112233m 44556z")), 2);
    }

    #[test]
    fn test_thirteen_orphans() {
        assert_eq!(
            calculate_thirteen_orphans(&counts("19m 19p 19s 1234567z")),
            0
        );
        assert_eq!(
            calculate_thirteen_orphans(&counts("19m 19p 19s 1234567z 1m")),
            -1
        );
    }

    #[test]
    fn test_complete_hand() {
        assert_eq!(calculate(&counts("122334m 678p 12s 22z 4s"), 4), 0);
    }
}
