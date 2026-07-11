use riichi_core::meld::{Meld, MeldKind};
use riichi_core::tile::{Suit, Tile, TileType};

use crate::analysis::{
    decompose_all_standard_with_mentsu, decompose_kokushi, decompose_seven_pairs,
    is_standard_win_with_mentsu, is_winning,
};
use crate::dora::calculate_dora;
use crate::fu::calculate_fu_with_winning_tile;
use crate::scoring::calculate_points_with_loser;
use crate::types::{
    HandType, MentsuKind, TileCounts, WinContext, WinResult, WinningHand, YakuName, YakuResult,
};

/// 判和：判形 + 判振 + (判役 → 算翻 → 算符 → 算点)
///
/// # 参数
/// - `all_tiles`: 手牌 + 副露 + 和了牌的实体牌（用于宝牌/赤宝牌计算）
/// - `hand_tiles`: 仅门清部分的 TileType（用于判形和拆解）
/// - `ctx`: 判和上下文
/// - `is_furiten`: 是否振听
pub fn check_win(
    all_tiles: &[Tile],
    hand_tiles: &[TileType],
    ctx: &WinContext,
    is_furiten: bool,
    winning_tile: Tile,
) -> Option<WinResult> {
    // ── Step 1: 判振 ──
    if is_furiten && !ctx.is_tsumo {
        return None;
    }

    // ── Step 2: 判形 ──
    if !is_win_shape_with_open_melds(hand_tiles, ctx.melds.len()) {
        return None;
    }

    // ── Step 3: 判役 ──
    let decompositions = decompose_hand_with_open_melds(hand_tiles, ctx.melds.len());
    if decompositions.is_empty() {
        return None;
    }

    // ── Step 4: 对每个完整分解独立计算役、宝牌、符和点数 ──
    let dora_result = calculate_dora(
        all_tiles,
        &ctx.dora_indicators,
        &ctx.ura_dora_indicators,
        ctx.is_riichi,
        ctx.red_fives,
    );
    let mut best_result: Option<WinResult> = None;
    for hand in &decompositions {
        // detect_yaku 对单个分解调用，避免把一个分解的役和另一个分解的
        // 符组合在一起。
        let yaku_results = detect_yaku(std::slice::from_ref(hand), ctx, winning_tile);
        if yaku_results.is_empty() {
            continue;
        }
        if !ctx.atozuke
            && yaku_results.iter().all(|result| {
                matches!(
                    result.yaku,
                    YakuName::YakuhaiJikaze | YakuName::YakuhaiBakaze | YakuName::YakuhaiSangen
                )
            })
            && winning_tile.tile_type().is_honor()
            && all_tiles
                .iter()
                .filter(|tile| tile.tile_type() == winning_tile.tile_type())
                .count()
                .saturating_sub(1)
                < 3
        {
            // 后付禁止仅靠和了牌补出役牌刻子；若役牌刻子在和牌前已存在，
            // 则 pre-win 计数已经达到 3，不会被这里拦截。
            continue;
        }
        let mut all_yaku = yaku_results;
        if dora_result.dora > 0 {
            all_yaku.push(YakuResult::new(
                crate::types::YakuName::Dora,
                dora_result.dora,
            ));
        }
        if dora_result.aka_dora > 0 {
            all_yaku.push(YakuResult::new(
                crate::types::YakuName::AkaDora,
                dora_result.aka_dora,
            ));
        }
        if dora_result.ura_dora > 0 {
            all_yaku.push(YakuResult::new(
                crate::types::YakuName::UraDora,
                dora_result.ura_dora,
            ));
        }

        let fu = calculate_fu_with_winning_tile(
            hand,
            &ctx.melds,
            &all_yaku,
            ctx.is_tsumo,
            ctx.seat_wind,
            ctx.field_wind,
            Some(winning_tile.tile_type()),
        );
        let total_han: u8 = all_yaku.iter().map(|y| y.han).sum();
        // 役满项目可能是双倍役满（26 翻标记），不能只按项目数量计数。
        let yakuman_count: u8 = all_yaku
            .iter()
            .filter(|y| y.han >= 13)
            .map(|y| (y.han / 13).max(1))
            .sum();
        let points = calculate_points_with_loser(
            total_han,
            fu,
            yakuman_count,
            ctx.winner,
            ctx.loser,
            ctx.dealer,
            ctx.riichi_sticks,
            ctx.honba,
            ctx.is_tsumo,
        );
        let candidate = WinResult {
            yaku_results: all_yaku,
            total_han,
            fu,
            points,
        };
        let is_better = match best_result.as_ref() {
            None => true,
            Some(best) => candidate.points[ctx.winner] > best.points[ctx.winner],
        };
        if is_better {
            best_result = Some(candidate);
        }
    }

    best_result
}

// ═══════════════════════════════════════════════════════════════
//  判形
// ═══════════════════════════════════════════════════════════════

/// 判形：检查 tiles（手牌+副露+和了牌）是否组成和牌形
pub fn is_win_shape(tiles: &[TileType]) -> bool {
    let mut counts = TileCounts::new();
    for &tt in tiles {
        counts.inc(tt);
    }
    is_winning(&mut counts)
}

/// 判定包含固定副露的和牌形。
pub fn is_win_shape_with_open_melds(tiles: &[TileType], open_meld_count: usize) -> bool {
    if open_meld_count == 0 {
        return is_win_shape(tiles);
    }
    if open_meld_count > 4 {
        return false;
    }
    let mut counts = TileCounts::new();
    for &tt in tiles {
        counts.inc(tt);
    }
    is_standard_win_with_mentsu(&mut counts, 4 - open_meld_count)
}

// ═══════════════════════════════════════════════════════════════
//  拆解
// ═══════════════════════════════════════════════════════════════

/// 拆解手牌（门清部分），返回所有可能的分解方式
///
/// 接收门清部分的牌（不含副露），返回所有标准形+七对子+国士无双的分解
pub fn decompose_hand(hand_tiles: &[TileType]) -> Vec<WinningHand> {
    decompose_hand_with_open_melds(hand_tiles, 0)
}

/// 分解包含固定副露的和牌。
///
/// `hand_tiles` 只包含门清部分和和了牌；`open_meld_count` 个副露作为
/// 已完成面子，不参与门清牌的分解。
pub fn decompose_hand_with_open_melds(
    hand_tiles: &[TileType],
    open_meld_count: usize,
) -> Vec<WinningHand> {
    let mut counts = TileCounts::new();
    for &tt in hand_tiles {
        counts.inc(tt);
    }

    let required_mentsu = 4usize.saturating_sub(open_meld_count);
    let mut results = decompose_all_standard_with_mentsu(&mut counts, required_mentsu);
    if open_meld_count == 0 {
        if let Some(sp) = decompose_seven_pairs(&counts) {
            results.push(sp);
        }
        if let Some(k) = decompose_kokushi(&counts) {
            results.push(k);
        }
    }
    results
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

fn is_yakuhai(t: TileType, ctx: &WinContext) -> bool {
    t.is_dragon() || t == ctx.seat_wind || t == ctx.field_wind
}

fn has_open_sequence(melds: &[Meld]) -> bool {
    melds.iter().any(|m| m.kind == MeldKind::Chi)
}

fn is_pinfu_wait(hand: &WinningHand, winning_tile: Tile) -> bool {
    let winning_type = winning_tile.tile_type();
    hand.mentsu.iter().any(|m| {
        if m.kind != MentsuKind::Shuntsu || m.tile_type.suit() != winning_type.suit() {
            return false;
        }
        let start = m.tile_type.rank().0;
        let win_rank = winning_type.rank().0;
        // 23 听 1/4、34 听 2/5……；12 听 3 和 78 听 7 属边张，不能算平和。
        (win_rank == start && (2..=6).contains(&start))
            || (win_rank == start + 2 && (1..=5).contains(&start))
    })
}

fn collect_hand_tiles(hand: &WinningHand) -> Vec<TileType> {
    let mut tiles = vec![hand.jantai; 2];
    for m in &hand.mentsu {
        match m.kind {
            MentsuKind::Shuntsu => {
                tiles.push(m.tile_type);
                tiles.push(TileType(m.tile_type.0 + 1));
                tiles.push(TileType(m.tile_type.0 + 2));
            }
            MentsuKind::Koutsu => {
                let count = if hand.hand_type == HandType::SevenPairs {
                    2
                } else {
                    3
                };
                for _ in 0..count {
                    tiles.push(m.tile_type);
                }
            }
        }
    }
    tiles
}

fn collect_all_tiles(hand: &WinningHand, melds: &[Meld]) -> Vec<TileType> {
    let mut tiles = collect_hand_tiles(hand);
    for m in melds {
        tiles.extend(m.tiles.iter().map(|t| t.tile_type()));
    }
    tiles
}

fn detect_yaku(
    decompositions: &[WinningHand],
    ctx: &WinContext,
    winning_tile: Tile,
) -> Vec<YakuResult> {
    let open_triplets = count_open_triplets(&ctx.melds);
    let concealed_triplets_melds = count_concealed_triplets(&ctx.melds);

    let mut best: Option<Vec<YakuResult>> = None;
    let mut best_han = 0u8;

    for hand in decompositions {
        let all_tiles = collect_all_tiles(hand, &ctx.melds);
        let mut yaku = Vec::new();

        let num_koutsu = hand
            .mentsu
            .iter()
            .filter(|m| m.kind == MentsuKind::Koutsu)
            .count();
        let num_shuntsu = hand
            .mentsu
            .iter()
            .filter(|m| m.kind == MentsuKind::Shuntsu)
            .count();
        let is_menzen = ctx.melds.iter().all(|m| m.is_concealed());

        let koutsu_in_hand = num_koutsu + concealed_triplets_melds;
        let total_koutsu = koutsu_in_hand + open_triplets;
        let total_kans = count_kans(&ctx.melds);

        let mut concealed_triplet_count = koutsu_in_hand;
        if !ctx.is_tsumo
            && hand
                .mentsu
                .iter()
                .any(|m| m.kind == MentsuKind::Koutsu && m.tile_type == winning_tile.tile_type())
        {
            concealed_triplet_count = concealed_triplet_count.saturating_sub(1);
        }

        if hand.hand_type == HandType::Kokushi {
            let mut thirteen_wait = false;
            for &tt in &TileType::YAOCHUUHAI {
                if ctx
                    .melds
                    .iter()
                    .all(|m| !m.tiles.iter().any(|t| t.tile_type() == tt))
                    && decompositions
                        .iter()
                        .any(|d| d.all_tiles().iter().filter(|&&t| t == tt).count() == 1)
                {
                    thirteen_wait = true;
                    break;
                }
            }
            if thirteen_wait {
                yaku.push(YakuResult::new(YakuName::Kokushi13, 26));
            } else {
                yaku.push(YakuResult::new(YakuName::Kokushi, 13));
            }
            let total_han: u8 = yaku.iter().map(|y| y.han).sum();
            if total_han > best_han {
                best_han = total_han;
                best = Some(yaku.clone());
            }
            continue;
        }

        if is_menzen && hand.hand_type != HandType::SevenPairs {
            let suit = hand.all_tiles().first().map(|t| t.suit());
            if let Some(s) = suit {
                let required = [3, 1, 1, 1, 1, 1, 1, 1, 3];
                let mut ok = true;
                for (i, &need) in required.iter().enumerate() {
                    let tt = TileType(suit_base(s) + i as u8);
                    if all_tiles.iter().filter(|&&t| t == tt).count() != need {
                        ok = false;
                        break;
                    }
                }
                if ok {
                    let mut nine_wait = false;
                    for i in 0..9u8 {
                        let tt = TileType(suit_base(s) + i);
                        if winning_tile.tile_type() == tt
                            && all_tiles.iter().filter(|&&t| t == tt).count() == 1
                        {
                            nine_wait = true;
                            break;
                        }
                    }
                    if nine_wait {
                        yaku.push(YakuResult::new(YakuName::ChuurenPoutou9, 26));
                    } else {
                        yaku.push(YakuResult::new(YakuName::ChuurenPoutou, 13));
                    }
                    let total_han: u8 = yaku.iter().map(|y| y.han).sum();
                    if total_han > best_han {
                        best_han = total_han;
                        best = Some(yaku.clone());
                    }
                    continue;
                }
            }
        }

        if ctx.is_tsumo && concealed_triplet_count >= 4 {
            yaku.push(YakuResult::new(YakuName::Suuankou, 13));
        } else if !ctx.is_tsumo && concealed_triplet_count >= 4 {
            if hand.jantai == winning_tile.tile_type() {
                yaku.push(YakuResult::new(YakuName::SuuankouTanki, 26));
            } else {
                yaku.push(YakuResult::new(YakuName::Suuankou, 13));
            }
        }

        let dragon_koutsu = (31..=33u8)
            .filter(|&i| {
                let tt = TileType(i);
                hand.mentsu
                    .iter()
                    .any(|m| m.tile_type == tt && m.kind == MentsuKind::Koutsu)
                    || ctx.melds.iter().any(|m| {
                        m.tiles[0].tile_type() == tt
                            && matches!(
                                m.kind,
                                MeldKind::Pon
                                    | MeldKind::Minkan
                                    | MeldKind::Kakan
                                    | MeldKind::Ankan
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
                hand.mentsu
                    .iter()
                    .any(|m| m.tile_type == tt && m.kind == MentsuKind::Koutsu)
                    || ctx.melds.iter().any(|m| {
                        m.tiles[0].tile_type() == tt
                            && matches!(
                                m.kind,
                                MeldKind::Pon
                                    | MeldKind::Minkan
                                    | MeldKind::Kakan
                                    | MeldKind::Ankan
                            )
                    })
            })
            .count();
        if wind_koutsu == 4 {
            yaku.push(YakuResult::new(YakuName::Daisuushii, 26));
        } else if wind_koutsu == 3 && hand.jantai.is_wind() {
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

        if ctx.is_riichi && is_menzen {
            yaku.push(YakuResult::new(YakuName::Riichi, 1));
        }

        if ctx.is_ippatsu && is_menzen {
            yaku.push(YakuResult::new(YakuName::Ippatsu, 1));
        }

        if all_tiles.iter().all(|&t| all_simple(t)) && (is_menzen || ctx.kuitan) {
            yaku.push(YakuResult::new(YakuName::Tanyao, 1));
        }

        if is_menzen
            && hand.hand_type != HandType::SevenPairs
            && num_shuntsu == 4
            && !is_yakuhai(hand.jantai, ctx)
            && is_pinfu_wait(hand, winning_tile)
        {
            yaku.push(YakuResult::new(YakuName::Pinfu, 1));
        }

        if is_menzen && hand.hand_type != HandType::SevenPairs {
            let mut found = false;
            for i in 0..hand.mentsu.len() {
                if hand.mentsu[i].kind != MentsuKind::Shuntsu {
                    continue;
                }
                for j in (i + 1)..hand.mentsu.len() {
                    if hand.mentsu[j].kind == MentsuKind::Shuntsu
                        && hand.mentsu[i].tile_type == hand.mentsu[j].tile_type
                    {
                        found = true;
                        break;
                    }
                }
                if found {
                    break;
                }
            }
            if found {
                yaku.push(YakuResult::new(YakuName::Iipeiko, 1));
            }
        }

        // 役牌必须来自刻子/杠子，雀头不能单独构成役牌。
        let mut yakuhai_triplets = Vec::new();
        for mentsu in &hand.mentsu {
            if mentsu.kind == MentsuKind::Koutsu && !yakuhai_triplets.contains(&mentsu.tile_type) {
                yakuhai_triplets.push(mentsu.tile_type);
            }
        }
        for meld in &ctx.melds {
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

        if ctx.is_double_riichi {
            yaku.push(YakuResult::new(YakuName::DoubleRiichi, 2));
        }

        if hand.hand_type == HandType::SevenPairs {
            yaku.push(YakuResult::new(YakuName::Chiitoitsu, 2));
        }

        if total_koutsu == 4 && hand.hand_type != HandType::SevenPairs {
            yaku.push(YakuResult::new(YakuName::Toitoi, 2));
        }

        if concealed_triplet_count >= 3 {
            yaku.push(YakuResult::new(YakuName::Sananko, 2));
        }

        if total_kans == 3 {
            yaku.push(YakuResult::new(YakuName::Sankantsu, 2));
        }

        if hand.hand_type != HandType::SevenPairs {
            let mut found = false;
            for i in 0..hand.mentsu.len() {
                if hand.mentsu[i].kind != MentsuKind::Koutsu {
                    continue;
                }
                let tt = hand.mentsu[i].tile_type;
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
                        .mentsu
                        .iter()
                        .any(|m| m.kind == MentsuKind::Koutsu && m.tile_type == other_tt);
                    let has_in_meld = ctx.melds.iter().any(|m| {
                        m.tiles[0].tile_type() == other_tt
                            && matches!(
                                m.kind,
                                MeldKind::Pon
                                    | MeldKind::Minkan
                                    | MeldKind::Kakan
                                    | MeldKind::Ankan
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
                        .mentsu
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
            if found {
                yaku.push(YakuResult::new(YakuName::SanshokuDoukou, 2));
            }
        }

        if dragon_koutsu == 2 && hand.jantai.is_dragon() {
            yaku.push(YakuResult::new(YakuName::Shousangen, 2));
        }

        if all_tiles.iter().all(|&t| all_terminal_or_honor(t)) {
            yaku.push(YakuResult::new(YakuName::Honroutou, 2));
        }

        if hand.hand_type != HandType::SevenPairs {
            let has_honor = all_tiles.iter().any(|t| t.is_honor());
            let has_number = all_tiles.iter().any(|t| t.is_number());
            if has_honor && has_number {
                let has_sequence = num_shuntsu > 0 || has_open_sequence(&ctx.melds);
                let all_groups_have_yaochuuhai = hand.mentsu.iter().all(|m| match m.kind {
                    MentsuKind::Shuntsu => {
                        m.tile_type.is_yaochuuhai() || TileType(m.tile_type.0 + 2).is_yaochuuhai()
                    }
                    MentsuKind::Koutsu => m.tile_type.is_yaochuuhai(),
                }) && hand.jantai.is_yaochuuhai()
                    && ctx.melds.iter().all(|m| {
                        let first = m.tiles[0].tile_type();
                        let last = m.tiles.last().unwrap().tile_type();
                        first.is_yaochuuhai() || last.is_yaochuuhai()
                    });
                if all_groups_have_yaochuuhai && has_sequence {
                    yaku.push(YakuResult::new(
                        YakuName::Honchantai,
                        if is_menzen { 2 } else { 1 },
                    ));
                }
            }
        }

        if hand.hand_type != HandType::SevenPairs {
            let mut found = false;
            for i in 0..hand.mentsu.len() {
                if hand.mentsu[i].kind != MentsuKind::Shuntsu {
                    continue;
                }
                let tt = hand.mentsu[i].tile_type;
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
                        .mentsu
                        .iter()
                        .any(|m| m.kind == MentsuKind::Shuntsu && m.tile_type == other_tt);
                    let has_in_meld = ctx.melds.iter().any(|m| {
                        if matches!(m.kind, MeldKind::Chi) {
                            let mut ts: Vec<TileType> =
                                m.tiles.iter().map(|t| t.tile_type()).collect();
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
                        .mentsu
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
            if found {
                yaku.push(YakuResult::new(
                    YakuName::SanshokuDoujun,
                    if is_menzen { 2 } else { 1 },
                ));
            }
        }

        if hand.hand_type != HandType::SevenPairs {
            let mut found = false;
            for i in 0..hand.mentsu.len() {
                if hand.mentsu[i].kind != MentsuKind::Shuntsu {
                    continue;
                }
                let tt = hand.mentsu[i].tile_type;
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
                    .mentsu
                    .iter()
                    .any(|m| m.kind == MentsuKind::Shuntsu && m.tile_type == mid)
                    || ctx.melds.iter().any(|m| {
                        if matches!(m.kind, MeldKind::Chi) {
                            let mut ts: Vec<TileType> =
                                m.tiles.iter().map(|t| t.tile_type()).collect();
                            ts.sort_by_key(|t| t.0);
                            ts[0] == mid
                        } else {
                            false
                        }
                    });
                let has_high = hand
                    .mentsu
                    .iter()
                    .any(|m| m.kind == MentsuKind::Shuntsu && m.tile_type == high)
                    || ctx.melds.iter().any(|m| {
                        if matches!(m.kind, MeldKind::Chi) {
                            let mut ts: Vec<TileType> =
                                m.tiles.iter().map(|t| t.tile_type()).collect();
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

        if hand.hand_type != HandType::SevenPairs {
            let has_honor = all_tiles.iter().any(|t| t.is_honor());
            let has_number = all_tiles.iter().any(|t| t.is_number());
            if !has_honor && has_number {
                let has_sequence = num_shuntsu > 0 || has_open_sequence(&ctx.melds);
                let all_groups_have_terminal = hand.mentsu.iter().all(|m| match m.kind {
                    MentsuKind::Shuntsu => {
                        m.tile_type.is_terminal() || TileType(m.tile_type.0 + 2).is_terminal()
                    }
                    MentsuKind::Koutsu => m.tile_type.is_terminal(),
                }) && hand.jantai.is_terminal()
                    && ctx.melds.iter().all(|m| {
                        let first = m.tiles[0].tile_type();
                        let last = m.tiles.last().unwrap().tile_type();
                        first.is_terminal() || last.is_terminal()
                    });
                if all_groups_have_terminal && has_sequence {
                    yaku.push(YakuResult::new(
                        YakuName::Junchan,
                        if is_menzen { 3 } else { 2 },
                    ));
                }
            }
        }

        if is_menzen && hand.hand_type != HandType::SevenPairs {
            let mut pairs = 0usize;
            for i in 0..hand.mentsu.len() {
                if hand.mentsu[i].kind != MentsuKind::Shuntsu {
                    continue;
                }
                for j in (i + 1)..hand.mentsu.len() {
                    if hand.mentsu[j].kind == MentsuKind::Shuntsu
                        && hand.mentsu[i].tile_type == hand.mentsu[j].tile_type
                    {
                        pairs += 1;
                    }
                }
            }
            if pairs >= 2 {
                yaku.push(YakuResult::new(YakuName::Ryanpeiko, 3));
            }
        }

        let total_han: u8 = yaku.iter().map(|y| y.han).sum();
        if total_han > best_han {
            best_han = total_han;
            best = Some(yaku.clone());
        }
    }

    best.unwrap_or_default()
}
