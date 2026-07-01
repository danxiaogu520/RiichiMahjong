#![allow(clippy::manual_range_patterns)]

use super::point::Payout;
use super::shanten;
use riichi_core::tile37::Tile37;
use riichi_core::{tile_id, tile_matches, tile_unchecked};
use std::collections::HashMap;
use std::iter;
use std::sync::LazyLock;

use byteorder::{LittleEndian, ReadBytesExt};
use flate2::read::GzDecoder;

const TABLE_SIZE: usize = 9_362;

static HAND_TABLE: LazyLock<HashMap<u32, Vec<HandDivision>>> = LazyLock::new(|| {
    let mut raw = GzDecoder::new(include_bytes!("data/agari.bin.gz").as_slice());

    let (keys, values): (Vec<_>, Vec<_>) = (0..TABLE_SIZE)
        .map(|_| {
            let key = raw.read_u32::<LittleEndian>().unwrap();
            let v_size = raw.read_u8().unwrap();
            let value = (0..v_size)
                .map(|_| raw.read_u32::<LittleEndian>().unwrap())
                .map(HandDivision::from)
                .collect();
            (key, value)
        })
        .unzip();

    keys.into_iter().zip(values).collect()
});

#[derive(Debug, Default, Clone)]
struct HandDivision {
    pair_pos: u8,
    triplet_positions: Vec<u8>,
    sequence_positions: Vec<u8>,
    has_seven_pairs: bool,
    has_nine_gates: bool,
    has_full_straight: bool,
    has_twice_pure_double_sequence: bool,
    has_pure_double_sequence: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum HandScore {
    Basic { fu: u8, han: u8 },
    Yakuman(u8),
}

pub struct HandEvalContext<'a> {
    pub hand: &'a [u8; 34],
    pub is_concealed: bool,
    pub chis: &'a [u8],
    pub pons: &'a [u8],
    pub minkans: &'a [u8],
    pub ankans: &'a [u8],

    pub table_wind: u8,
    pub seat_wind: u8,

    pub winning_tile: u8,
    pub is_ron: bool,
}

struct DivisionScorer<'a> {
    ctx: &'a HandEvalContext<'a>,
    tile14: &'a [u8; 14],
    div: &'a HandDivision,
    pair_tile: u8,
    concealed_triplets: Vec<u8>,
    concealed_sequences: Vec<u8>,
    winning_tile_makes_open_triplet: bool,
}

impl From<u32> for HandDivision {
    fn from(v: u32) -> Self {
        let pair_pos = ((v >> 6) & 0b1111) as u8;

        let triplet_count = v & 0b111;
        let triplet_positions = (0..triplet_count)
            .map(|i| ((v >> (10 + i * 4)) & 0b1111) as u8)
            .collect();

        let sequence_count = (v >> 3) & 0b111;
        let sequence_positions = (triplet_count..triplet_count + sequence_count)
            .map(|i| ((v >> (10 + i * 4)) & 0b1111) as u8)
            .collect();

        Self {
            pair_pos,
            triplet_positions,
            sequence_positions,
            has_seven_pairs: (v >> 26) & 0b1 == 0b1,
            has_nine_gates: (v >> 27) & 0b1 == 0b1,
            has_full_straight: (v >> 28) & 0b1 == 0b1,
            has_twice_pure_double_sequence: (v >> 29) & 0b1 == 0b1,
            has_pure_double_sequence: (v >> 30) & 0b1 == 0b1,
        }
    }
}

impl PartialOrd for HandScore {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HandScore {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Self::Yakuman(l), Self::Yakuman(r)) => l.cmp(r),
            (Self::Yakuman(_), Self::Basic { .. }) => std::cmp::Ordering::Greater,
            (Self::Basic { .. }, Self::Yakuman(_)) => std::cmp::Ordering::Less,
            (Self::Basic { fu: lf, han: lh }, Self::Basic { fu: rf, han: rh }) => {
                match lh.cmp(rh) {
                    std::cmp::Ordering::Equal => lf.cmp(rf),
                    v => v,
                }
            }
        }
    }
}

impl HandScore {
    #[must_use]
    pub fn payout(self, is_dealer: bool) -> Payout {
        match self {
            Self::Basic { fu, han } => Payout::lookup(is_dealer, fu, han),
            Self::Yakuman(n) => Payout::yakuman(is_dealer, n as i32),
        }
    }
}

impl HandEvalContext<'_> {
    #[inline]
    #[must_use]
    pub fn has_yaku(&self) -> bool {
        self.evaluate(true).is_some()
    }

    #[inline]
    #[must_use]
    pub fn search_yaku(&self) -> Option<HandScore> {
        self.evaluate(false)
    }

    pub fn evaluate_full(&self, additional_han: u8, dora_count: u8) -> Option<HandScore> {
        if let Some(score) = self.search_yaku() {
            Some(match score {
                HandScore::Basic { fu, han } => HandScore::Basic {
                    fu,
                    han: han + additional_han + dora_count,
                },
                _ => score,
            })
        } else if additional_han == 0 {
            None
        } else if additional_han + dora_count >= 5 {
            Some(HandScore::Basic {
                fu: 0,
                han: additional_han + dora_count,
            })
        } else {
            let (tile14, key) = flatten_hand(self.hand);
            let divs = HAND_TABLE.get(&key)?;

            let fu = divs
                .iter()
                .map(|div| DivisionScorer::new(self, &tile14, div))
                .map(|w| w.calculate_fu(false))
                .max()?;
            Some(HandScore::Basic {
                fu,
                han: additional_han + dora_count,
            })
        }
    }

    fn evaluate(&self, return_if_any: bool) -> Option<HandScore> {
        assert_eq!(
            self.is_concealed,
            self.chis.is_empty() && self.pons.is_empty() && self.minkans.is_empty(),
        );

        if self.is_concealed && shanten::calculate_thirteen_orphans(self.hand) == -1 {
            return Some(HandScore::Yakuman(1));
        }

        let (tile14, key) = flatten_hand(self.hand);
        let divs = HAND_TABLE.get(&key)?;

        if return_if_any {
            divs.iter()
                .map(|div| DivisionScorer::new(self, &tile14, div))
                .find_map(|w| Self::score_one::<true>(&w))
        } else {
            divs.iter()
                .map(|div| DivisionScorer::new(self, &tile14, div))
                .filter_map(|w| Self::score_one::<false>(&w))
                .max()
        }
    }

    fn score_one<const RETURN_IF_ANY: bool>(w: &DivisionScorer) -> Option<HandScore> {
        w.search_yaku::<RETURN_IF_ANY>()
    }
}

impl<'a> DivisionScorer<'a> {
    fn new(ctx: &'a HandEvalContext<'a>, tile14: &'a [u8; 14], div: &'a HandDivision) -> Self {
        let pair_tile = tile14[div.pair_pos as usize];
        let concealed_triplets = div
            .triplet_positions
            .iter()
            .map(|&i| tile14[i as usize])
            .collect();
        let concealed_sequences = div
            .sequence_positions
            .iter()
            .map(|&i| tile14[i as usize])
            .collect();

        let mut ret = Self {
            ctx,
            tile14,
            div,
            pair_tile,
            concealed_triplets,
            concealed_sequences,
            winning_tile_makes_open_triplet: false,
        };
        ret.winning_tile_makes_open_triplet = ret.detect_open_triplet();
        ret
    }

    fn detect_open_triplet(&self) -> bool {
        if !self.ctx.is_ron {
            return false;
        }
        if !self.concealed_triplets.contains(&self.ctx.winning_tile) {
            return false;
        }
        if self.ctx.winning_tile >= 3 * 9 {
            return true;
        }
        let kind = self.ctx.winning_tile / 9;
        let num = self.ctx.winning_tile % 9;
        let low = kind * 9 + num.saturating_sub(2);
        let high = kind * 9 + num.min(6);
        !(low..=high).any(|t| self.concealed_sequences.contains(&t))
    }

    fn seven_pairs_tiles(&self) -> impl Iterator<Item = u8> + '_ {
        self.tile14.iter().take(7).copied()
    }

    fn all_triplets_and_kans(&self) -> impl Iterator<Item = u8> + '_ {
        self.concealed_triplets
            .iter()
            .chain(self.ctx.pons)
            .chain(self.ctx.minkans)
            .chain(self.ctx.ankans)
            .copied()
    }

    fn all_sequences(&self) -> impl Iterator<Item = u8> + '_ {
        self.concealed_sequences
            .iter()
            .chain(self.ctx.chis)
            .copied()
    }

    fn all_melds(&self) -> impl Iterator<Item = u8> + '_ {
        self.all_triplets_and_kans().chain(self.all_sequences())
    }

    fn calculate_fu(&self, has_pinfu: bool) -> u8 {
        if self.div.has_seven_pairs {
            return 25;
        }
        let mut fu = 20;

        fu += self
            .concealed_triplets
            .iter()
            .map(|&t| {
                let is_open = self.winning_tile_makes_open_triplet && t == self.ctx.winning_tile;
                match (is_open, tile_unchecked!(t).is_terminal_or_honour()) {
                    (false, true) => 8,
                    (false, false) | (true, true) => 4,
                    (true, false) => 2,
                }
            })
            .sum::<u8>();
        fu += self
            .ctx
            .pons
            .iter()
            .map(|&t| {
                if tile_unchecked!(t).is_terminal_or_honour() {
                    4
                } else {
                    2
                }
            })
            .sum::<u8>();
        fu += self
            .ctx
            .ankans
            .iter()
            .map(|&t| {
                if tile_unchecked!(t).is_terminal_or_honour() {
                    32
                } else {
                    16
                }
            })
            .sum::<u8>();
        fu += self
            .ctx
            .minkans
            .iter()
            .map(|&t| {
                if tile_unchecked!(t).is_terminal_or_honour() {
                    16
                } else {
                    8
                }
            })
            .sum::<u8>();

        if tile_matches!(self.pair_tile, P | F | C) {
            fu += 2;
        } else {
            if self.pair_tile == self.ctx.table_wind {
                fu += 2;
            }
            if self.pair_tile == self.ctx.seat_wind {
                fu += 2;
            }
        }

        if fu == 20 {
            return if !self.ctx.is_concealed {
                30
            } else if has_pinfu {
                if self.ctx.is_ron {
                    30
                } else {
                    20
                }
            } else if self.ctx.is_ron {
                40
            } else {
                30
            };
        }

        if !self.ctx.is_ron {
            fu += 2;
        } else if self.ctx.is_concealed {
            fu += 10;
        }

        if !self.winning_tile_makes_open_triplet {
            if self.pair_tile == self.ctx.winning_tile {
                fu += 2;
            } else {
                let is_edge_or_closed_wait = self.concealed_sequences.iter().any(|&s| {
                    s + 1 == self.ctx.winning_tile
                        || s % 9 == 0 && s + 2 == self.ctx.winning_tile
                        || s % 9 == 6 && s == self.ctx.winning_tile
                });
                if is_edge_or_closed_wait {
                    fu += 2;
                }
            }
        }

        ((fu - 1) / 10 + 1) * 10
    }

    fn search_yaku<const RETURN_IF_ANY: bool>(&self) -> Option<HandScore> {
        let mut han = 0;
        let mut yakuman = 0;

        let has_pinfu = self.concealed_sequences.len() == 4
            && !tile_matches!(self.pair_tile, P | F | C)
            && self.pair_tile != self.ctx.table_wind
            && self.pair_tile != self.ctx.seat_wind
            && self.concealed_sequences.iter().any(|&s| {
                let num = s % 9 + 1;
                num <= 6 && s == self.ctx.winning_tile || num >= 2 && s + 2 == self.ctx.winning_tile
            });

        macro_rules! make_return {
            () => {
                return if yakuman > 0 {
                    Some(HandScore::Yakuman(yakuman))
                } else if han > 0 {
                    let fu = if RETURN_IF_ANY || han >= 5 {
                        0
                    } else {
                        self.calculate_fu(has_pinfu)
                    };
                    Some(HandScore::Basic { fu, han })
                } else {
                    None
                };
            };
        }

        macro_rules! check_early {
            ($($block:tt)*) => {{
                $($block)*
                if RETURN_IF_ANY {
                    make_return!();
                }
            }};
        }

        if has_pinfu {
            check_early! { han += 1; }
        }
        if self.div.has_seven_pairs {
            check_early! { han += 2; }
        }
        if self.div.has_twice_pure_double_sequence {
            check_early! { han += 3; }
        }
        if self.div.has_nine_gates {
            check_early! { yakuman += 1; }
        }

        let has_tanyao = if self.div.has_seven_pairs {
            self.seven_pairs_tiles().all(|t| {
                let kind = t / 9;
                let num = t % 9;
                kind < 3 && num > 0 && num < 8
            })
        } else {
            self.all_sequences().all(|s| {
                let num = s % 9;
                num > 0 && num < 6
            }) && self
                .all_triplets_and_kans()
                .chain(iter::once(self.pair_tile))
                .all(|k| {
                    let kind = k / 9;
                    let num = k % 9;
                    kind < 3 && num > 0 && num < 8
                })
        };
        if has_tanyao {
            check_early! { han += 1; }
        }

        let has_all_triplets = !self.div.has_seven_pairs
            && self.concealed_sequences.is_empty()
            && self.ctx.chis.is_empty();
        if has_all_triplets {
            check_early! { han += 2; }
        }

        let mut suit_kind = None;
        let mut has_honour_seen = false;
        let mut is_flush = true;
        let iter_fn = |&m: &u8| {
            let kind = m / 9;
            if kind >= 3 {
                has_honour_seen = true;
                return true;
            }
            if let Some(prev) = suit_kind {
                if prev != kind {
                    is_flush = false;
                    return false;
                }
            } else {
                suit_kind = Some(kind);
            }
            true
        };
        if self.div.has_seven_pairs {
            self.seven_pairs_tiles().take_while(iter_fn).for_each(drop);
        } else {
            self.all_melds()
                .chain(iter::once(self.pair_tile))
                .take_while(iter_fn)
                .for_each(drop);
        }
        if suit_kind.is_none() {
            check_early! { yakuman += 1; }
        } else if is_flush {
            let n = if has_honour_seen { 2 } else { 5 } + self.ctx.is_concealed as u8;
            check_early! { han += n; }
        }

        if !self.div.has_seven_pairs {
            if self.div.has_pure_double_sequence {
                check_early! { han += 1; }
            } else if !self.ctx.ankans.is_empty()
                && self.ctx.is_concealed
                && self.concealed_sequences.len() >= 2
            {
                let mut seq_marks = [0_u8; 3];
                let has_ipeikou = self.concealed_sequences.iter().any(|&t| {
                    let kind = t as usize / 9;
                    let num = t % 9;
                    let mark = &mut seq_marks[kind];
                    if (*mark >> num) & 0b1 == 0b1 {
                        true
                    } else {
                        *mark |= 0b1 << num;
                        false
                    }
                });
                if has_ipeikou {
                    check_early! { han += 1; }
                }
            }

            if self.ctx.is_concealed && self.div.has_full_straight {
                check_early! { han += 2; }
            } else if self.ctx.chis.is_empty() && self.div.has_full_straight {
                check_early! { han += 1; }
            } else if self.concealed_sequences.len() + self.ctx.chis.len() >= 3 {
                let mut kinds = [0; 3];
                for s in self.all_sequences() {
                    let kind = s as usize / 9;
                    let num = s % 9;
                    match num {
                        0 => kinds[kind] |= 0b001,
                        3 => kinds[kind] |= 0b010,
                        6 => kinds[kind] |= 0b100,
                        _ => (),
                    };
                }
                if kinds.contains(&0b111) {
                    check_early! { han += 1; }
                }
            }

            let mut seq_counter = [0; 9];
            for s in self.all_sequences() {
                let kind = s / 9;
                let num = s % 9;
                seq_counter[num as usize] |= 0b1 << kind;
            }
            if seq_counter.contains(&0b111) {
                let n = if self.ctx.is_concealed { 2 } else { 1 };
                check_early! { han += n; }
            } else {
                let mut trip_counter = [0; 9];
                for k in self.all_triplets_and_kans() {
                    let kind = k / 9;
                    if kind < 3 {
                        let num = k % 9;
                        trip_counter[num as usize] |= 1 << kind;
                    }
                }
                if trip_counter.contains(&0b111) {
                    check_early! { han += 2; }
                }
            }

            let concealed_triplet_count = self.ctx.ankans.len() + self.concealed_triplets.len()
                - self.winning_tile_makes_open_triplet as usize;
            match concealed_triplet_count {
                4 => check_early! { yakuman += 1; },
                3 => check_early! { han += 2; },
                _ => (),
            };

            let kan_count = self.ctx.ankans.len() + self.ctx.minkans.len();
            match kan_count {
                4 => check_early! { yakuman += 1; },
                3 => check_early! { han += 2; },
                _ => (),
            };

            let has_all_green = self
                .all_triplets_and_kans()
                .chain(iter::once(self.pair_tile))
                .all(|k| tile_matches!(k, 2s | 3s | 4s | 6s | 8s | F))
                && self.all_sequences().all(|s| s == tile_id!(2s));
            if has_all_green {
                check_early! { yakuman += 1; }
            }

            if !has_tanyao {
                let mut has_jihai = [false; 7];
                for k in self.all_triplets_and_kans() {
                    if k >= 3 * 9 {
                        has_jihai[k as usize - 3 * 9] = true;
                    }
                }
                if has_jihai[self.ctx.table_wind as usize - 3 * 9] {
                    check_early! { han += 1; }
                }
                if has_jihai[self.ctx.seat_wind as usize - 3 * 9] {
                    check_early! { han += 1; }
                }

                let dragons = (4..7).filter(|&i| has_jihai[i]).count() as u8;
                if dragons > 0 {
                    check_early! { han += dragons; }
                    if dragons == 3 {
                        check_early! { yakuman += 1; }
                    } else if dragons == 2 && tile_matches!(self.pair_tile, P | F | C) {
                        check_early! { han += 2; }
                    }
                }

                let winds = (0..4).filter(|&i| has_jihai[i]).count();
                #[allow(clippy::if_same_then_else)]
                if winds == 4 {
                    check_early! { yakuman += 1; }
                } else if winds == 3 && tile_matches!(self.pair_tile, E | S | W | N) {
                    check_early! { yakuman += 1; }
                }
            }
        }

        if !has_tanyao {
            let mut has_honour = false;
            let is_terminal = |k| {
                let kind = k / 9;
                if kind >= 3 {
                    has_honour = true;
                    true
                } else {
                    let num = k % 9;
                    num == 0 || num == 8
                }
            };
            let is_terminal_hand = if self.div.has_seven_pairs {
                self.seven_pairs_tiles().all(is_terminal)
            } else {
                self.all_triplets_and_kans()
                    .chain(iter::once(self.pair_tile))
                    .all(is_terminal)
            };
            if is_terminal_hand {
                if self.div.has_seven_pairs || has_all_triplets {
                    if has_honour {
                        check_early! { han += 2; }
                    } else {
                        check_early! { yakuman += 1; }
                    }
                } else {
                    let is_junchan_or_chanta = self.all_sequences().all(|s| {
                        let num = s % 9;
                        num == 0 || num == 6
                    });
                    if is_junchan_or_chanta {
                        let n = if has_honour { 1 } else { 2 } + self.ctx.is_concealed as u8;
                        check_early! { han += n; }
                    }
                }
            }
        }

        make_return!();
    }
}

pub fn ensure_init() {
    assert_eq!(HAND_TABLE.len(), TABLE_SIZE);
}

fn flatten_hand(tiles: &[u8; 34]) -> ([u8; 14], u32) {
    let mut tile14 = [0; 14];
    let mut tile14_iter = tile14.iter_mut();
    let mut key = 0;

    let mut bit_pos: i8 = -1;
    let mut prev_present = None;
    for (kind, chunk) in tiles.chunks_exact(9).enumerate() {
        for (num, c) in chunk.iter().copied().enumerate() {
            if c > 0 {
                prev_present = Some(());
                *tile14_iter.next().unwrap() = (kind * 9 + num) as u8;
                bit_pos += 1;
                match c {
                    2 => {
                        key |= 0b11 << bit_pos;
                        bit_pos += 2;
                    }
                    3 => {
                        key |= 0b1111 << bit_pos;
                        bit_pos += 4;
                    }
                    4 => {
                        key |= 0b11_1111 << bit_pos;
                        bit_pos += 6;
                    }
                    _ => (),
                }
            } else if prev_present.take().is_some() {
                key |= 0b1 << bit_pos;
                bit_pos += 1;
            }
        }
        if prev_present.take().is_some() {
            key |= 0b1 << bit_pos;
            bit_pos += 1;
        }
    }

    tiles
        .iter()
        .enumerate()
        .skip(3 * 9)
        .filter(|&(_, &c)| c > 0)
        .for_each(|(tile_id_val, &c)| {
            *tile14_iter.next().unwrap() = tile_id_val as u8;
            bit_pos += 1;
            match c {
                2 => {
                    key |= 0b11 << bit_pos;
                    bit_pos += 2;
                }
                3 => {
                    key |= 0b1111 << bit_pos;
                    bit_pos += 4;
                }
                4 => {
                    key |= 0b11_1111 << bit_pos;
                    bit_pos += 6;
                }
                _ => (),
            }
            key |= 0b1 << bit_pos;
            bit_pos += 1;
        });

    (tile14, key)
}

#[must_use]
pub fn check_ankan_after_riichi(
    tiles: &[u8; 34],
    len_div3: u8,
    tile: Tile37,
    strict: bool,
) -> bool {
    let tile_id_val = tile.regular().index();
    if tiles[tile_id_val] != 4 {
        return false;
    }
    if tile_id_val >= 3 * 9 {
        return true;
    }

    let mut tehai_before = *tiles;
    tehai_before[tile_id_val] -= 1;

    (0..34)
        .filter(|&t| {
            if tehai_before[t] == 4 {
                return false;
            }
            let mut tmp = tehai_before;
            tmp[t] += 1;
            shanten::calculate(&tmp, len_div3) == -1
        })
        .all(|wait| {
            if wait == tile_id_val {
                return false;
            }
            let mut tehai_after = *tiles;
            tehai_after[tile_id_val] = 0;
            tehai_after[wait] += 1;
            let (_, key) = flatten_hand(&tehai_after);
            let Some(divs_after) = HAND_TABLE.get(&key) else {
                return false;
            };

            if strict {
                let mut tehai_before_with_wait = tehai_before;
                tehai_before_with_wait[wait] += 1;
                let (_, key) = flatten_hand(&tehai_before_with_wait);
                let divs_before = HAND_TABLE
                    .get(&key)
                    .expect("invalid riichi detected when testing ankan after riichi");
                if divs_after.len() != divs_before.len() {
                    return false;
                }
            }
            true
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use riichi_core::{tile_id, tile_index};

    fn make_counts(s: &str) -> [u8; 34] {
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
    fn test_flatten_hand() {
        let counts = make_counts("122334m 678p 12s 22z 4s");
        let (tile14, _key) = flatten_hand(&counts);
        assert!(!tile14.is_empty());
    }

    #[test]
    fn test_tanyao_hand() {
        let counts = make_counts("223344m 667788s 33m");
        let ctx = HandEvalContext {
            hand: &counts,
            is_concealed: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &[],
            table_wind: tile_id!(S),
            seat_wind: tile_id!(N),
            winning_tile: tile_id!(3m),
            is_ron: false,
        };
        let score = ctx.search_yaku().unwrap();
        assert_eq!(score, HandScore::Basic { fu: 30, han: 4 });
    }

    #[test]
    fn test_no_yaku() {
        let counts = make_counts("234678m 1123488p 8p");
        let ctx = HandEvalContext {
            hand: &counts,
            is_concealed: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &[],
            table_wind: tile_id!(E),
            seat_wind: tile_id!(E),
            winning_tile: tile_id!(8p),
            is_ron: true,
        };
        assert_eq!(ctx.search_yaku(), None);
    }

    #[test]
    fn test_yakuman_kokushi() {
        let counts = make_counts("19m 19p 19s 1234567z 1m");
        let ctx = HandEvalContext {
            hand: &counts,
            is_concealed: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &[],
            table_wind: tile_id!(E),
            seat_wind: tile_id!(E),
            winning_tile: tile_id!(1m),
            is_ron: true,
        };
        assert_eq!(ctx.search_yaku(), Some(HandScore::Yakuman(1)));
    }

    #[test]
    fn test_has_yaku() {
        let counts = make_counts("223344m 667788s 33m");
        let ctx = HandEvalContext {
            hand: &counts,
            is_concealed: true,
            chis: &[],
            pons: &[],
            minkans: &[],
            ankans: &[],
            table_wind: tile_id!(S),
            seat_wind: tile_id!(N),
            winning_tile: tile_id!(3m),
            is_ron: false,
        };
        assert!(ctx.has_yaku());
    }
}
