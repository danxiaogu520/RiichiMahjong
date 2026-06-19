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
        let pay = if is_dealer {
            round_up_100(bp * 6)
        } else {
            round_up_100(bp * 4)
        } + honba_val * 3
            + riichi_bonus;

        changes[winner] = pay;
        // 荣和时 loser 必须有效，但这里由调用方保证
        // 点数变化数组只描述变化量，具体谁付由调用方设置
    }

    changes
}

/// 计算基础点数 bp = fu × 2^(han+2)
///
/// 满贯以上查固定表
fn base_points(han: u8, fu: u32) -> i32 {
    match han {
        0 => 0,
        1..=4 => fu as i32 * (1 << (han as u32 + 2)),
        5 => 2000,  // 满贯
        6..=7 => 3000,  // 跳满
        8..=10 => 4000, // 倍满
        11..=12 => 6000, // 三倍满
        _ => 8000,  // 役满（非 yakuman_count 时的兜底）
    }
}

/// 向上取整到百位
fn round_up_100(n: i32) -> i32 {
    ((n + 99) / 100) * 100
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tsumo_dealer() {
        // 庄家自摸 1000 all
        let changes = calculate_points(1, 30, 0, 0, 0, 0, 0, true);
        // bp = 30 * 2^3 = 240, each = ⌈480⌉ + 0 = 500
        assert_eq!(changes[0], 1500);
        assert_eq!(changes[1], -500);
        assert_eq!(changes[2], -500);
        assert_eq!(changes[3], -500);
    }

    #[test]
    fn test_basic_tsumo_non_dealer() {
        // 闲家自摸 30符1翻
        let changes = calculate_points(1, 30, 0, 1, 0, 0, 0, true);
        // bp = 240, dealer_pay = ⌈480⌉ = 500, other_pay = ⌈240⌉ = 300
        assert_eq!(changes[1], 500 + 300 * 2); // 1100
        assert_eq!(changes[0], -500); // dealer
        assert_eq!(changes[2], -300);
        assert_eq!(changes[3], -300);
    }

    #[test]
    fn test_ron_non_dealer() {
        // 闲家荣和 30符1翻
        let changes = calculate_points(1, 30, 0, 1, 0, 0, 0, false);
        // bp = 240, pay = ⌈960⌉ = 1000
        assert_eq!(changes[1], 1000);
    }

    #[test]
    fn test_ron_dealer() {
        // 庄家荣和 30符1翻
        let changes = calculate_points(1, 30, 0, 0, 0, 0, 0, false);
        // bp = 240, pay = ⌈1440⌉ = 1500
        assert_eq!(changes[0], 1500);
    }

    #[test]
    fn test_haneman() {
        // 跳满 6翻
        let changes = calculate_points(6, 30, 0, 0, 0, 0, 0, false);
        assert_eq!(changes[0], 12000); // 庄家荣和跳满
    }

    #[test]
    fn test_yakuman() {
        // 役满
        let changes = calculate_points(0, 0, 1, 1, 0, 0, 0, false);
        assert_eq!(changes[1], 32000); // 闲家荣和役满
    }

    #[test]
    fn test_yakuman_dealer() {
        // 庄家役满
        let changes = calculate_points(0, 0, 1, 0, 0, 0, 0, false);
        assert_eq!(changes[0], 48000); // 庄家荣和役满
    }

    #[test]
    fn test_with_honba() {
        // 闲家荣和 + 2本场
        let changes = calculate_points(1, 30, 0, 1, 0, 0, 2, false);
        // pay = ⌈960⌉ + 600 = 1600
        assert_eq!(changes[1], 1600);
    }

    #[test]
    fn test_with_riichi_sticks() {
        // 闲家荣和 + 1根立直棒
        let changes = calculate_points(1, 30, 0, 1, 0, 1, 0, false);
        // pay = ⌈960⌉ + 1000 = 2000
        assert_eq!(changes[1], 2000);
    }
}
