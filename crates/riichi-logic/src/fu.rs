use riichi_core::meld::{Meld, MeldKind};
use riichi_core::tile::TileType;

use crate::types::{HandType, MentsuKind, WinningHand, YakuName, YakuResult};

/// 计算符数
///
/// # 参数
/// - `hand`: 手牌分解结果
/// - `melds`: 副露列表
/// - `yaku_results`: 役结果（用于检测平和、七对子等特殊规则）
/// - `is_tsumo`: 是否自摸
/// - `seat_wind`: 自风
/// - `field_wind`: 场风
///
/// # 特殊规则
/// - 七对子：固定 25 符
/// - 平和 + 自摸：固定 20 符
/// - 平和 + 荣和：固定 30 符
/// - 国士无双：固定 20 符
///
/// # 一般计算
/// 底符 20，然后累加各面子/雀头/听牌类型的符数
pub fn calculate_fu(
    hand: &WinningHand,
    melds: &[Meld],
    yaku_results: &[YakuResult],
    is_tsumo: bool,
    seat_wind: TileType,
    field_wind: TileType,
) -> u32 {
    calculate_fu_with_winning_tile(
        hand,
        melds,
        yaku_results,
        is_tsumo,
        seat_wind,
        field_wind,
        None,
    )
}

/// 计算符数，并在提供和了牌时计算单骑、边张、坎张符。
pub fn calculate_fu_with_winning_tile(
    hand: &WinningHand,
    melds: &[Meld],
    yaku_results: &[YakuResult],
    is_tsumo: bool,
    seat_wind: TileType,
    field_wind: TileType,
    winning_tile: Option<TileType>,
) -> u32 {
    let has_pinfu = yaku_results.iter().any(|y| y.yaku == YakuName::Pinfu);
    let has_chiitoitsu = yaku_results.iter().any(|y| y.yaku == YakuName::Chiitoitsu);

    // 七对子：固定 25 符
    if has_chiitoitsu {
        return 25;
    }

    // 国士无双：固定 20 符
    if hand.hand_type == HandType::Kokushi {
        return 20;
    }

    // 平和
    if has_pinfu {
        if is_tsumo {
            return 20;
        } else {
            return 30;
        }
    }

    // 一般计算
    let mut fu = 20u32; // 底符

    // 自摸 +2（平和除外，已在上面处理）
    if is_tsumo {
        fu += 2;
    }

    // 门清荣和 +10
    if !is_tsumo && melds.iter().all(|m| m.is_concealed()) {
        fu += 10;
    }

    // 雀头符
    let jantai = hand.jantai;
    if jantai.is_dragon() {
        fu += 2; // 三元牌雀头
    }
    if jantai == seat_wind {
        fu += 2; // 自风雀头
    }
    if jantai == field_wind {
        fu += 2; // 场风雀头；连风牌雀头与自风分别计符，共 4 符
    }

    // 面子符（手牌中的面子）
    for m in &hand.mentsu {
        let is_yao = m.tile_type.is_yaochuuhai();
        match m.kind {
            MentsuKind::Koutsu => {
                // 荣和牌正好补成门清刻子时，该分解中的刻子按明刻计符。
                // 如果同一张牌也能组成顺子，分解器会分别产生候选，取高点法
                // 时由上层选择得点更高的分解。
                let winning_tile_makes_minkou =
                    !is_tsumo && winning_tile == Some(m.tile_type) && !m.is_open;
                if m.is_open || winning_tile_makes_minkou {
                    fu += if is_yao { 4 } else { 2 };
                } else {
                    fu += if is_yao { 8 } else { 4 };
                }
            }
            MentsuKind::Shuntsu => {} // 顺子无符
        }
    }

    // 副露面子符（明杠/暗杠）
    for meld in melds {
        let tt = meld.tiles[0].tile_type();
        let is_yao = tt.is_yaochuuhai();
        match meld.kind {
            MeldKind::Ankan => {
                fu += if is_yao { 32 } else { 16 };
            }
            MeldKind::Pon => {
                fu += if is_yao { 4 } else { 2 };
            }
            MeldKind::Minkan | MeldKind::Kakan => {
                fu += if is_yao { 16 } else { 8 };
            }
            MeldKind::Chi => {} // 吃没有符
        }
    }

    // 听牌类型符（单骑/边张/坎张 +2）。
    // 和了牌可能同时出现在多个候选分解中，取该分解能成立的最高符值。
    if let Some(winning_tile) = winning_tile {
        let mut wait_fu = 0;
        if hand.jantai == winning_tile {
            wait_fu = 2; // 单骑
        }
        for m in &hand.mentsu {
            if m.kind != MentsuKind::Shuntsu {
                continue;
            }
            let start = m.tile_type.rank().0;
            let win_rank = winning_tile.rank().0;
            if m.tile_type.suit() != winning_tile.suit() {
                continue;
            }
            // 只有 12 听 3、89 听 7 才是边张；
            // 123 听 1 与 789 听 9 属于两面听，不加边张符。
            let edge_wait = (start == 1 && win_rank == 3) || (start == 7 && win_rank == 7);
            let closed_wait = (2..=7).contains(&start) && win_rank == start + 1;
            if edge_wait || closed_wait {
                wait_fu = 2;
            }
        }
        fu += wait_fu;
    }

    // 向上取整到 10
    if fu > 0 && !fu.is_multiple_of(10) {
        fu = (fu / 10 + 1) * 10;
    }

    // 最低 30 符（七对子已单独处理）
    if fu < 30 {
        fu = 30;
    }

    fu
}

#[cfg(test)]
mod tests {
    use super::calculate_fu_with_winning_tile;
    use crate::types::{HandType, Mentsu, MentsuKind, WinningHand};
    use riichi_core::meld::Meld;
    use riichi_core::player::PlayerId;
    use riichi_core::tile::TileType;

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

    #[test]
    fn ron_completed_triplet_uses_minkou_fu() {
        let hand = WinningHand {
            hand_type: HandType::Standard,
            jantai: TileType::EAST,
            mentsu: vec![
                triplet(TileType(4)),
                triplet(TileType(14)),
                sequence(TileType(18)),
                sequence(TileType(21)),
            ],
        };
        let fu = calculate_fu_with_winning_tile(
            &hand,
            &[],
            &[],
            false,
            TileType::EAST,
            TileType::EAST,
            Some(TileType(4)),
        );
        // 底符20 + 门清荣和10 + 连风雀头4 + 明刻2 + 暗刻4 = 40符。
        assert_eq!(fu, 40);
    }

    #[test]
    fn edge_wait_fu_only_applies_to_the_edge_side() {
        let hand = WinningHand {
            hand_type: HandType::Standard,
            jantai: TileType(10),
            mentsu: vec![
                sequence(TileType(0)),
                sequence(TileType(3)),
                sequence(TileType(6)),
                sequence(TileType(1)),
            ],
        };

        let two_sided = calculate_fu_with_winning_tile(
            &hand,
            &[],
            &[],
            false,
            TileType::EAST,
            TileType::EAST,
            Some(TileType(0)),
        );
        let edge = calculate_fu_with_winning_tile(
            &hand,
            &[],
            &[],
            false,
            TileType::EAST,
            TileType::EAST,
            Some(TileType(2)),
        );

        assert_eq!(two_sided, 30);
        assert_eq!(edge, 40);
    }

    #[test]
    fn open_pon_contributes_two_or_four_fu() {
        let hand = WinningHand {
            hand_type: HandType::Standard,
            jantai: TileType(10),
            mentsu: vec![
                triplet(TileType(1)),
                triplet(TileType(2)),
                sequence(TileType(12)),
            ],
        };
        let simple_pon = Meld::pon(
            vec![
                TileType(22).with_copy(0),
                TileType(22).with_copy(1),
                TileType(22).with_copy(2),
            ],
            TileType(22).with_copy(0),
            PlayerId(1),
        );
        let terminal_pon = Meld::pon(
            vec![
                TileType(0).with_copy(0),
                TileType(0).with_copy(1),
                TileType(0).with_copy(2),
            ],
            TileType(0).with_copy(0),
            PlayerId(1),
        );

        let simple_fu = calculate_fu_with_winning_tile(
            &hand,
            &[simple_pon],
            &[],
            false,
            TileType::EAST,
            TileType::EAST,
            Some(TileType(12)),
        );
        let terminal_fu = calculate_fu_with_winning_tile(
            &hand,
            &[terminal_pon],
            &[],
            false,
            TileType::EAST,
            TileType::EAST,
            Some(TileType(12)),
        );

        assert_eq!(simple_fu, 30);
        assert_eq!(terminal_fu, 40);
    }
}
