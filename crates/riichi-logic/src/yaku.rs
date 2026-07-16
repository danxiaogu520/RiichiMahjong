use riichi_core::meld::{Meld, MeldKind};
use riichi_core::tile::{Suit, Tile, TileType};

use crate::model::{
    MentsuKind, WinSituation, WinningHand, WinningTilePlacement, YakuName, YakuResult,
};

struct YakuContext<'a> {
    situation: &'a WinSituation,
    melds: &'a [Meld],
}

impl std::ops::Deref for YakuContext<'_> {
    type Target = WinSituation;

    fn deref(&self) -> &Self::Target {
        self.situation
    }
}

pub(crate) fn detect_yaku(
    hand: &WinningHand,
    situation: &WinSituation,
    melds: &[Meld],
    winning_tile: Tile,
    placement: WinningTilePlacement,
) -> Vec<YakuResult> {
    detect_yaku_inner(
        hand,
        &YakuContext { situation, melds },
        winning_tile,
        placement,
    )
}

fn suit_base(suit: Suit) -> u8 {
    match suit {
        Suit::Man => 0,
        Suit::Pin => 9,
        Suit::Sou => 18,
        _ => 0,
    }
}

fn count_kans(melds: &[Meld]) -> usize {
    melds
        .iter()
        .filter(|m| matches!(m.kind, MeldKind::Ankan | MeldKind::Minkan | MeldKind::Kakan))
        .count()
}

fn count_open_triplets(melds: &[Meld]) -> usize {
    melds
        .iter()
        .filter(|m| matches!(m.kind, MeldKind::Pon | MeldKind::Minkan | MeldKind::Kakan))
        .count()
}

fn count_concealed_triplets(melds: &[Meld]) -> usize {
    melds.iter().filter(|m| m.kind == MeldKind::Ankan).count()
}

fn all_honor(t: TileType) -> bool {
    t.is_honor()
}

fn all_terminal(t: TileType) -> bool {
    t.is_terminal()
}

fn all_terminal_or_honor(t: TileType) -> bool {
    t.is_yaochuuhai()
}

fn all_simple(t: TileType) -> bool {
    t.is_number() && !t.is_terminal()
}

fn is_green_tile(t: TileType) -> bool {
    matches!(t.0, 19 | 20 | 21 | 23 | 25 | 32)
}

fn is_yakuhai(t: TileType, ctx: &YakuContext<'_>) -> bool {
    t.is_dragon() || t == ctx.seat_wind || t == ctx.field_wind
}

fn has_open_sequence(melds: &[Meld]) -> bool {
    melds.iter().any(|m| m.kind == MeldKind::Chi)
}

fn meld_sequence_start(meld: &Meld) -> Option<TileType> {
    if meld.kind != MeldKind::Chi {
        return None;
    }
    meld.tiles
        .iter()
        .map(|tile| tile.tile_type())
        .min_by_key(|tile| tile.0)
}

fn has_sequence_start(hand: &WinningHand, melds: &[Meld], start: TileType) -> bool {
    hand.groups()
        .iter()
        .any(|m| m.kind == MentsuKind::Shuntsu && m.tile_type == start)
        || melds
            .iter()
            .filter_map(meld_sequence_start)
            .any(|meld_start| meld_start == start)
}

fn meld_has_yaochuuhai(meld: &Meld) -> bool {
    match meld.kind {
        MeldKind::Chi => meld_sequence_start(meld)
            .is_some_and(|start| start.is_terminal() || TileType(start.0 + 2).is_terminal()),
        MeldKind::Pon | MeldKind::Minkan | MeldKind::Kakan | MeldKind::Ankan => meld
            .tiles
            .first()
            .is_some_and(|tile| tile.tile_type().is_yaochuuhai()),
    }
}

fn meld_has_terminal(meld: &Meld) -> bool {
    match meld.kind {
        MeldKind::Chi => meld_sequence_start(meld)
            .is_some_and(|start| start.is_terminal() || TileType(start.0 + 2).is_terminal()),
        MeldKind::Pon | MeldKind::Minkan | MeldKind::Kakan | MeldKind::Ankan => meld
            .tiles
            .first()
            .is_some_and(|tile| tile.tile_type().is_terminal()),
    }
}

fn meld_triplet_tile(meld: &Meld) -> Option<TileType> {
    matches!(
        meld.kind,
        MeldKind::Pon | MeldKind::Minkan | MeldKind::Kakan | MeldKind::Ankan
    )
    .then(|| meld.tiles.first().map(|tile| tile.tile_type()))
    .flatten()
}

fn is_pinfu_wait(hand: &WinningHand, winning_tile: Tile, placement: WinningTilePlacement) -> bool {
    let winning_type = winning_tile.tile_type();
    let WinningTilePlacement::Group(index) = placement else {
        return false;
    };
    let Some(group) = hand.groups().get(index) else {
        return false;
    };
    if group.kind != MentsuKind::Shuntsu || group.tile_type.suit() != winning_type.suit() {
        return false;
    }
    let start = group.tile_type.rank().0;
    let win_rank = winning_type.rank().0;
    // 23 听 1/4、34 听 2/5……；12 听 3 和 78 听 7 属边张，不能算平和。
    (win_rank == start && (1..=6).contains(&start))
        || (win_rank == start + 2 && (2..=7).contains(&start))
}

fn collect_hand_tiles(hand: &WinningHand) -> Vec<TileType> {
    hand.all_tiles()
}

fn collect_all_tiles(hand: &WinningHand, melds: &[Meld]) -> Vec<TileType> {
    let mut tiles = collect_hand_tiles(hand);
    for m in melds {
        tiles.extend(m.tiles.iter().map(|t| t.tile_type()));
    }
    tiles
}

fn detect_yaku_inner(
    hand: &WinningHand,
    ctx: &YakuContext<'_>,
    winning_tile: Tile,
    placement: WinningTilePlacement,
) -> Vec<YakuResult> {
    let open_triplets = count_open_triplets(ctx.melds);
    let concealed_triplets_melds = count_concealed_triplets(ctx.melds);

    let all_tiles = collect_all_tiles(hand, ctx.melds);
    let mut yaku = Vec::new();

    let num_koutsu = hand
        .groups()
        .iter()
        .filter(|m| m.kind == MentsuKind::Koutsu)
        .count();
    let num_shuntsu = hand
        .groups()
        .iter()
        .filter(|m| m.kind == MentsuKind::Shuntsu)
        .count();
    let is_menzen = ctx.melds.iter().all(|m| m.is_concealed());

    let koutsu_in_hand = num_koutsu + concealed_triplets_melds;
    let total_koutsu = koutsu_in_hand + open_triplets;
    let total_kans = count_kans(ctx.melds);

    let winning_tile_makes_minkou = !ctx.is_tsumo
        && matches!(
            placement,
            WinningTilePlacement::Group(index)
                if hand.groups().get(index).is_some_and(|group|
                    group.kind == MentsuKind::Koutsu
                        && group.tile_type == winning_tile.tile_type())
        );
    let mut concealed_triplet_count = koutsu_in_hand;
    if winning_tile_makes_minkou {
        concealed_triplet_count = concealed_triplet_count.saturating_sub(1);
    }

    if ctx.is_tenhou {
        yaku.push(YakuResult::new(YakuName::Tenhou, 13));
    } else if ctx.is_chiihou {
        yaku.push(YakuResult::new(YakuName::Chiihou, 13));
    }

    // Mortal 将天和/地和作为独立的特殊役满直接返回，不再叠加
    // 牌型役满或普通役。
    if ctx.is_tenhou || ctx.is_chiihou {
        return yaku;
    }

    if matches!(hand, WinningHand::Kokushi { .. }) {
        // 十三面只能在“和了牌正好把唯一缺少的重复牌补成雀头”时成立。
        // 不能因为牌型中存在若干张单张幺九牌就直接判成十三面。
        let winning_type = winning_tile.tile_type();
        let thirteen_wait = ctx.melds.is_empty()
            && winning_type.is_yaochuuhai()
            && TileType::YAOCHUUHAI.iter().all(|&tt| {
                let count = all_tiles.iter().filter(|&&tile| tile == tt).count();
                if tt == winning_type {
                    count == 2
                } else {
                    count == 1
                }
            });
        if thirteen_wait {
            yaku.push(YakuResult::new(YakuName::Kokushi13, 26));
        } else {
            yaku.push(YakuResult::new(YakuName::Kokushi, 13));
        }
        return yaku;
    }

    if is_menzen && ctx.melds.is_empty() && matches!(hand, WinningHand::Standard { .. }) {
        let suit = hand.all_tiles().first().map(|t| t.suit());
        if let Some(s) = suit {
            let required = [3usize, 1, 1, 1, 1, 1, 1, 1, 3];
            let counts: Vec<usize> = (0..9u8)
                .map(|i| {
                    let tt = TileType(suit_base(s) + i);
                    all_tiles.iter().filter(|&&tile| tile == tt).count()
                })
                .collect();
            let extra_count: usize = counts
                .iter()
                .zip(required.iter())
                .map(|(count, need)| count.saturating_sub(*need))
                .sum();
            let ok = all_tiles.len() == 14
                && counts
                    .iter()
                    .zip(required.iter())
                    .all(|(count, need)| *count >= *need)
                && extra_count == 1;
            if ok {
                let winning_index = winning_tile.tile_type().0.saturating_sub(suit_base(s));
                let nine_wait = winning_tile.tile_type().suit() == s
                    && winning_index < 9
                    && counts[winning_index as usize].saturating_sub(1)
                        == required[winning_index as usize]
                    && counts.iter().enumerate().all(|(index, count)| {
                        let expected =
                            required[index] + usize::from(index == winning_index as usize);
                        *count == expected
                    });
                if nine_wait {
                    yaku.push(YakuResult::new(YakuName::ChuurenPoutou9, 26));
                } else {
                    yaku.push(YakuResult::new(YakuName::ChuurenPoutou, 13));
                }
                return yaku;
            }
        }
    }

    if matches!(hand, WinningHand::Standard { .. }) && concealed_triplet_count >= 4 {
        if placement == WinningTilePlacement::Pair && hand.pair() == winning_tile.tile_type() {
            yaku.push(YakuResult::new(YakuName::SuuankouTanki, 26));
        } else {
            yaku.push(YakuResult::new(YakuName::Suuankou, 13));
        }
    }

    let dragon_koutsu = (31..=33u8)
        .filter(|&i| {
            let tt = TileType(i);
            (matches!(hand, WinningHand::Standard { .. })
                && hand
                    .groups()
                    .iter()
                    .any(|m| m.tile_type == tt && m.kind == MentsuKind::Koutsu))
                || ctx.melds.iter().any(|m| {
                    m.tiles[0].tile_type() == tt
                        && matches!(
                            m.kind,
                            MeldKind::Pon | MeldKind::Minkan | MeldKind::Kakan | MeldKind::Ankan
                        )
                })
        })
        .count();
    if dragon_koutsu == 3 {
        yaku.push(YakuResult::new(YakuName::Daisangen, 13));
    }

    let wind_koutsu = (27..=30u8)
        .filter(|&i| {
            let tt = TileType(i);
            (matches!(hand, WinningHand::Standard { .. })
                && hand
                    .groups()
                    .iter()
                    .any(|m| m.tile_type == tt && m.kind == MentsuKind::Koutsu))
                || ctx.melds.iter().any(|m| {
                    m.tiles[0].tile_type() == tt
                        && matches!(
                            m.kind,
                            MeldKind::Pon | MeldKind::Minkan | MeldKind::Kakan | MeldKind::Ankan
                        )
                })
        })
        .count();
    if wind_koutsu == 4 {
        yaku.push(YakuResult::new(YakuName::Daisuushii, 26));
    } else if wind_koutsu == 3 && hand.pair().is_wind() {
        yaku.push(YakuResult::new(YakuName::Shousuushii, 13));
    }

    if all_tiles.iter().all(|&t| all_honor(t)) {
        yaku.push(YakuResult::new(YakuName::Tsuuiisou, 13));
    }

    if all_tiles.iter().all(|&t| is_green_tile(t)) {
        yaku.push(YakuResult::new(YakuName::Ryuuiisou, 13));
    }

    if all_tiles.iter().all(|&t| all_terminal(t)) {
        yaku.push(YakuResult::new(YakuName::Chinroutou, 13));
    }

    if total_kans == 4 {
        yaku.push(YakuResult::new(YakuName::Suukantsu, 13));
    }

    if ctx.is_tsumo && is_menzen {
        yaku.push(YakuResult::new(YakuName::MenzenTsumo, 1));
    }

    if ctx.is_double_riichi && is_menzen {
        yaku.push(YakuResult::new(YakuName::DoubleRiichi, 2));
    } else if ctx.is_riichi && is_menzen {
        yaku.push(YakuResult::new(YakuName::Riichi, 1));
    }

    if ctx.is_ippatsu && is_menzen {
        yaku.push(YakuResult::new(YakuName::Ippatsu, 1));
    }

    if all_tiles.iter().all(|&t| all_simple(t)) {
        yaku.push(YakuResult::new(YakuName::Tanyao, 1));
    }

    if is_menzen
        && matches!(hand, WinningHand::Standard { .. })
        && num_shuntsu == 4
        && !is_yakuhai(hand.pair(), ctx)
        && is_pinfu_wait(hand, winning_tile, placement)
    {
        yaku.push(YakuResult::new(YakuName::Pinfu, 1));
    }

    if is_menzen && matches!(hand, WinningHand::Standard { .. }) {
        let mut duplicate_sequence_pairs = 0usize;
        for i in 0..hand.groups().len() {
            if hand.groups()[i].kind != MentsuKind::Shuntsu {
                continue;
            }
            for j in (i + 1)..hand.groups().len() {
                if hand.groups()[j].kind == MentsuKind::Shuntsu
                    && hand.groups()[i].tile_type == hand.groups()[j].tile_type
                {
                    duplicate_sequence_pairs += 1;
                }
            }
        }
        // 两杯口已经覆盖一杯口，不能重复计番。
        if duplicate_sequence_pairs == 1 {
            yaku.push(YakuResult::new(YakuName::Iipeiko, 1));
        }
    }

    // 役牌必须来自刻子/杠子，雀头不能单独构成役牌。
    let mut yakuhai_triplets = Vec::new();
    for mentsu in hand.groups() {
        if matches!(hand, WinningHand::Standard { .. })
            && mentsu.kind == MentsuKind::Koutsu
            && !yakuhai_triplets.contains(&mentsu.tile_type)
        {
            yakuhai_triplets.push(mentsu.tile_type);
        }
    }
    for meld in ctx.melds {
        if matches!(
            meld.kind,
            MeldKind::Pon | MeldKind::Minkan | MeldKind::Kakan | MeldKind::Ankan
        ) {
            let tile_type = meld.tiles[0].tile_type();
            if !yakuhai_triplets.contains(&tile_type) {
                yakuhai_triplets.push(tile_type);
            }
        }
    }
    for tile_type in yakuhai_triplets {
        if is_yakuhai(tile_type, ctx) {
            if tile_type.is_dragon() {
                yaku.push(YakuResult::new(YakuName::YakuhaiSangen, 1));
            }
            if tile_type == ctx.seat_wind {
                yaku.push(YakuResult::new(YakuName::YakuhaiJikaze, 1));
            }
            if tile_type == ctx.field_wind {
                yaku.push(YakuResult::new(YakuName::YakuhaiBakaze, 1));
            }
        }
    }

    if ctx.is_rinshan {
        yaku.push(YakuResult::new(YakuName::RinshanKaihou, 1));
    }
    if ctx.is_chankan {
        yaku.push(YakuResult::new(YakuName::Chankan, 1));
    }
    if ctx.is_haitei {
        yaku.push(YakuResult::new(YakuName::Haitei, 1));
    }
    if ctx.is_houtei {
        yaku.push(YakuResult::new(YakuName::Houtei, 1));
    }

    if matches!(hand, WinningHand::SevenPairs { .. }) {
        yaku.push(YakuResult::new(YakuName::Chiitoitsu, 2));
    }

    if total_koutsu == 4 && matches!(hand, WinningHand::Standard { .. }) {
        yaku.push(YakuResult::new(YakuName::Toitoi, 2));
    }

    if matches!(hand, WinningHand::Standard { .. }) && concealed_triplet_count >= 3 {
        yaku.push(YakuResult::new(YakuName::Sananko, 2));
    }

    if total_kans == 3 {
        yaku.push(YakuResult::new(YakuName::Sankantsu, 2));
    }

    if matches!(hand, WinningHand::Standard { .. }) {
        let mut found = false;
        for i in 0..hand.groups().len() {
            if hand.groups()[i].kind != MentsuKind::Koutsu {
                continue;
            }
            let tt = hand.groups()[i].tile_type;
            if !tt.is_number() {
                continue;
            }
            let rank = tt.rank().0;
            let suit = tt.suit();
            for other_suit in [Suit::Man, Suit::Pin, Suit::Sou] {
                if other_suit == suit {
                    continue;
                }
                let other_tt = TileType(suit_base(other_suit) + rank - 1);
                let has_in_hand = hand
                    .groups()
                    .iter()
                    .any(|m| m.kind == MentsuKind::Koutsu && m.tile_type == other_tt);
                let has_in_meld = ctx.melds.iter().any(|m| {
                    m.tiles[0].tile_type() == other_tt
                        && matches!(
                            m.kind,
                            MeldKind::Pon | MeldKind::Minkan | MeldKind::Kakan | MeldKind::Ankan
                        )
                });
                if !has_in_hand && !has_in_meld {
                    continue;
                }
                let third_suit = [Suit::Man, Suit::Pin, Suit::Sou]
                    .iter()
                    .find(|&&s| s != suit && s != other_suit)
                    .unwrap();
                let third_tt = TileType(suit_base(*third_suit) + rank - 1);
                let has_third = hand
                    .groups()
                    .iter()
                    .any(|m| m.kind == MentsuKind::Koutsu && m.tile_type == third_tt)
                    || ctx.melds.iter().any(|m| {
                        m.tiles[0].tile_type() == third_tt
                            && matches!(
                                m.kind,
                                MeldKind::Pon
                                    | MeldKind::Minkan
                                    | MeldKind::Kakan
                                    | MeldKind::Ankan
                            )
                    });
                if has_third {
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
        if !found {
            for tt in ctx.melds.iter().filter_map(meld_triplet_tile) {
                if !tt.is_number() {
                    continue;
                }
                let rank = tt.rank().0;
                let suit = tt.suit();
                for other_suit in [Suit::Man, Suit::Pin, Suit::Sou] {
                    if other_suit == suit {
                        continue;
                    }
                    let other = TileType(suit_base(other_suit) + rank - 1);
                    let third_suit = [Suit::Man, Suit::Pin, Suit::Sou]
                        .iter()
                        .find(|&&candidate| candidate != suit && candidate != other_suit)
                        .unwrap();
                    let third = TileType(suit_base(*third_suit) + rank - 1);
                    let has_triplet = |target: TileType| {
                        hand.groups()
                            .iter()
                            .any(|m| m.kind == MentsuKind::Koutsu && m.tile_type == target)
                            || ctx
                                .melds
                                .iter()
                                .filter_map(meld_triplet_tile)
                                .any(|meld_tile| meld_tile == target)
                    };
                    if has_triplet(other) && has_triplet(third) {
                        found = true;
                        break;
                    }
                }
                if found {
                    break;
                }
            }
        }
        if found {
            yaku.push(YakuResult::new(YakuName::SanshokuDoukou, 2));
        }
    }

    if dragon_koutsu == 2 && hand.pair().is_dragon() {
        yaku.push(YakuResult::new(YakuName::Shousangen, 2));
    }

    if all_tiles.iter().all(|&t| all_terminal_or_honor(t)) {
        yaku.push(YakuResult::new(YakuName::Honroutou, 2));
    }

    if matches!(hand, WinningHand::Standard { .. }) {
        let has_honor = all_tiles.iter().any(|t| t.is_honor());
        let has_number = all_tiles.iter().any(|t| t.is_number());
        if has_honor && has_number {
            let has_sequence = num_shuntsu > 0 || has_open_sequence(ctx.melds);
            let all_groups_have_yaochuuhai = hand.groups().iter().all(|m| match m.kind {
                MentsuKind::Shuntsu => {
                    m.tile_type.is_yaochuuhai() || TileType(m.tile_type.0 + 2).is_yaochuuhai()
                }
                MentsuKind::Koutsu => m.tile_type.is_yaochuuhai(),
            }) && hand.pair().is_yaochuuhai()
                && ctx.melds.iter().all(meld_has_yaochuuhai);
            if all_groups_have_yaochuuhai && has_sequence {
                yaku.push(YakuResult::new(
                    YakuName::Honchantai,
                    if is_menzen { 2 } else { 1 },
                ));
            }
        }
    }

    if matches!(hand, WinningHand::Standard { .. }) {
        let mut found = false;
        for i in 0..hand.groups().len() {
            if hand.groups()[i].kind != MentsuKind::Shuntsu {
                continue;
            }
            let tt = hand.groups()[i].tile_type;
            if !tt.is_number() {
                continue;
            }
            let rank = tt.rank().0;
            let suit = tt.suit();
            for other_suit in [Suit::Man, Suit::Pin, Suit::Sou] {
                if other_suit == suit {
                    continue;
                }
                let other_tt = TileType(suit_base(other_suit) + rank - 1);
                let has_in_hand = hand
                    .groups()
                    .iter()
                    .any(|m| m.kind == MentsuKind::Shuntsu && m.tile_type == other_tt);
                let has_in_meld = ctx.melds.iter().any(|m| {
                    if matches!(m.kind, MeldKind::Chi) {
                        let mut ts: Vec<TileType> = m.tiles.iter().map(|t| t.tile_type()).collect();
                        ts.sort_by_key(|t| t.0);
                        ts[0] == other_tt
                    } else {
                        false
                    }
                });
                if !has_in_hand && !has_in_meld {
                    continue;
                }
                let third_suit = [Suit::Man, Suit::Pin, Suit::Sou]
                    .iter()
                    .find(|&&s| s != suit && s != other_suit)
                    .unwrap();
                let third_tt = TileType(suit_base(*third_suit) + rank - 1);
                let has_third = hand
                    .groups()
                    .iter()
                    .any(|m| m.kind == MentsuKind::Shuntsu && m.tile_type == third_tt)
                    || ctx.melds.iter().any(|m| {
                        if matches!(m.kind, MeldKind::Chi) {
                            let mut ts: Vec<TileType> =
                                m.tiles.iter().map(|t| t.tile_type()).collect();
                            ts.sort_by_key(|t| t.0);
                            ts[0] == third_tt
                        } else {
                            false
                        }
                    });
                if has_third {
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
        if !found {
            for start in ctx.melds.iter().filter_map(meld_sequence_start) {
                let rank = start.rank().0;
                let suit = start.suit();
                for other_suit in [Suit::Man, Suit::Pin, Suit::Sou] {
                    if other_suit == suit {
                        continue;
                    }
                    let other = TileType(suit_base(other_suit) + rank - 1);
                    let third_suit = [Suit::Man, Suit::Pin, Suit::Sou]
                        .iter()
                        .find(|&&candidate| candidate != suit && candidate != other_suit)
                        .unwrap();
                    let third = TileType(suit_base(*third_suit) + rank - 1);
                    if has_sequence_start(hand, ctx.melds, other)
                        && has_sequence_start(hand, ctx.melds, third)
                    {
                        found = true;
                        break;
                    }
                }
                if found {
                    break;
                }
            }
        }
        if found {
            yaku.push(YakuResult::new(
                YakuName::SanshokuDoujun,
                if is_menzen { 2 } else { 1 },
            ));
        }
    }

    if matches!(hand, WinningHand::Standard { .. }) {
        let mut found = false;
        for i in 0..hand.groups().len() {
            if hand.groups()[i].kind != MentsuKind::Shuntsu {
                continue;
            }
            let tt = hand.groups()[i].tile_type;
            if !tt.is_number() {
                continue;
            }
            let rank = tt.rank().0;
            if rank != 1 {
                continue;
            }
            let suit = tt.suit();
            let mid = TileType(suit_base(suit) + 3);
            let high = TileType(suit_base(suit) + 6);
            let has_mid = hand
                .groups()
                .iter()
                .any(|m| m.kind == MentsuKind::Shuntsu && m.tile_type == mid)
                || ctx.melds.iter().any(|m| {
                    if matches!(m.kind, MeldKind::Chi) {
                        let mut ts: Vec<TileType> = m.tiles.iter().map(|t| t.tile_type()).collect();
                        ts.sort_by_key(|t| t.0);
                        ts[0] == mid
                    } else {
                        false
                    }
                });
            let has_high = hand
                .groups()
                .iter()
                .any(|m| m.kind == MentsuKind::Shuntsu && m.tile_type == high)
                || ctx.melds.iter().any(|m| {
                    if matches!(m.kind, MeldKind::Chi) {
                        let mut ts: Vec<TileType> = m.tiles.iter().map(|t| t.tile_type()).collect();
                        ts.sort_by_key(|t| t.0);
                        ts[0] == high
                    } else {
                        false
                    }
                });
            if has_mid && has_high {
                found = true;
                break;
            }
        }
        if !found {
            for meld in ctx.melds {
                let Some(low) = meld_sequence_start(meld) else {
                    continue;
                };
                if low.rank().0 != 1 {
                    continue;
                }
                let suit = low.suit();
                let mid = TileType(suit_base(suit) + 3);
                let high = TileType(suit_base(suit) + 6);
                let has_start = |start: TileType| {
                    hand.groups()
                        .iter()
                        .any(|m| m.kind == MentsuKind::Shuntsu && m.tile_type == start)
                        || ctx
                            .melds
                            .iter()
                            .filter_map(meld_sequence_start)
                            .any(|start_type| start_type == start)
                };
                if has_start(mid) && has_start(high) {
                    found = true;
                    break;
                }
            }
        }
        if found {
            yaku.push(YakuResult::new(
                YakuName::Ittsu,
                if is_menzen { 2 } else { 1 },
            ));
        }
    }

    {
        let has_honor = all_tiles.iter().any(|t| t.is_honor());
        let mut single_suit: Option<Suit> = None;
        let mut single = true;
        for &t in &all_tiles {
            if t.is_honor() {
                continue;
            }
            let s = t.suit();
            match single_suit {
                None => single_suit = Some(s),
                Some(prev) if prev != s => {
                    single = false;
                    break;
                }
                _ => {}
            }
        }
        if single && single_suit.is_some() {
            if !has_honor {
                yaku.push(YakuResult::new(
                    YakuName::Chinitsu,
                    if is_menzen { 6 } else { 5 },
                ));
            } else {
                yaku.push(YakuResult::new(
                    YakuName::Honitsu,
                    if is_menzen { 3 } else { 2 },
                ));
            }
        }
    }

    if matches!(hand, WinningHand::Standard { .. }) {
        let has_honor = all_tiles.iter().any(|t| t.is_honor());
        let has_number = all_tiles.iter().any(|t| t.is_number());
        if !has_honor && has_number {
            let has_sequence = num_shuntsu > 0 || has_open_sequence(ctx.melds);
            let all_groups_have_terminal = hand.groups().iter().all(|m| match m.kind {
                MentsuKind::Shuntsu => {
                    m.tile_type.is_terminal() || TileType(m.tile_type.0 + 2).is_terminal()
                }
                MentsuKind::Koutsu => m.tile_type.is_terminal(),
            }) && hand.pair().is_terminal()
                && ctx.melds.iter().all(meld_has_terminal);
            if all_groups_have_terminal && has_sequence {
                yaku.push(YakuResult::new(
                    YakuName::Junchan,
                    if is_menzen { 3 } else { 2 },
                ));
            }
        }
    }

    if is_menzen && matches!(hand, WinningHand::Standard { .. }) {
        let mut pairs = 0usize;
        for i in 0..hand.groups().len() {
            if hand.groups()[i].kind != MentsuKind::Shuntsu {
                continue;
            }
            for j in (i + 1)..hand.groups().len() {
                if hand.groups()[j].kind == MentsuKind::Shuntsu
                    && hand.groups()[i].tile_type == hand.groups()[j].tile_type
                {
                    pairs += 1;
                }
            }
        }
        if pairs >= 2 {
            yaku.push(YakuResult::new(YakuName::Ryanpeiko, 3));
        }
    }

    yaku
}
