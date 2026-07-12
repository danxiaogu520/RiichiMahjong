// ═══════════════════════════════════════════════════════════════
//  算点：计算点棒变化
// ═══════════════════════════════════════════════════════════════

/// 计算和了后的点棒变化
///
/// # 参数
/// - `total_han`: 总翻数
/// - `total_fu`: 总符数
/// - `yakuman_count`: 役满倍数（0=非役满，1=役满，2=双倍役满，...）
/// - `winner`: 和了玩家座位号 (0-3)
/// - `dealer`: 庄家座位号 (0-3)
/// - `riichi_sticks`: 场上立直棒数
/// - `honba`: 本场数
/// - `is_tsumo`: 是否自摸
///
/// # 返回
/// `[i32; 4]` — 4 家的点数变化
#[allow(clippy::too_many_arguments, clippy::needless_range_loop)]
pub fn calculate_points(
    total_han: u8,
    total_fu: u32,
    yakuman_count: u8,
    winner: usize,
    dealer: usize,
    riichi_sticks: u32,
    honba: u32,
    is_tsumo: bool,
) -> [i32; 4] {
    calculate_points_with_loser(
        total_han,
        total_fu,
        yakuman_count,
        winner,
        None,
        dealer,
        riichi_sticks,
        honba,
        is_tsumo,
    )
}

/// 计算和了后的点数变化，并在荣和时扣除指定放铳者。
///
/// `loser` 为 `Some` 时仅用于荣和；自摸时必须为 `None`。
#[allow(clippy::too_many_arguments, clippy::needless_range_loop)]
pub fn calculate_points_with_loser(
    total_han: u8,
    total_fu: u32,
    yakuman_count: u8,
    winner: usize,
    loser: Option<usize>,
    dealer: usize,
    riichi_sticks: u32,
    honba: u32,
    is_tsumo: bool,
) -> [i32; 4] {
    // 一本场总计增加 300 点：荣和由放铳者支付，自摸由三家各支付 100 点。
    let honba_val = (honba * 100) as i32;
    let riichi_bonus = (riichi_sticks * 1000) as i32;
    let is_dealer = winner == dealer;

    let bp = if yakuman_count > 0 {
        8000 * yakuman_count as i32
    } else {
        base_points(total_han, total_fu)
    };

    let mut changes = [0i32; 4];

    if is_tsumo {
        if is_dealer {
            // 庄家自摸：闲家每人付 ⌈2bp⌉ + honba
            let each_pay = round_up_100(bp * 2) + honba_val;
            for i in 0..4 {
                if i == winner {
                    changes[i] = each_pay * 3 + riichi_bonus;
                } else {
                    changes[i] = -each_pay;
                }
            }
        } else {
            // 闲家自摸：庄家付 ⌈2bp⌉ + honba，闲家付 ⌈bp⌉ + honba
            let dealer_pay = round_up_100(bp * 2) + honba_val;
            let other_pay = round_up_100(bp) + honba_val;
            for i in 0..4 {
                if i == winner {
                    changes[i] = dealer_pay + other_pay * 2 + riichi_bonus;
                } else if i == dealer {
                    changes[i] = -dealer_pay;
                } else {
                    changes[i] = -other_pay;
                }
            }
        }
    } else {
        // 荣和
        let hand_pay = if is_dealer {
            round_up_100(bp * 6)
        } else {
            round_up_100(bp * 4)
        };
        let pay = hand_pay + (honba * 300) as i32;

        changes[winner] = pay + riichi_bonus;
        if let Some(loser) = loser {
            if loser < changes.len() && loser != winner {
                changes[loser] -= pay;
            }
        }
    }

    changes
}

/// 计算基础点数 bp = fu × 2^(han+2)
///
/// 满贯以上查固定表
fn base_points(han: u8, fu: u32) -> i32 {
    match han {
        0 => 0,
        // 1～4翻仍须按满贯基础点 2000 封顶。
        1..=4 => (fu as i32 * (1 << (han as u32 + 2))).min(2000),
        5 => 2000,       // 满贯
        6..=7 => 3000,   // 跳满
        8..=10 => 4000,  // 倍满
        11..=12 => 6000, // 三倍满
        _ => 8000,       // 役满（非 yakuman_count 时的兜底）
    }
}

/// 向上取整到百位
fn round_up_100(n: i32) -> i32 {
    ((n + 99) / 100) * 100
}

#[cfg(test)]
mod tests {
    use super::calculate_points_with_loser;

    #[test]
    fn ron_transfers_points_from_loser() {
        let changes = calculate_points_with_loser(1, 30, 0, 0, Some(2), 1, 0, 0, false);
        assert_eq!(changes, [1000, 0, -1000, 0]);
        assert_eq!(changes.iter().sum::<i32>(), 0);
    }

    #[test]
    fn honba_is_three_hundred_points_on_ron() {
        let changes = calculate_points_with_loser(1, 30, 0, 0, Some(2), 1, 0, 2, false);
        assert_eq!(changes, [1600, 0, -1600, 0]);
    }

    #[test]
    fn tsumo_honba_is_one_hundred_per_opponent() {
        let changes = calculate_points_with_loser(1, 30, 0, 0, None, 1, 0, 2, true);
        assert_eq!(changes, [1700, -700, -500, -500]);
        assert_eq!(changes.iter().sum::<i32>(), 0);
    }

    #[test]
    fn double_yakuman_uses_two_yakuman_sticks() {
        let changes = calculate_points_with_loser(26, 0, 2, 0, Some(2), 1, 0, 0, false);
        assert_eq!(changes, [64_000, 0, -64_000, 0]);
    }

    #[test]
    fn ron_riichi_sticks_are_not_paid_by_the_loser() {
        let changes = calculate_points_with_loser(1, 30, 0, 0, Some(2), 1, 2, 0, false);
        assert_eq!(changes, [3_000, 0, -1_000, 0]);
    }

    #[test]
    fn high_fu_is_capped_at_mangan() {
        let four_han = calculate_points_with_loser(4, 40, 0, 0, Some(2), 1, 0, 0, false);
        let three_han = calculate_points_with_loser(3, 70, 0, 0, Some(2), 1, 0, 0, false);
        assert_eq!(four_han, [8_000, 0, -8_000, 0]);
        assert_eq!(three_han, [8_000, 0, -8_000, 0]);
    }
}
