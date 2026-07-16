use riichi_core::meld::{Meld, MeldKind};
use riichi_core::tile::TileType;

use crate::model::{
    MentsuKind, WinSituation, WinningHand, WinningTilePlacement, YakuName, YakuResult,
};

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
    situation: &WinSituation,
    winning_tile: TileType,
    placement: WinningTilePlacement,
) -> u32 {
    let has_pinfu = yaku_results.iter().any(|y| y.yaku == YakuName::Pinfu);
    let has_chiitoitsu = yaku_results.iter().any(|y| y.yaku == YakuName::Chiitoitsu);

    // 七对子：固定 25 符
    if has_chiitoitsu {
        return 25;
    }

    // 国士无双：固定 20 符
    if matches!(hand, WinningHand::Kokushi { .. }) {
        return 20;
    }

    // 平和
    if has_pinfu {
        if situation.is_tsumo {
            return 20;
        } else {
            return 30;
        }
    }

    // 一般计算
    let mut fu = 20u32; // 底符

    // 自摸 +2（平和除外，已在上面处理）
    if situation.is_tsumo {
        fu += 2;
    }

    // 门清荣和 +10
    if !situation.is_tsumo && melds.iter().all(|m| m.is_concealed()) {
        fu += 10;
    }

    // 雀头符
    let jantai = hand.pair();
    if jantai.is_dragon() {
        fu += 2; // 三元牌雀头
    }
    if jantai == situation.seat_wind {
        fu += 2; // 自风雀头
    }
    if jantai == situation.field_wind {
        fu += 2; // 场风雀头；连风牌雀头与自风分别计符，共 4 符
    }

    // 面子符（手牌中的面子）
    for (index, m) in hand.groups().iter().enumerate() {
        let is_yao = m.tile_type.is_yaochuuhai();
        match m.kind {
            MentsuKind::Koutsu => {
                // 荣和牌正好补成门清刻子时，该分解中的刻子按明刻计符。
                // 如果同一张牌也能组成顺子，评估层会为同一分解枚举不同的
                // 和牌归属，再按高点法选择得点更高的候选。
                let winning_tile_makes_minkou = !situation.is_tsumo
                    && placement == WinningTilePlacement::Group(index)
                    && winning_tile == m.tile_type;
                if winning_tile_makes_minkou {
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

    // 听牌类型符（单骑/边张/坎张 +2）。判役与计符共用同一个和牌归属，
    // 避免把“荣和补刻子”和“顺子坎张”两种互斥解释拼在一起。
    let wait_fu = match placement {
        WinningTilePlacement::Pair => 2,
        WinningTilePlacement::Group(index) => hand
            .groups()
            .get(index)
            .filter(|group| group.kind == MentsuKind::Shuntsu)
            .map_or(0, |m| {
                let start = m.tile_type.rank().0;
                let win_rank = winning_tile.rank().0;
                if m.tile_type.suit() != winning_tile.suit() {
                    return 0;
                }
                // 只有 12 听 3、89 听 7 才是边张；
                // 123 听 1 与 789 听 9 属于两面听，不加边张符。
                let edge_wait = (start == 1 && win_rank == 3) || (start == 7 && win_rank == 7);
                let closed_wait = (2..=7).contains(&start) && win_rank == start + 1;
                if edge_wait || closed_wait {
                    2
                } else {
                    0
                }
            }),
        WinningTilePlacement::Special => 0,
    };
    fu += wait_fu;

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
    use super::calculate_fu;
    use crate::model::{ClosedGroup, MentsuKind, WinSituation, WinningHand, WinningTilePlacement};
    use riichi_core::meld::Meld;
    use riichi_core::player::PlayerId;
    use riichi_core::tile::TileType;

    fn sequence(tile_type: TileType) -> ClosedGroup {
        ClosedGroup {
            kind: MentsuKind::Shuntsu,
            tile_type,
        }
    }

    fn triplet(tile_type: TileType) -> ClosedGroup {
        ClosedGroup {
            kind: MentsuKind::Koutsu,
            tile_type,
        }
    }

    fn situation(is_tsumo: bool) -> WinSituation {
        WinSituation {
            is_tsumo,
            is_riichi: false,
            is_double_riichi: false,
            is_ippatsu: false,
            is_rinshan: false,
            is_chankan: false,
            is_haitei: false,
            is_houtei: false,
            is_tenhou: false,
            is_chiihou: false,
            seat_wind: TileType::EAST,
            field_wind: TileType::EAST,
        }
    }

    #[test]
    fn ron_completed_triplet_uses_minkou_fu() {
        let hand = WinningHand::Standard {
            pair: TileType::EAST,
            groups: vec![
                triplet(TileType(4)),
                triplet(TileType(14)),
                sequence(TileType(18)),
                sequence(TileType(21)),
            ],
        };
        let fu = calculate_fu(
            &hand,
            &[],
            &[],
            &situation(false),
            TileType(4),
            WinningTilePlacement::Group(0),
        );
        // 底符20 + 门清荣和10 + 连风雀头4 + 明刻2 + 暗刻4 = 40符。
        assert_eq!(fu, 40);
    }

    #[test]
    fn edge_wait_fu_only_applies_to_the_edge_side() {
        let hand = WinningHand::Standard {
            pair: TileType(10),
            groups: vec![
                sequence(TileType(0)),
                sequence(TileType(3)),
                sequence(TileType(6)),
                sequence(TileType(1)),
            ],
        };

        let two_sided = calculate_fu(
            &hand,
            &[],
            &[],
            &situation(false),
            TileType(0),
            WinningTilePlacement::Group(0),
        );
        let edge = calculate_fu(
            &hand,
            &[],
            &[],
            &situation(false),
            TileType(2),
            WinningTilePlacement::Group(0),
        );

        assert_eq!(two_sided, 30);
        assert_eq!(edge, 40);
    }

    #[test]
    fn open_pon_contributes_two_or_four_fu() {
        let hand = WinningHand::Standard {
            pair: TileType(10),
            groups: vec![
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

        let simple_fu = calculate_fu(
            &hand,
            &[simple_pon],
            &[],
            &situation(false),
            TileType(12),
            WinningTilePlacement::Group(2),
        );
        let terminal_fu = calculate_fu(
            &hand,
            &[terminal_pon],
            &[],
            &situation(false),
            TileType(12),
            WinningTilePlacement::Group(2),
        );

        assert_eq!(simple_fu, 30);
        assert_eq!(terminal_fu, 40);
    }
}
