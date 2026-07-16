use crate::dora::calculate_dora;
use crate::fu::calculate_fu;
use crate::model::{
    DoraResult, WinInput, WinResult, WinningHand, WinningTilePlacement, YakuResult,
};
use crate::scoring::calculate_points;
use crate::shape::{decompose_hand_with_open_melds, is_win_shape_with_open_melds};
use crate::yaku::detect_yaku;

/// 完整和牌评估：判形、判役、计符、计点，并按高点法选择最终结果。
///
/// `WinInput` 是和牌评估唯一入口，固定规则不通过参数注入。
pub fn evaluate_win(input: WinInput<'_>) -> Option<WinResult> {
    // ── Step 1: 判振 ──
    if input.is_furiten && !input.situation.is_tsumo {
        return None;
    }

    let mut hand_tiles: Vec<_> = input
        .concealed_tiles
        .iter()
        .map(|tile| tile.tile_type())
        .collect();
    hand_tiles.push(input.winning_tile.tile_type());

    let mut all_tiles = input.concealed_tiles.to_vec();
    for meld in input.melds {
        all_tiles.extend_from_slice(&meld.tiles);
    }
    all_tiles.push(input.winning_tile);

    // ── Step 2: 判形 ──
    if !is_win_shape_with_open_melds(&hand_tiles, input.melds.len()) {
        return None;
    }

    // ── Step 3: 判役 ──
    let decompositions = decompose_hand_with_open_melds(&hand_tiles, input.melds.len());
    if decompositions.is_empty() {
        return None;
    }

    // ── Step 4: 对“牌组分解 × 和牌归属”独立计算役、符和点数 ──
    // Mortal 会在刻子/顺子歧义时优先保留暗刻；这里显式枚举所有合法归属，
    // 再统一按最终得点取最高，避免判役和计符采用互相矛盾的解释。
    let dora_result = calculate_dora(
        &all_tiles,
        input.dora_indicators,
        input.ura_dora_indicators,
        input.situation.is_riichi,
    );
    let mut best_result: Option<WinResult> = None;
    for hand in &decompositions {
        for placement in hand.winning_tile_placements(input.winning_tile.tile_type()) {
            let Some(candidate) = evaluate_candidate(input, hand, placement, &dora_result) else {
                continue;
            };
            let is_better = match best_result.as_ref() {
                None => true,
                Some(best) => {
                    (
                        candidate.points[input.settlement.winner],
                        candidate.total_han,
                        candidate.fu,
                    ) > (
                        best.points[input.settlement.winner],
                        best.total_han,
                        best.fu,
                    )
                }
            };
            if is_better {
                best_result = Some(candidate);
            }
        }
    }

    best_result
}

fn evaluate_candidate(
    input: WinInput<'_>,
    hand: &WinningHand,
    placement: WinningTilePlacement,
    dora_result: &DoraResult,
) -> Option<WinResult> {
    let yaku_results = detect_yaku(
        hand,
        input.situation,
        input.melds,
        input.winning_tile,
        placement,
    );
    if yaku_results.is_empty() {
        return None;
    }

    // 后付/片和了按当前“分解 + 和牌归属”判定；宝牌本身不能构成役。
    let mut all_yaku = yaku_results;
    if dora_result.dora > 0 {
        all_yaku.push(YakuResult::new(
            crate::model::YakuName::Dora,
            dora_result.dora,
        ));
    }
    if dora_result.aka_dora > 0 {
        all_yaku.push(YakuResult::new(
            crate::model::YakuName::AkaDora,
            dora_result.aka_dora,
        ));
    }
    if dora_result.ura_dora > 0 {
        all_yaku.push(YakuResult::new(
            crate::model::YakuName::UraDora,
            dora_result.ura_dora,
        ));
    }

    // 役满结算不再叠加普通役和宝牌；多个役满之间仍然保留并累计。
    if all_yaku.iter().any(|result| result.han >= 13) {
        all_yaku.retain(|result| result.han >= 13);
    }

    let fu = calculate_fu(
        hand,
        input.melds,
        &all_yaku,
        input.situation,
        input.winning_tile.tile_type(),
        placement,
    );
    let total_han: u8 = all_yaku.iter().map(|y| y.han).sum();
    let yakuman_count: u8 = all_yaku
        .iter()
        .filter(|y| y.han >= 13)
        .map(|y| (y.han / 13).max(1))
        .sum();
    let points = calculate_points(
        total_han,
        fu,
        yakuman_count,
        input.settlement,
        input.situation.is_tsumo,
    );
    Some(WinResult {
        yaku_results: all_yaku,
        total_han,
        fu,
        points,
    })
}

#[cfg(test)]
#[path = "evaluation_tests.rs"]
mod tests;
