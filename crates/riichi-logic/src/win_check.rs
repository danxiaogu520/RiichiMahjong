use riichi_core::meld::{Meld, MeldKind};
use riichi_core::tile::{Suit, Tile, TileType};

use crate::analysis::{
    decompose_all_standard_with_mentsu, decompose_kokushi, decompose_seven_pairs,
    is_standard_win_with_mentsu, is_winning,
};
use crate::dora::calculate_dora;
use crate::fu::calculate_fu_with_winning_tile;
use crate::scoring::calculate_points_with_loser_and_pao;
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
        // 后付/片和了按“具体和了牌”判定：当前分解只要已经产生至少一项
        // 役，就允许这张牌和；无役的另一张听牌不会因为同一牌型可和而放行。
        // 不再根据役牌是否在和了前已经存在进行启发式拦截。
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

        // 役满结算不再叠加普通役和宝牌；多个役满之间仍然保留并累计。
        if all_yaku.iter().any(|result| result.han >= 13) {
            all_yaku.retain(|result| result.han >= 13);
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
        let points = calculate_points_with_loser_and_pao(
            total_han,
            fu,
            yakuman_count,
            ctx.winner,
            ctx.loser,
            ctx.dealer,
            ctx.riichi_sticks,
            ctx.honba,
            ctx.is_tsumo,
            ctx.pao_target,
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
    hand.mentsu
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

fn is_pinfu_wait(hand: &WinningHand, winning_tile: Tile) -> bool {
    let winning_type = winning_tile.tile_type();
    hand.mentsu.iter().any(|m| {
        if m.kind != MentsuKind::Shuntsu || m.tile_type.suit() != winning_type.suit() {
            return false;
        }
        let start = m.tile_type.rank().0;
        let win_rank = winning_type.rank().0;
        // 23 听 1/4、34 听 2/5……；12 听 3 和 78 听 7 属边张，不能算平和。
        (win_rank == start && (1..=6).contains(&start))
            || (win_rank == start + 2 && (2..=7).contains(&start))
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
        let all_tiles = if hand.hand_type == HandType::Kokushi {
            // 国士分解不使用普通面子列表，必须显式还原 13 种幺九字牌和雀头。
            let mut tiles = TileType::YAOCHUUHAI.to_vec();
            tiles.push(hand.jantai);
            tiles
        } else {
            collect_all_tiles(hand, &ctx.melds)
        };
        let mut yaku = Vec::new();

        let num_koutsu = if hand.hand_type == HandType::SevenPairs {
            0
        } else {
            hand.mentsu
                .iter()
                .filter(|m| m.kind == MentsuKind::Koutsu)
                .count()
        };
        let num_shuntsu = hand
            .mentsu
            .iter()
            .filter(|m| m.kind == MentsuKind::Shuntsu)
            .count();
        let is_menzen = ctx.melds.iter().all(|m| m.is_concealed());

        let koutsu_in_hand = if hand.hand_type == HandType::SevenPairs {
            0
        } else {
            num_koutsu + concealed_triplets_melds
        };
        let total_koutsu = koutsu_in_hand + open_triplets;
        let total_kans = count_kans(&ctx.melds);

        let mut concealed_triplet_count = koutsu_in_hand;
        if hand.hand_type != HandType::SevenPairs
            && !ctx.is_tsumo
            && hand
                .mentsu
                .iter()
                .any(|m| m.kind == MentsuKind::Koutsu && m.tile_type == winning_tile.tile_type())
        {
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
            let total_han: u8 = yaku.iter().map(|y| y.han).sum();
            if total_han > best_han {
                best_han = total_han;
                best = Some(yaku.clone());
            }
            continue;
        }

        if hand.hand_type == HandType::Kokushi {
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
            if thirteen_wait && ctx.allow_double_yakuman {
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

        if is_menzen && ctx.melds.is_empty() && hand.hand_type != HandType::SevenPairs {
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
                    if nine_wait && ctx.allow_double_yakuman {
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

        if hand.hand_type != HandType::SevenPairs && ctx.is_tsumo && concealed_triplet_count >= 4 {
            yaku.push(YakuResult::new(YakuName::Suuankou, 13));
        } else if hand.hand_type != HandType::SevenPairs
            && !ctx.is_tsumo
            && concealed_triplet_count >= 4
        {
            if hand.jantai == winning_tile.tile_type() && ctx.allow_double_yakuman {
                yaku.push(YakuResult::new(YakuName::SuuankouTanki, 26));
            } else {
                yaku.push(YakuResult::new(YakuName::Suuankou, 13));
            }
        }

        let dragon_koutsu = (31..=33u8)
            .filter(|&i| {
                let tt = TileType(i);
                (hand.hand_type != HandType::SevenPairs
                    && hand
                        .mentsu
                        .iter()
                        .any(|m| m.tile_type == tt && m.kind == MentsuKind::Koutsu))
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
                (hand.hand_type != HandType::SevenPairs
                    && hand
                        .mentsu
                        .iter()
                        .any(|m| m.tile_type == tt && m.kind == MentsuKind::Koutsu))
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
            yaku.push(YakuResult::new(
                YakuName::Daisuushii,
                if ctx.allow_double_yakuman { 26 } else { 13 },
            ));
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

        if ctx.is_double_riichi && is_menzen {
            yaku.push(YakuResult::new(YakuName::DoubleRiichi, 2));
        } else if ctx.is_riichi && is_menzen {
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
            let mut duplicate_sequence_pairs = 0usize;
            for i in 0..hand.mentsu.len() {
                if hand.mentsu[i].kind != MentsuKind::Shuntsu {
                    continue;
                }
                for j in (i + 1)..hand.mentsu.len() {
                    if hand.mentsu[j].kind == MentsuKind::Shuntsu
                        && hand.mentsu[i].tile_type == hand.mentsu[j].tile_type
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
        for mentsu in &hand.mentsu {
            if hand.hand_type != HandType::SevenPairs
                && mentsu.kind == MentsuKind::Koutsu
                && !yakuhai_triplets.contains(&mentsu.tile_type)
            {
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

        if hand.hand_type == HandType::SevenPairs {
            yaku.push(YakuResult::new(YakuName::Chiitoitsu, 2));
        }

        if total_koutsu == 4 && hand.hand_type != HandType::SevenPairs {
            yaku.push(YakuResult::new(YakuName::Toitoi, 2));
        }

        if hand.hand_type != HandType::SevenPairs && concealed_triplet_count >= 3 {
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
                            hand.mentsu
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
                    && ctx.melds.iter().all(meld_has_yaochuuhai);
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
                        if has_sequence_start(hand, &ctx.melds, other)
                            && has_sequence_start(hand, &ctx.melds, third)
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
            if !found {
                for meld in &ctx.melds {
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
                        hand.mentsu
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
                    && ctx.melds.iter().all(meld_has_terminal);
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

#[cfg(test)]
mod tests {
    use super::{check_win, detect_yaku};
    use crate::types::{HandType, Mentsu, MentsuKind, WinContext, WinningHand, YakuName};
    use riichi_core::meld::{Meld, MeldKind};
    use riichi_core::player::PlayerId;
    use riichi_core::tile::TileType;

    fn context(atozuke: bool, melds: Vec<Meld>) -> WinContext {
        WinContext {
            is_tsumo: false,
            is_riichi: false,
            is_double_riichi: false,
            is_ippatsu: false,
            is_rinshan: false,
            is_chankan: false,
            is_haitei: false,
            is_houtei: false,
            is_tenhou: false,
            is_chiihou: false,
            red_fives: [0; 3],
            kuitan: true,
            atozuke,
            allow_double_yakuman: true,
            seat_wind: TileType::EAST,
            field_wind: TileType::EAST,
            dora_indicators: Vec::new(),
            ura_dora_indicators: Vec::new(),
            melds,
            dealer: 0,
            winner: 1,
            loser: Some(0),
            pao_target: None,
            honba: 0,
            riichi_sticks: 0,
        }
    }

    #[test]
    fn no_atozuke_allows_preexisting_open_yakuhai() {
        let haku = TileType::HAKU.with_copy(0);
        let meld = Meld::pon(
            vec![
                haku,
                TileType::HAKU.with_copy(1),
                TileType::HAKU.with_copy(2),
            ],
            haku,
            PlayerId(0),
        );
        let hand_types = vec![
            TileType(0),
            TileType(1),
            TileType(2),
            TileType(3),
            TileType(4),
            TileType(5),
            TileType(6),
            TileType(7),
            TileType(8),
            TileType(9),
            TileType(9),
        ];
        let all_tiles = hand_types
            .iter()
            .enumerate()
            .map(|(i, &tile_type)| tile_type.with_copy((i % 4) as u8))
            .chain(meld.tiles.iter().copied())
            .collect::<Vec<_>>();

        let result = check_win(
            &all_tiles,
            &hand_types,
            &context(false, vec![meld]),
            false,
            TileType(9).with_copy(2),
        );
        assert!(result.is_some());
    }

    #[test]
    fn yaku_completed_by_this_wait_is_valid_under_partial_wait_rule() {
        let hand_types = vec![
            TileType(0),
            TileType(1),
            TileType(2),
            TileType(3),
            TileType(4),
            TileType(5),
            TileType(6),
            TileType(7),
            TileType(8),
            TileType(9),
            TileType(9),
            TileType::HAKU,
            TileType::HAKU,
            TileType::HAKU,
        ];
        let all_tiles = hand_types
            .iter()
            .enumerate()
            .map(|(i, &tile_type)| tile_type.with_copy((i % 4) as u8))
            .collect::<Vec<_>>();
        let result = check_win(
            &all_tiles,
            &hand_types,
            &context(false, Vec::new()),
            false,
            TileType::HAKU.with_copy(3),
        );
        assert!(result.is_some());
    }

    fn sequence(tile_type: TileType) -> Mentsu {
        Mentsu {
            kind: MentsuKind::Shuntsu,
            tile_type,
            is_open: false,
        }
    }

    fn triplet(tile_type: TileType) -> Mentsu {
        Mentsu {
            kind: MentsuKind::Koutsu,
            tile_type,
            is_open: false,
        }
    }

    fn yaku_han(yaku: &[crate::types::YakuResult], target: YakuName) -> Option<u8> {
        yaku.iter()
            .find(|result| result.yaku == target)
            .map(|result| result.han)
    }

    fn parse_hand(spec: &str) -> Vec<TileType> {
        let mut result = Vec::new();
        let mut digits = Vec::new();
        for ch in spec.chars() {
            match ch {
                '1'..='9' => digits.push(ch as u8 - b'0'),
                'm' | 'p' | 's' | 'z' => {
                    let base = match ch {
                        'm' => 0,
                        'p' => 9,
                        's' => 18,
                        'z' => 27,
                        _ => unreachable!(),
                    };
                    result.extend(digits.drain(..).map(|rank| TileType(base + rank - 1)));
                }
                ' ' => {}
                _ => panic!("invalid test hand character: {ch}"),
            }
        }
        assert!(digits.is_empty(), "unfinished test hand segment: {spec}");
        result
    }

    fn physical_tiles(types: &[TileType]) -> Vec<riichi_core::tile::Tile> {
        let mut next_copy = [0u8; 34];
        types
            .iter()
            .map(|&tile_type| {
                let copy = next_copy[tile_type.0 as usize];
                next_copy[tile_type.0 as usize] += 1;
                tile_type.with_copy(copy)
            })
            .collect()
    }

    fn evaluate_closed(
        spec: &str,
        winning_tile: TileType,
        is_tsumo: bool,
    ) -> crate::types::WinResult {
        let hand_types = parse_hand(spec);
        assert_eq!(hand_types.len(), 14);
        let all_tiles = physical_tiles(&hand_types);
        let mut ctx = context(true, Vec::new());
        ctx.is_tsumo = is_tsumo;
        check_win(
            &all_tiles,
            &hand_types,
            &ctx,
            false,
            winning_tile.with_copy(3),
        )
        .expect("test hand should be a valid win")
    }

    #[test]
    fn open_yaku_use_their_reduced_han_values() {
        let open_chi = |tiles: &[TileType]| {
            let actual: Vec<_> = tiles
                .iter()
                .enumerate()
                .map(|(i, &t)| t.with_copy(i as u8))
                .collect();
            Meld::chi(actual.clone(), actual[0], PlayerId(0))
        };

        // 副露混一色：2 翻。
        let honitsu_meld = open_chi(&[TileType(0), TileType(1), TileType(2)]);
        let honitsu_hand = WinningHand {
            hand_type: HandType::Standard,
            jantai: TileType::HAKU,
            mentsu: vec![
                sequence(TileType(3)),
                sequence(TileType(6)),
                triplet(TileType(0)),
            ],
        };
        let mut honitsu_ctx = context(true, vec![honitsu_meld]);
        honitsu_ctx.is_tsumo = true;
        let yaku = detect_yaku(&[honitsu_hand], &honitsu_ctx, TileType(3).with_copy(0));
        assert_eq!(yaku_han(&yaku, YakuName::Honitsu), Some(2));

        // 副露一气通贯：1 翻。
        let ittsu_meld = open_chi(&[TileType(0), TileType(1), TileType(2)]);
        let ittsu_hand = WinningHand {
            hand_type: HandType::Standard,
            jantai: TileType(9),
            mentsu: vec![
                sequence(TileType(3)),
                sequence(TileType(6)),
                triplet(TileType(9)),
            ],
        };
        let ittsu_ctx = context(true, vec![ittsu_meld]);
        let yaku = detect_yaku(&[ittsu_hand], &ittsu_ctx, TileType(3).with_copy(0));
        assert_eq!(yaku_han(&yaku, YakuName::Ittsu), Some(1));
    }

    #[test]
    fn mortal_chiitoitsu_case_keeps_tanyao_and_fixed_25_fu() {
        let result = evaluate_closed("2255m445p667788s5p", TileType(13), false);
        assert_eq!(result.fu, 25);
        assert_eq!(result.total_han, 3);
        assert_eq!(
            yaku_han(&result.yaku_results, YakuName::Chiitoitsu),
            Some(2)
        );
        assert_eq!(yaku_han(&result.yaku_results, YakuName::Tanyao), Some(1));
    }

    #[test]
    fn mortal_ryanpeiko_pinfu_case_selects_the_highest_decomposition() {
        let pinfu = evaluate_closed("666677778888m99p", TileType(7), false);
        assert_eq!(pinfu.fu, 30);
        assert_eq!(pinfu.total_han, 4);
        assert_eq!(yaku_han(&pinfu.yaku_results, YakuName::Ryanpeiko), Some(3));
        assert_eq!(yaku_han(&pinfu.yaku_results, YakuName::Pinfu), Some(1));

        let ryanpeiko = evaluate_closed("666677778888m99p", TileType(6), false);
        assert_eq!(ryanpeiko.fu, 40);
        assert_eq!(ryanpeiko.total_han, 3);
        assert_eq!(
            yaku_han(&ryanpeiko.yaku_results, YakuName::Ryanpeiko),
            Some(3)
        );
        assert_eq!(yaku_han(&ryanpeiko.yaku_results, YakuName::Pinfu), None);
    }

    #[test]
    fn all_open_sanshoku_patterns_are_detected() {
        let open_meld = |kind: MeldKind, tile_type: TileType| {
            let tiles: Vec<_> = (0..3).map(|copy| tile_type.with_copy(copy)).collect();
            match kind {
                MeldKind::Chi => {
                    let called = tiles[0];
                    Meld::chi(tiles, called, PlayerId(0))
                }
                _ => {
                    let called = tiles[0];
                    Meld::pon(tiles, called, PlayerId(0))
                }
            }
        };

        let sequence_melds = vec![
            Meld::chi(
                vec![
                    TileType(0).with_copy(0),
                    TileType(1).with_copy(0),
                    TileType(2).with_copy(0),
                ],
                TileType(0).with_copy(0),
                PlayerId(0),
            ),
            Meld::chi(
                vec![
                    TileType(9).with_copy(0),
                    TileType(10).with_copy(0),
                    TileType(11).with_copy(0),
                ],
                TileType(9).with_copy(0),
                PlayerId(1),
            ),
            Meld::chi(
                vec![
                    TileType(18).with_copy(0),
                    TileType(19).with_copy(0),
                    TileType(20).with_copy(0),
                ],
                TileType(18).with_copy(0),
                PlayerId(2),
            ),
        ];
        let sequence_hand = WinningHand {
            hand_type: HandType::Standard,
            jantai: TileType(17),
            mentsu: vec![triplet(TileType(8))],
        };
        let yaku = detect_yaku(
            &[sequence_hand],
            &context(true, sequence_melds),
            TileType(17).with_copy(0),
        );
        assert_eq!(yaku_han(&yaku, YakuName::SanshokuDoujun), Some(1));
        assert_eq!(yaku_han(&yaku, YakuName::Junchan), Some(2));

        let triplet_melds = vec![
            open_meld(MeldKind::Pon, TileType(1)),
            open_meld(MeldKind::Pon, TileType(10)),
            open_meld(MeldKind::Pon, TileType(19)),
        ];
        let triplet_hand = WinningHand {
            hand_type: HandType::Standard,
            jantai: TileType(13),
            mentsu: vec![sequence(TileType(0))],
        };
        let yaku = detect_yaku(
            &[triplet_hand],
            &context(true, triplet_melds),
            TileType(13).with_copy(0),
        );
        assert_eq!(yaku_han(&yaku, YakuName::SanshokuDoukou), Some(2));
    }

    #[test]
    fn double_riichi_does_not_also_count_riichi() {
        let hand = WinningHand {
            hand_type: HandType::Standard,
            jantai: TileType(8),
            mentsu: vec![
                sequence(TileType(0)),
                sequence(TileType(3)),
                sequence(TileType(6)),
                sequence(TileType(1)),
            ],
        };
        let mut ctx = context(true, Vec::new());
        ctx.is_riichi = true;
        ctx.is_double_riichi = true;
        let yaku = detect_yaku(&[hand], &ctx, TileType(1).with_copy(0));
        assert_eq!(yaku_han(&yaku, YakuName::DoubleRiichi), Some(2));
        assert_eq!(yaku_han(&yaku, YakuName::Riichi), None);
    }

    #[test]
    fn thirteen_sided_kokushi_and_pure_chuuren_are_distinguished() {
        let kokushi_types: Vec<TileType> = TileType::YAOCHUUHAI
            .into_iter()
            .chain(std::iter::once(TileType::MAN1))
            .collect();
        let kokushi_tiles: Vec<_> = kokushi_types
            .iter()
            .enumerate()
            .map(|(index, &tile_type)| tile_type.with_copy((index % 4) as u8))
            .collect();
        let kokushi = check_win(
            &kokushi_tiles,
            &kokushi_types,
            &context(true, Vec::new()),
            false,
            TileType::MAN1.with_copy(3),
        )
        .expect("国士十三面应当和牌");
        assert!(yaku_han(&kokushi.yaku_results, YakuName::Kokushi13).is_some());

        let mut chuuren_types = Vec::new();
        for (rank, count) in [3, 1, 1, 1, 1, 1, 1, 1, 3].into_iter().enumerate() {
            chuuren_types.extend(std::iter::repeat_n(TileType(rank as u8), count));
        }
        chuuren_types.push(TileType(4));
        let chuuren_tiles: Vec<_> = chuuren_types
            .iter()
            .enumerate()
            .map(|(index, &tile_type)| tile_type.with_copy((index % 4) as u8))
            .collect();
        let pure = check_win(
            &chuuren_tiles,
            &chuuren_types,
            &context(true, Vec::new()),
            false,
            TileType(4).with_copy(3),
        )
        .expect("纯正九莲应当和牌");
        assert!(yaku_han(&pure.yaku_results, YakuName::ChuurenPoutou9).is_some());

        let regular = check_win(
            &chuuren_tiles,
            &chuuren_types,
            &context(true, Vec::new()),
            false,
            TileType(0).with_copy(3),
        )
        .expect("九莲宝灯应当和牌");
        assert!(yaku_han(&regular.yaku_results, YakuName::ChuurenPoutou).is_some());
        assert_eq!(
            yaku_han(&regular.yaku_results, YakuName::ChuurenPoutou9),
            None
        );
    }

    #[test]
    fn yakuman_result_does_not_include_regular_yaku_or_dora() {
        let hand_types = vec![
            TileType(0),
            TileType(0),
            TileType(0),
            TileType(8),
            TileType(8),
            TileType(8),
            TileType(9),
            TileType(9),
            TileType(9),
            TileType(17),
            TileType(17),
            TileType(17),
            TileType(18),
            TileType(18),
        ];
        let all_tiles = hand_types
            .iter()
            .enumerate()
            .map(|(index, &tile_type)| tile_type.with_copy((index % 4) as u8))
            .collect::<Vec<_>>();
        let result = check_win(
            &all_tiles,
            &hand_types,
            &context(true, Vec::new()),
            false,
            TileType(18).with_copy(3),
        )
        .expect("清老头应当和牌");
        assert!(yaku_han(&result.yaku_results, YakuName::Chinroutou).is_some());
        assert_eq!(yaku_han(&result.yaku_results, YakuName::Honroutou), None);
        assert!(result.yaku_results.iter().all(|yaku| yaku.han >= 13));
    }
}
