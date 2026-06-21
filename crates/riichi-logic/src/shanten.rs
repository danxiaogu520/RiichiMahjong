use riichi_core::tile::Tile;
use crate::types::TileCounts;

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

