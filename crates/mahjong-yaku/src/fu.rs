use mahjong_core::meld::{Meld, MeldKind};
use mahjong_core::tile::TileType;

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
    if jantai == field_wind && jantai != seat_wind {
        fu += 2; // 场风雀头（与自风不同时才加）
    }

    // 面子符（手牌中的面子）
    for m in &hand.mentsu {
        let is_yao = m.tile_type.is_yaochuuhai();
        match m.kind {
            MentsuKind::Koutsu => {
                if m.is_open {
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
            MeldKind::Minkan | MeldKind::Kakan => {
                fu += if is_yao { 16 } else { 8 };
            }
            MeldKind::Chi | MeldKind::Pon => {} // 吃碰的符已在面子符中计算
        }
    }

    // 听牌类型符（单骑/边张/坎张 +2）
    // 需要配合和了牌的听牌类型判断，暂时在此处不处理
    // TODO: 听牌类型符

    // 向上取整到 10
    if fu > 0 && fu % 10 != 0 {
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
    use super::*;
    use crate::types::{HandType, Mentsu, MentsuKind, WinningHand};

    fn make_standard_hand(jantai: TileType, mentsu: Vec<Mentsu>) -> WinningHand {
        WinningHand {
            hand_type: HandType::Standard,
            jantai,
            mentsu,
        }
    }

    #[test]
    fn test_chiitoitsu_fixed_25fu() {
        let hand = make_standard_hand(TileType(0), vec![]);
        let yaku = vec![YakuResult::new(YakuName::Chiitoitsu, 2)];
        let fu = calculate_fu(&hand, &[], &yaku, false, TileType::EAST, TileType::EAST);
        assert_eq!(fu, 25);
    }

    #[test]
    fn test_pinfu_tsumo_fixed_20fu() {
        let hand = make_standard_hand(TileType(0), vec![]);
        let yaku = vec![YakuResult::new(YakuName::Pinfu, 1)];
        let fu = calculate_fu(&hand, &[], &yaku, true, TileType::EAST, TileType::EAST);
        assert_eq!(fu, 20);
    }

    #[test]
    fn test_pinfu_ron_fixed_30fu() {
        let hand = make_standard_hand(TileType(0), vec![]);
        let yaku = vec![YakuResult::new(YakuName::Pinfu, 1)];
        let fu = calculate_fu(&hand, &[], &yaku, false, TileType::EAST, TileType::EAST);
        assert_eq!(fu, 30);
    }

    #[test]
    fn test_kokushi_fixed_20fu() {
        let hand = WinningHand {
            hand_type: HandType::Kokushi,
            jantai: TileType(0),
            mentsu: vec![],
        };
        let fu = calculate_fu(&hand, &[], &[], false, TileType::EAST, TileType::EAST);
        assert_eq!(fu, 20);
    }

    #[test]
    fn test_basic_tsumo_no_yaku() {
        // 自摸，无特殊役，底符20 + 自摸2 = 22 → 取整30
        let hand = make_standard_hand(
            TileType(31), // 白雀头 +2
            vec![
                Mentsu { kind: MentsuKind::Shuntsu, tile_type: TileType(0), is_open: false },
                Mentsu { kind: MentsuKind::Shuntsu, tile_type: TileType(9), is_open: false },
                Mentsu { kind: MentsuKind::Shuntsu, tile_type: TileType(18), is_open: false },
            ],
        );
        let fu = calculate_fu(&hand, &[], &[], true, TileType::EAST, TileType::EAST);
        // 20(底) + 2(自摸) + 2(役牌雀头) = 24 → 30
        assert_eq!(fu, 30);
    }

    #[test]
    fn test_ron_menzen_no_yaku() {
        // 门清荣和，无特殊役
        let hand = make_standard_hand(
            TileType(31), // 白雀头 +2
            vec![
                Mentsu { kind: MentsuKind::Shuntsu, tile_type: TileType(0), is_open: false },
                Mentsu { kind: MentsuKind::Shuntsu, tile_type: TileType(9), is_open: false },
                Mentsu { kind: MentsuKind::Shuntsu, tile_type: TileType(18), is_open: false },
            ],
        );
        let fu = calculate_fu(&hand, &[], &[], false, TileType::EAST, TileType::EAST);
        // 20(底) + 10(门清荣和) + 2(役牌雀头) = 32 → 40
        assert_eq!(fu, 40);
    }
}
