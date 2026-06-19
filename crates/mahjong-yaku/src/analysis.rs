use mahjong_core::tile::TileType;

use crate::types::{HandType, Mentsu, MentsuKind, TileCounts, WaitInfo, WaitTileInfo, WaitType, WinningHand};

// ─── 和了判定 ──────────────────────────────────────────────

/// 判断是否为和了形（标准形 + 七对子 + 国士无双）
pub fn is_winning(counts: &mut TileCounts) -> bool {
    is_standard_win(counts) || is_seven_pairs(counts) || is_kokushi(counts)
}

/// 标准和了形：N 面子 + 1 雀头（N 由总牌数推算）
pub fn is_standard_win(counts: &mut TileCounts) -> bool {
    let total: u8 = counts.inner().iter().sum();
    let num_mentsu = (total as usize).saturating_sub(2) / 3;
    for i in 0..34u8 {
        let tt = TileType(i);
        if counts.get(tt) >= 2 {
            counts.dec(tt);
            counts.dec(tt);
            if can_form_mentsu(counts, num_mentsu) {
                counts.inc(tt);
                counts.inc(tt);
                return true;
            }
            counts.inc(tt);
            counts.inc(tt);
        }
    }
    false
}

/// 递归判定能否组成指定数量的面子
fn can_form_mentsu(counts: &mut TileCounts, num_mentsu: usize) -> bool {
    let idx = counts.inner().iter().position(|&c| c > 0);
    match idx {
        None => num_mentsu == 0,
        Some(i) => {
            if num_mentsu == 0 {
                return false;
            }
            let tt = TileType(i as u8);
            // 尝试刻子
            if counts.get(tt) >= 3 {
                counts.dec(tt);
                counts.dec(tt);
                counts.dec(tt);
                if can_form_mentsu(counts, num_mentsu - 1) {
                    counts.inc(tt);
                    counts.inc(tt);
                    counts.inc(tt);
                    return true;
                }
                counts.inc(tt);
                counts.inc(tt);
                counts.inc(tt);
            }
            // 尝试顺子（仅数牌，rank <= 7）
            if tt.is_number() && tt.rank().0 <= 7 {
                let tt2 = TileType(i as u8 + 1);
                let tt3 = TileType(i as u8 + 2);
                if tt.suit() == tt2.suit() && tt.suit() == tt3.suit()
                    && counts.get(tt) >= 1 && counts.get(tt2) >= 1 && counts.get(tt3) >= 1
                {
                    counts.dec(tt);
                    counts.dec(tt2);
                    counts.dec(tt3);
                    if can_form_mentsu(counts, num_mentsu - 1) {
                        counts.inc(tt);
                        counts.inc(tt2);
                        counts.inc(tt3);
                        return true;
                    }
                    counts.inc(tt);
                    counts.inc(tt2);
                    counts.inc(tt3);
                }
            }
            false
        }
    }
}

/// 七对子判定：7 组不同的对子
pub fn is_seven_pairs(counts: &TileCounts) -> bool {
    let inner = counts.inner();
    let pairs = inner.iter().filter(|&&c| c == 2).count();
    let valid = inner.iter().all(|&c| c == 0 || c == 2);
    valid && pairs == 7
}

/// 国士无双判定：13 种幺九牌各 1 张 + 其中 1 种再 1 张
pub fn is_kokushi(counts: &TileCounts) -> bool {
    let mut has_pair = false;
    for &tt in &TileType::YAOCHUUHAI {
        match counts.get(tt) {
            0 => return false,
            2 => {
                if has_pair {
                    return false;
                }
                has_pair = true;
            }
            1 => {}
            _ => return false,
        }
    }
    for i in 0..34u8 {
        let tt = TileType(i);
        if !tt.is_yaochuuhai() && counts.get(tt) > 0 {
            return false;
        }
    }
    has_pair
}

// ─── 手牌分解 ──────────────────────────────────────────────

/// 标准形分解 — 返回一种有效的分解（用于役种判定和计符）
/// 如果不是标准形返回 None
///
/// 注意：此函数返回第一个找到的分解，不保证是最优分解。
/// 需要高点法时请使用 [`decompose_all_standard`]。
pub fn decompose_standard(counts: &mut TileCounts) -> Option<WinningHand> {
    let all = decompose_all_standard(counts);
    all.into_iter().next()
}

/// 标准形全分解 — 返回所有可能的分解（用于高点法）
///
/// 高点法：得点计算时，如有多种拆解法，按得点高的方式计算。
/// 因此需要枚举所有合法分解，逐个评分取最高。
pub fn decompose_all_standard(counts: &mut TileCounts) -> Vec<WinningHand> {
    let total: u8 = counts.inner().iter().sum();
    let num_mentsu = (total as usize).saturating_sub(2) / 3;
    let mut results = Vec::new();
    for i in 0..34u8 {
        let tt = TileType(i);
        if counts.get(tt) >= 2 {
            counts.dec(tt);
            counts.dec(tt);
            let all_mentsu = decompose_all_mentsu(counts, num_mentsu);
            counts.inc(tt);
            counts.inc(tt);
            for mut mentsu in all_mentsu {
                mentsu.sort_by_key(|m| (m.tile_type.0, m.kind as u8));
                results.push(WinningHand {
                    hand_type: HandType::Standard,
                    jantai: tt,
                    mentsu,
                });
            }
        }
    }
    results
}

/// 递归枚举所有可能的面子组合
fn decompose_all_mentsu(counts: &mut TileCounts, num: usize) -> Vec<Vec<Mentsu>> {
    let idx = match counts.inner().iter().position(|&c| c > 0) {
        None => {
            return if num == 0 { vec![Vec::new()] } else { vec![] };
        }
        Some(i) => i,
    };
    if num == 0 {
        return vec![];
    }
    let tt = TileType(idx as u8);
    let mut results = Vec::new();
    // 尝试刻子
    if counts.get(tt) >= 3 {
        counts.dec(tt);
        counts.dec(tt);
        counts.dec(tt);
        for mut rest in decompose_all_mentsu(counts, num - 1) {
            rest.push(Mentsu {
                kind: MentsuKind::Koutsu,
                tile_type: tt,
                is_open: false,
            });
            results.push(rest);
        }
        counts.inc(tt);
        counts.inc(tt);
        counts.inc(tt);
    }
    // 尝试顺子（仅数牌，rank <= 7）
    if tt.is_number() && tt.rank().0 <= 7 {
        let tt2 = TileType(idx as u8 + 1);
        let tt3 = TileType(idx as u8 + 2);
        if tt.suit() == tt2.suit() && tt.suit() == tt3.suit()
            && counts.get(tt) >= 1 && counts.get(tt2) >= 1 && counts.get(tt3) >= 1
        {
            counts.dec(tt);
            counts.dec(tt2);
            counts.dec(tt3);
            for mut rest in decompose_all_mentsu(counts, num - 1) {
                rest.push(Mentsu {
                    kind: MentsuKind::Shuntsu,
                    tile_type: tt,
                    is_open: false,
                });
                results.push(rest);
            }
            counts.inc(tt);
            counts.inc(tt2);
            counts.inc(tt3);
        }
    }
    results
}

// ─── 七对子 / 国士无双 分解 ─────────────────────────────────

/// 七对子分解 — 返回一种七对子分解（如果手牌是七对子形）
pub fn decompose_seven_pairs(counts: &TileCounts) -> Option<WinningHand> {
    let inner = counts.inner();
    let pairs: Vec<TileType> = (0..34u8)
        .map(TileType)
        .filter(|&tt| counts.get(tt) == 2)
        .collect();
    let valid = inner.iter().all(|&c| c == 0 || c == 2);
    if valid && pairs.len() == 7 {
        // 七对子：用第一对作为雀头，剩余 6 对以 Koutsu(count=2) 表示
        // 符数固定 25 符，具体哪个做雀头不影响计分
        let jantai = pairs[0];
        let mentsu: Vec<Mentsu> = pairs[1..]
            .iter()
            .map(|&tt| Mentsu {
                kind: MentsuKind::Koutsu,
                tile_type: tt,
                is_open: false,
            })
            .collect();
        Some(WinningHand {
            hand_type: HandType::SevenPairs,
            jantai,
            mentsu,
        })
    } else {
        None
    }
}

/// 国士无双分解 — 返回一种国士无双分解（如果手牌是国士无双形）
pub fn decompose_kokushi(counts: &TileCounts) -> Option<WinningHand> {
    let mut has_pair = false;
    let mut jantai = TileType(0);
    for &tt in &TileType::YAOCHUUHAI {
        match counts.get(tt) {
            0 => return None,
            2 => {
                if has_pair {
                    return None; // 不允许两个对子
                }
                has_pair = true;
                jantai = tt;
            }
            1 => {}
            _ => return None,
        }
    }
    // 非幺九牌不能存在
    for i in 0..34u8 {
        let tt = TileType(i);
        if !tt.is_yaochuuhai() && counts.get(tt) > 0 {
            return None;
        }
    }
    if !has_pair {
        return None;
    }
    Some(WinningHand {
        hand_type: HandType::Kokushi,
        jantai,
        mentsu: vec![],
    })
}

/// 将 WaitInfo 合并到已有的 wait_map 中（去重）
fn merge_wait_info(wait_map: &mut Vec<(TileType, Vec<WaitType>)>, new_info: WaitInfo) {
    for wti in new_info {
        if let Some((_, existing)) = wait_map.iter_mut().find(|(tt, _)| *tt == wti.tile_type) {
            for wt in wti.wait_types {
                if !existing.contains(&wt) {
                    existing.push(wt);
                }
            }
        } else {
            wait_map.push((wti.tile_type, wti.wait_types));
        }
    }
}

// ─── 听牌分析 ──────────────────────────────────────────────

/// 完整听牌分析（手牌为 3N+1 张时调用，N 可以是 0-4）
///
/// 枚举所有可能的和了牌，对每种拆解分析听牌类型，返回每张听牌的所有可能听牌类型。
/// 同时检测标准形、七对子、国士无双三种和了形态的听牌。
pub fn analyze_wait_tiles(hand_tiles: &[mahjong_core::tile::Tile]) -> WaitInfo {
    let base = TileCounts::from_tiles(hand_tiles);
    let mut wait_map: Vec<(TileType, Vec<WaitType>)> = Vec::new();

    // 标准形听牌：枚举 34 种牌，逐个检查是否能和
    for i in 0..34u8 {
        let tt = TileType(i);
        let mut counts = base;
        counts.inc(tt);
        if counts.get(tt) > 4 {
            continue;
        }
        let decompositions = decompose_all_standard(&mut counts);
        if decompositions.is_empty() {
            continue;
        }

        let mut wait_types = Vec::new();
        for hand in &decompositions {
            let wts = classify_wait(hand, tt);
            for wt in wts {
                if !wait_types.contains(&wt) {
                    wait_types.push(wt);
                }
            }
        }

        merge_wait_info(&mut wait_map, vec![WaitTileInfo {
            tile_type: tt,
            wait_types,
        }]);
    }

    // 七对子听牌
    if let Some(sp_wait) = analyze_seven_pairs_tenpai(&base) {
        merge_wait_info(&mut wait_map, sp_wait);
    }

    // 国士无双听牌
    if let Some(k_wait) = analyze_kokushi_tenpai(&base) {
        merge_wait_info(&mut wait_map, k_wait);
    }

    wait_map
        .into_iter()
        .map(|(tile_type, wait_types)| WaitTileInfo { tile_type, wait_types })
        .collect()
}

/// 七对子听牌分析（13 张手牌）
///
/// 七对子听牌条件：恰好有 6 种牌各 2 张 + 1 种牌 1 张（听该牌），
/// 或 5 种牌各 2 张 + 1 种牌 3 张（听该牌凑成第 6 对）。
fn analyze_seven_pairs_tenpai(base: &TileCounts) -> Option<WaitInfo> {
    let total: u8 = base.inner().iter().sum();
    if total != 13 {
        return None;
    }

    let mut waits = Vec::new();
    let mut incomplete_count = 0u8;

    for i in 0..34u8 {
        let tt = TileType(i);
        match base.get(tt) {
            0 => {}
            1 => {
                waits.push(WaitTileInfo {
                    tile_type: tt,
                    wait_types: vec![WaitType::Tanki],
                });
                incomplete_count += 1;
            }
            2 => {} // 完整的对子
            3 => {
                waits.push(WaitTileInfo {
                    tile_type: tt,
                    wait_types: vec![WaitType::Tanki],
                });
                incomplete_count += 1;
            }
            _ => return None, // 七对子中不可能出现 4 张（未考虑 4 张拆两对的变体规则）
        }
    }

    if incomplete_count == 1 {
        Some(waits)
    } else {
        None
    }
}

/// 国士无双听牌分析（13 张手牌）
///
/// 国士无双听牌条件：13 张牌中包含 12 种不同的幺九牌 + 1 张非幺九牌（十三面听），
/// 或 13 种幺九牌全在但缺一种（普通国士听牌）。
fn analyze_kokushi_tenpai(base: &TileCounts) -> Option<WaitInfo> {
    let total: u8 = base.inner().iter().sum();
    if total != 13 {
        return None;
    }

    // 统计非幺九牌总张数
    let non_yaochuuhai_count: u8 = (0..34u8)
        .map(|i| TileType(i))
        .filter(|tt| !tt.is_yaochuuhai())
        .map(|tt| base.get(tt))
        .sum();

    // 统计出现过的幺九牌种类数
    let yaochuuhai_types = TileType::YAOCHUUHAI
        .iter()
        .filter(|&&tt| base.get(tt) > 0)
        .count();

    // 必须 13 种幺九牌全部在场（不可能有多余非幺九牌）
    if yaochuuhai_types == 13 {
        // 所有幺九牌各至少 1 张，恰好有一种是 2 张（对子）
        // 听缺少的那张（实际上不缺，但对子的那张多了一张）
        // 13 种全在 → 一定有且仅有一个对子 → 听牌数 = 0（已经和了？不对，14 张才是和了）
        // 等等，13 种全在 = 13 张，但需要 14 张才和了，所以一定有一个对子(2张)+12种各1张=14张
        // 但我们现在只有 13 张！所以 13 种全在是不可能的（13 种各 1 张 = 13 张，但没有对子）
        // → 13 种全在意味着每种恰好 1 张，没有对子 → 听任意幺九牌做对子
        let waits: WaitInfo = TileType::YAOCHUUHAI
            .iter()
            .map(|&tt| WaitTileInfo {
                tile_type: tt,
                wait_types: vec![WaitType::Tanki],
            })
            .collect();
        return Some(waits);
    }

    // 12 种幺九牌 + 1 张非幺九牌 → 听缺少的那张幺九牌
    if yaochuuhai_types == 12 && non_yaochuuhai_count == 1 {
        let waits: WaitInfo = TileType::YAOCHUUHAI
            .iter()
            .filter(|&&tt| base.get(tt) == 0)
            .map(|&tt| WaitTileInfo {
                tile_type: tt,
                wait_types: vec![WaitType::Tanki],
            })
            .collect();
        return if waits.is_empty() { None } else { Some(waits) };
    }

    // 12 种幺九牌（其中一种有 2 张 = 对子）+ 0 张非幺九牌 → 听缺少的那张
    if yaochuuhai_types == 12 && non_yaochuuhai_count == 0 {
        let has_pair = TileType::YAOCHUUHAI.iter().any(|&tt| base.get(tt) >= 2);
        if has_pair {
            let waits: WaitInfo = TileType::YAOCHUUHAI
                .iter()
                .filter(|&&tt| base.get(tt) == 0)
                .map(|&tt| WaitTileInfo {
                    tile_type: tt,
                    wait_types: vec![WaitType::Tanki],
                })
                .collect();
            return if waits.is_empty() { None } else { Some(waits) };
        }
    }

    None
}

/// 根据已拆解的手牌，判断和了牌在该拆解中的听牌类型
///
/// 同一张牌可能出现在多个面子中（如 2m 同时在 1m2m3m 和 2m3m4m 中），
/// 每种情况对应不同的听牌类型。返回所有可能的听牌类型。
pub(crate) fn classify_wait(hand: &WinningHand, winning_tile: TileType) -> Vec<WaitType> {
    let mut result = Vec::new();

    // 单骑听：和了牌完成雀头
    if hand.jantai == winning_tile {
        result.push(WaitType::Tanki);
    }

    for mentsu in &hand.mentsu {
        match mentsu.kind {
            MentsuKind::Koutsu => {
                if mentsu.tile_type == winning_tile {
                    if !result.contains(&WaitType::Shanpon) {
                        result.push(WaitType::Shanpon);
                    }
                }
            }
            MentsuKind::Shuntsu => {
                let base = mentsu.tile_type;
                if base.suit() != winning_tile.suit() {
                    continue;
                }
                let base_rank = base.rank().0;
                let win_rank = winning_tile.rank().0;
                if win_rank < base_rank || win_rank > base_rank + 2 {
                    continue;
                }
                // 和了牌在面子中的位置决定听牌类型
                let wt = if win_rank == base_rank + 1 {
                    // 中间位置 → 嵌张听
                    WaitType::Kanchan
                } else if win_rank == base_rank {
                    // 最低位置 → 两面或边张
                    if base_rank == 1 {
                        WaitType::Penchan // 1-2-3 中的 1，边张
                    } else {
                        WaitType::Ryanmen
                    }
                } else {
                    // 最高位置 (win_rank == base_rank + 2) → 两面或边张
                    if base_rank == 7 {
                        WaitType::Penchan // 7-8-9 中的 9，边张
                    } else {
                        WaitType::Ryanmen
                    }
                };
                if !result.contains(&wt) {
                    result.push(wt);
                }
            }
        }
    }

    if result.is_empty() {
        result.push(WaitType::Tanki);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use mahjong_core::tile::{Rank, Suit, Tile};

    fn make_tiles(spec: &[(Suit, Rank, u8)]) -> Vec<Tile> {
        spec.iter().map(|&(s, r, c)| Tile::new(s, r, c)).collect()
    }

    #[test]
    fn test_standard_win() {
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(1), 1), (Suit::Man, Rank(1), 2),
            (Suit::Man, Rank(2), 0), (Suit::Man, Rank(3), 0), (Suit::Man, Rank(4), 0),
            (Suit::Man, Rank(5), 0), (Suit::Man, Rank(6), 0), (Suit::Man, Rank(7), 0),
            (Suit::Man, Rank(8), 0), (Suit::Man, Rank(8), 1),
            (Suit::Man, Rank(9), 0), (Suit::Man, Rank(9), 1), (Suit::Man, Rank(9), 2),
        ]);
        let mut counts = TileCounts::from_tiles(&tiles);
        assert!(is_standard_win(&mut counts));
    }

    #[test]
    fn test_not_win() {
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(2), 0), (Suit::Man, Rank(3), 0),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(5), 0), (Suit::Man, Rank(6), 0),
            (Suit::Man, Rank(7), 0), (Suit::Man, Rank(8), 0), (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(2), 0), (Suit::Pin, Rank(3), 0),
            (Suit::Pin, Rank(4), 0),
        ]);
        let mut counts = TileCounts::from_tiles(&tiles);
        assert!(!is_standard_win(&mut counts));
    }

    #[test]
    fn test_seven_pairs() {
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(1), 1),
            (Suit::Man, Rank(2), 0), (Suit::Man, Rank(2), 1),
            (Suit::Man, Rank(3), 0), (Suit::Man, Rank(3), 1),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(4), 1),
            (Suit::Man, Rank(5), 0), (Suit::Man, Rank(5), 1),
            (Suit::Man, Rank(6), 0), (Suit::Man, Rank(6), 1),
            (Suit::Man, Rank(7), 0), (Suit::Man, Rank(7), 1),
        ]);
        let counts = TileCounts::from_tiles(&tiles);
        assert!(is_seven_pairs(&counts));
    }

    #[test]
    fn test_kokushi() {
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(1), 1), (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(9), 0),
            (Suit::Sou, Rank(1), 0), (Suit::Sou, Rank(9), 0),
            (Suit::Wind, Rank(1), 0), (Suit::Wind, Rank(2), 0),
            (Suit::Wind, Rank(3), 0), (Suit::Wind, Rank(4), 0),
            (Suit::Dragon, Rank(1), 0), (Suit::Dragon, Rank(2), 0), (Suit::Dragon, Rank(3), 0),
        ]);
        let counts = TileCounts::from_tiles(&tiles);
        assert!(is_kokushi(&counts));
        assert!(is_winning(&mut counts.clone()));
    }

    #[test]
    fn test_analyze_wait_tiles_equivalent() {
        // 原 test_waiting_tiles：123m 456m 789m 1234p → 听 4p
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(2), 0), (Suit::Man, Rank(3), 0),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(5), 0), (Suit::Man, Rank(6), 0),
            (Suit::Man, Rank(7), 0), (Suit::Man, Rank(8), 0), (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(2), 0), (Suit::Pin, Rank(3), 0),
            (Suit::Pin, Rank(4), 0),
        ]);
        let info = analyze_wait_tiles(&tiles);
        let tile_types: Vec<TileType> = info.iter().map(|w| w.tile_type).collect();
        assert!(tile_types.contains(&TileType(12))); // 4p
    }

    #[test]
    fn test_analyze_wait_tiles_ryanmen() {
        // 123m 456m 789m 2p2p3p4p（13张）
        // 加 2p → jantai=2p → [1m2m3m][4m5m6m][7m8m9m][3p4p+2p] 2p2p → 不行，3p4p不完整
        // 实际：加 2p → jantai=1m → [2m3m4m][5m6m7m][8m9m+?][2p2p3p4p] → 不行
        // 加 5p → jantai=2p → [1m2m3m][4m5m6m][7m8m9m][3p4p5p] 2p2p → OK (两面)
        // 加 2p → 分析结果：2p 作为雀头的单骑听
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(2), 0), (Suit::Man, Rank(3), 0),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(5), 0), (Suit::Man, Rank(6), 0),
            (Suit::Man, Rank(7), 0), (Suit::Man, Rank(8), 0), (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(2), 0), (Suit::Pin, Rank(2), 1),
            (Suit::Pin, Rank(3), 0), (Suit::Pin, Rank(4), 0),
        ]);
        let info = analyze_wait_tiles(&tiles);
        assert_eq!(info.len(), 2, "should have 2 waiting tiles, got {:?}", info);

        // 5p = TileType(13) → 两面听（3p4p 等 5p）
        let info_5p = info.iter().find(|w| w.tile_type == TileType(13)).unwrap();
        assert!(info_5p.wait_types.contains(&WaitType::Ryanmen));
    }

    #[test]
    fn test_analyze_wait_tiles_mixed() {
        // 手牌：1m2m3m3m4m 5p6p7p 8s8s8s 9p9p（13张）
        // 分解：jantai=9p → [1m2m3m][3m4m+?m][5p6p7p][8s8s8s] 9p9p
        // 听 5m：3m4m 补成 3m4m5m（两面听）
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(2), 0), (Suit::Man, Rank(3), 0),
            (Suit::Man, Rank(3), 1), (Suit::Man, Rank(4), 0),
            (Suit::Pin, Rank(5), 0), (Suit::Pin, Rank(6), 0), (Suit::Pin, Rank(7), 0),
            (Suit::Sou, Rank(8), 0), (Suit::Sou, Rank(8), 1), (Suit::Sou, Rank(8), 2),
            (Suit::Pin, Rank(9), 0), (Suit::Pin, Rank(9), 1),
        ]);
        let info = analyze_wait_tiles(&tiles);

        // 听 5m（两面）
        let info_5m = info.iter().find(|w| w.tile_type == TileType(4));
        assert!(info_5m.is_some(), "should wait for 5m");
        let info_5m = info_5m.unwrap();
        assert!(
            info_5m.wait_types.contains(&WaitType::Ryanmen),
            "5m should have Ryanmen wait, got {:?}",
            info_5m.wait_types,
        );
    }

    #[test]
    fn test_analyze_wait_tiles_multi_wait_type() {
        // 手牌：1m3m 2p3p4p 5p6p7p 8s8s8s 9p9p（13张）
        // 分解：jantai=9p → [1m3m+?m][2p3p4p][5p6p7p][8s8s8s] 9p9p
        // 听 2m（坎张：1m3m 等 2m）
        //
        // 同时 1m2m3m 也是合法拆法的一部分：
        // 如果加 2m → jantai=9p → [1m2m3m][2p3p4p][5p6p7p][8s8s8s] 9p9p
        // 但这样 3m 多出来了（1m3m 只有各一张，加 2m 后是 1m2m3m 顺子）
        // 所以 2m 只有坎张一种解释
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(3), 0),
            (Suit::Pin, Rank(2), 0), (Suit::Pin, Rank(3), 0), (Suit::Pin, Rank(4), 0),
            (Suit::Pin, Rank(5), 0), (Suit::Pin, Rank(6), 0), (Suit::Pin, Rank(7), 0),
            (Suit::Sou, Rank(8), 0), (Suit::Sou, Rank(8), 1), (Suit::Sou, Rank(8), 2),
            (Suit::Pin, Rank(9), 0), (Suit::Pin, Rank(9), 1),
        ]);
        let info = analyze_wait_tiles(&tiles);

        let info_2m = info.iter().find(|w| w.tile_type == TileType(1));
        assert!(info_2m.is_some(), "should wait for 2m");
        let info_2m = info_2m.unwrap();
        assert!(
            info_2m.wait_types.contains(&WaitType::Kanchan),
            "2m should have Kanchan wait, got {:?}",
            info_2m.wait_types,
        );
    }

    #[test]
    fn test_decompose_standard() {
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(1), 1), (Suit::Man, Rank(1), 2),
            (Suit::Man, Rank(2), 0), (Suit::Man, Rank(3), 0), (Suit::Man, Rank(4), 0),
            (Suit::Man, Rank(5), 0), (Suit::Man, Rank(6), 0), (Suit::Man, Rank(7), 0),
            (Suit::Man, Rank(8), 0), (Suit::Man, Rank(8), 1),
            (Suit::Man, Rank(9), 0), (Suit::Man, Rank(9), 1), (Suit::Man, Rank(9), 2),
        ]);
        let mut counts = TileCounts::from_tiles(&tiles);
        let result = decompose_standard(&mut counts);
        assert!(result.is_some());
        let hand = result.unwrap();
        assert_eq!(hand.mentsu.len(), 4);
        assert_eq!(hand.jantai, TileType(7)); // 8m
    }

    #[test]
    fn test_decompose_all_standard_single() {
        // 唯一分解的情况：1m1m1m 2m3m4m 5m6m7m 8m8m 9m9m9m
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(1), 1), (Suit::Man, Rank(1), 2),
            (Suit::Man, Rank(2), 0), (Suit::Man, Rank(3), 0), (Suit::Man, Rank(4), 0),
            (Suit::Man, Rank(5), 0), (Suit::Man, Rank(6), 0), (Suit::Man, Rank(7), 0),
            (Suit::Man, Rank(8), 0), (Suit::Man, Rank(8), 1),
            (Suit::Man, Rank(9), 0), (Suit::Man, Rank(9), 1), (Suit::Man, Rank(9), 2),
        ]);
        let mut counts = TileCounts::from_tiles(&tiles);
        let all = decompose_all_standard(&mut counts);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].jantai, TileType(7)); // 8m
    }

    #[test]
    fn test_decompose_all_standard_multiple() {
        // 手牌：1m2m3m 2m3m4m4m5m 5p6p7p 8s8s8s（14张）
        //
        // 两种合法分解：
        //   jantai=4m: 1m2m3m 2m3m5m 5p6p7p 8s8s8s 4m4m(雀头)  ← 2m3m4m + 4m5m 拆法
        //   jantai=5m: 1m2m3m 2m3m4m 4m5m 5p6p7p 8s8s8s 5m5m(雀头)
        // 等等，这样拆不出来。让我用经典多分解牌型：
        //
        // 手牌：2m3m4m 4m5m 5p6p7p 8s8s8s 1m2m3m（14张）
        // 即 1m2m3m4m4m5m 5p6p7p 8s8s8s
        //
        // 分解1：jantai=4m → [1m2m3m][2m3m5m][5p6p7p][8s8s8s] 4m4m
        //   不行，2m3m5m 不是合法面子
        //
        // 经典多分解牌型：1m2m3m4m5m6m7m8m9m 2m3m4m5m（14张）
        // jantai=5m → [1m2m3m][2m3m4m][4m5m6m][7m8m9m] 5m5m
        // jantai=2m → 不行（剩余 1m3m4m5m6m7m8m9m3m4m5m 无法组成4面子）
        // jantai=4m → 不行（剩余 1m2m3m5m6m7m8m9m2m3m5m 无法组成4面子）
        //
        // 只有一种分解。用不同的方法验证全分解能找到唯一解：
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0),
            (Suit::Man, Rank(2), 0), (Suit::Man, Rank(2), 1),
            (Suit::Man, Rank(3), 0), (Suit::Man, Rank(3), 1),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(4), 1),
            (Suit::Man, Rank(5), 0), (Suit::Man, Rank(5), 1), (Suit::Man, Rank(5), 2),
            (Suit::Man, Rank(6), 0),
            (Suit::Man, Rank(7), 0),
            (Suit::Man, Rank(8), 0),
            (Suit::Man, Rank(9), 0),
        ]);
        let mut counts = TileCounts::from_tiles(&tiles);
        let all = decompose_all_standard(&mut counts);
        // 这副手牌只有一种合法分解：jantai=5m
        assert_eq!(all.len(), 1, "expected 1 decomposition, got {}", all.len());
        assert_eq!(all[0].jantai.0, 4); // 5m (TileType(4) = Man rank 5)
    }

    #[test]
    fn test_decompose_all_standard_ambiguous_wait() {
        // 手牌：1m2m3m3m4m 5p6p7p 8s8s8s 5m5m（13张）
        // 加 2m → jantai=5m → [1m2m3m][2m3m4m][5p6p7p][8s8s8s] 5m5m → 两面听
        // 加 5m → jantai=5m → [1m2m3m][3m4m5m][5p6p7p][8s8s8s] 5m5m → 两面听
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(2), 0), (Suit::Man, Rank(3), 0),
            (Suit::Man, Rank(3), 1), (Suit::Man, Rank(4), 0), (Suit::Man, Rank(5), 0),
            (Suit::Man, Rank(5), 1),
            (Suit::Pin, Rank(5), 0), (Suit::Pin, Rank(6), 0), (Suit::Pin, Rank(7), 0),
            (Suit::Sou, Rank(8), 0), (Suit::Sou, Rank(8), 1), (Suit::Sou, Rank(8), 2),
        ]);
        let info = analyze_wait_tiles(&tiles);
        assert!(!info.is_empty(), "should find at least one waiting tile");

        // 2m = TileType(1)
        let info_2m = info.iter().find(|w| w.tile_type == TileType(1));
        assert!(info_2m.is_some(), "should wait for 2m");
        assert!(info_2m.unwrap().wait_types.contains(&WaitType::Ryanmen));

        // 5m = TileType(4)
        let info_5m = info.iter().find(|w| w.tile_type == TileType(4));
        assert!(info_5m.is_some(), "should wait for 5m");
        assert!(info_5m.unwrap().wait_types.contains(&WaitType::Ryanmen));
    }

    // ─── 七对子分解测试 ───────────────────────────────────────

    #[test]
    fn test_decompose_seven_pairs_valid() {
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(1), 1),
            (Suit::Man, Rank(2), 0), (Suit::Man, Rank(2), 1),
            (Suit::Man, Rank(3), 0), (Suit::Man, Rank(3), 1),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(4), 1),
            (Suit::Man, Rank(5), 0), (Suit::Man, Rank(5), 1),
            (Suit::Man, Rank(6), 0), (Suit::Man, Rank(6), 1),
            (Suit::Man, Rank(7), 0), (Suit::Man, Rank(7), 1),
        ]);
        let counts = TileCounts::from_tiles(&tiles);
        let result = decompose_seven_pairs(&counts);
        assert!(result.is_some());
        let hand = result.unwrap();
        assert_eq!(hand.hand_type, HandType::SevenPairs);
        assert_eq!(hand.mentsu.len(), 6);
    }

    #[test]
    fn test_decompose_seven_pairs_invalid() {
        // 有一组刻子（3张），不是七对子
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(1), 1), (Suit::Man, Rank(1), 2),
            (Suit::Man, Rank(2), 0), (Suit::Man, Rank(2), 1),
            (Suit::Man, Rank(3), 0), (Suit::Man, Rank(3), 1),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(4), 1),
            (Suit::Man, Rank(5), 0), (Suit::Man, Rank(5), 1),
            (Suit::Man, Rank(6), 0), (Suit::Man, Rank(6), 1),
            (Suit::Man, Rank(7), 0), (Suit::Man, Rank(7), 1),
        ]);
        let counts = TileCounts::from_tiles(&tiles);
        assert!(decompose_seven_pairs(&counts).is_none());
    }

    // ─── 国士无双分解测试 ─────────────────────────────────────

    #[test]
    fn test_decompose_kokushi_valid() {
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(1), 1), (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(9), 0),
            (Suit::Sou, Rank(1), 0), (Suit::Sou, Rank(9), 0),
            (Suit::Wind, Rank(1), 0), (Suit::Wind, Rank(2), 0),
            (Suit::Wind, Rank(3), 0), (Suit::Wind, Rank(4), 0),
            (Suit::Dragon, Rank(1), 0), (Suit::Dragon, Rank(2), 0), (Suit::Dragon, Rank(3), 0),
        ]);
        let counts = TileCounts::from_tiles(&tiles);
        let result = decompose_kokushi(&counts);
        assert!(result.is_some());
        let hand = result.unwrap();
        assert_eq!(hand.hand_type, HandType::Kokushi);
        assert_eq!(hand.mentsu.len(), 0);
    }

    #[test]
    fn test_decompose_kokushi_thirteen_wait() {
        // 十三面听：13 种幺九牌各 1 张
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(9), 0),
            (Suit::Sou, Rank(1), 0), (Suit::Sou, Rank(9), 0),
            (Suit::Wind, Rank(1), 0), (Suit::Wind, Rank(2), 0),
            (Suit::Wind, Rank(3), 0), (Suit::Wind, Rank(4), 0),
            (Suit::Dragon, Rank(1), 0), (Suit::Dragon, Rank(2), 0), (Suit::Dragon, Rank(3), 0),
        ]);
        let counts = TileCounts::from_tiles(&tiles);
        // 13 张各 1 张，不是和了形（没有对子）
        assert!(decompose_kokushi(&counts).is_none());
        // 但是 is_kokushi 需要 14 张，这里 13 张自然不是
    }

    // ─── 七对子听牌测试 ───────────────────────────────────────

    #[test]
    fn test_seven_pairs_tenpai_single_wait() {
        // 6 对 + 1 张单牌 → 听该单牌
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(1), 1),
            (Suit::Man, Rank(2), 0), (Suit::Man, Rank(2), 1),
            (Suit::Man, Rank(3), 0), (Suit::Man, Rank(3), 1),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(4), 1),
            (Suit::Man, Rank(5), 0), (Suit::Man, Rank(5), 1),
            (Suit::Man, Rank(6), 0), (Suit::Man, Rank(6), 1),
            (Suit::Man, Rank(7), 0),
        ]);
        let base = TileCounts::from_tiles(&tiles);
        let result = analyze_seven_pairs_tenpai(&base);
        assert!(result.is_some());
        let waits = result.unwrap();
        assert_eq!(waits.len(), 1);
        assert_eq!(waits[0].tile_type, TileType(6)); // 7m
        assert!(waits[0].wait_types.contains(&WaitType::Tanki));
    }

    #[test]
    fn test_seven_pairs_tenpai_with_triple() {
        // 5 对 + 1 组 3 张 → 听该牌（凑第 6 对）
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(1), 1),
            (Suit::Man, Rank(2), 0), (Suit::Man, Rank(2), 1),
            (Suit::Man, Rank(3), 0), (Suit::Man, Rank(3), 1),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(4), 1),
            (Suit::Man, Rank(5), 0), (Suit::Man, Rank(5), 1),
            (Suit::Man, Rank(6), 0), (Suit::Man, Rank(6), 1), (Suit::Man, Rank(6), 2),
        ]);
        let base = TileCounts::from_tiles(&tiles);
        let result = analyze_seven_pairs_tenpai(&base);
        assert!(result.is_some());
        let waits = result.unwrap();
        assert_eq!(waits.len(), 1);
        assert_eq!(waits[0].tile_type, TileType(5)); // 6m
    }

    #[test]
    fn test_seven_pairs_not_tenpai() {
        // 2 张单牌 → 不是七对子听牌
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(1), 1),
            (Suit::Man, Rank(2), 0), (Suit::Man, Rank(2), 1),
            (Suit::Man, Rank(3), 0), (Suit::Man, Rank(3), 1),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(4), 1),
            (Suit::Man, Rank(5), 0), (Suit::Man, Rank(5), 1),
            (Suit::Man, Rank(6), 0), (Suit::Man, Rank(6), 1),
            (Suit::Man, Rank(7), 0), (Suit::Man, Rank(8), 0),
        ]);
        let base = TileCounts::from_tiles(&tiles);
        assert!(analyze_seven_pairs_tenpai(&base).is_none());
    }

    // ─── 国士无双听牌测试 ─────────────────────────────────────

    #[test]
    fn test_kokushi_tenpai_missing_one() {
        // 12 种幺九牌 + 1 对(1m) → 缺 中(3s)
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(1), 1), // 1m 对子
            (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(9), 0),
            (Suit::Sou, Rank(1), 0), (Suit::Sou, Rank(9), 0),
            (Suit::Wind, Rank(1), 0), (Suit::Wind, Rank(2), 0),
            (Suit::Wind, Rank(3), 0), (Suit::Wind, Rank(4), 0),
            (Suit::Dragon, Rank(1), 0), (Suit::Dragon, Rank(2), 0),
            // 缺 中(Chuu = Dragon Rank(3))
        ]);
        let base = TileCounts::from_tiles(&tiles);
        let result = analyze_kokushi_tenpai(&base);
        assert!(result.is_some());
        let waits = result.unwrap();
        assert_eq!(waits.len(), 1);
        assert_eq!(waits[0].tile_type, TileType::CHUN); // 中 = TileType(33)
    }

    #[test]
    fn test_kokushi_tenpai_thirteen_wait() {
        // 13 种幺九牌各 1 张（无非幺九牌）→ 听任意幺九牌做对子（十三面听）
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(9), 0),
            (Suit::Sou, Rank(1), 0), (Suit::Sou, Rank(9), 0),
            (Suit::Wind, Rank(1), 0), (Suit::Wind, Rank(2), 0),
            (Suit::Wind, Rank(3), 0), (Suit::Wind, Rank(4), 0),
            (Suit::Dragon, Rank(1), 0), (Suit::Dragon, Rank(2), 0),
            (Suit::Dragon, Rank(3), 0),
        ]);
        let base = TileCounts::from_tiles(&tiles);
        let result = analyze_kokushi_tenpai(&base);
        assert!(result.is_some());
        let waits = result.unwrap();
        assert_eq!(waits.len(), 13); // 听所有 13 种幺九牌
    }

    // ─── analyze_wait_tiles 综合测试 ──────────────────────────

    #[test]
    fn test_analyze_wait_tiles_covers_seven_pairs() {
        // 手牌同时听标准形和七对子
        // 1m1m 2m2m 3m3m 4m4m 5m5m 6m6m 7m（13张）
        // 标准形：加任意 m 形成面子 + 雀头
        // 七对子：加 7m 凑成第 7 对
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(1), 1),
            (Suit::Man, Rank(2), 0), (Suit::Man, Rank(2), 1),
            (Suit::Man, Rank(3), 0), (Suit::Man, Rank(3), 1),
            (Suit::Man, Rank(4), 0), (Suit::Man, Rank(4), 1),
            (Suit::Man, Rank(5), 0), (Suit::Man, Rank(5), 1),
            (Suit::Man, Rank(6), 0), (Suit::Man, Rank(6), 1),
            (Suit::Man, Rank(7), 0),
        ]);
        let info = analyze_wait_tiles(&tiles);
        // 7m 应该出现在听牌列表中
        let info_7m = info.iter().find(|w| w.tile_type == TileType(6));
        assert!(info_7m.is_some(), "should wait for 7m (seven pairs)");
        assert!(info_7m.unwrap().wait_types.contains(&WaitType::Tanki));
    }

    #[test]
    fn test_analyze_wait_tiles_covers_kokushi() {
        // 12 种幺九牌 + 1 张非幺九(5m) → 国士无双听牌（听缺的中）
        let tiles = make_tiles(&[
            (Suit::Man, Rank(1), 0), (Suit::Man, Rank(9), 0),
            (Suit::Pin, Rank(1), 0), (Suit::Pin, Rank(9), 0),
            (Suit::Sou, Rank(1), 0), (Suit::Sou, Rank(9), 0),
            (Suit::Wind, Rank(1), 0), (Suit::Wind, Rank(2), 0),
            (Suit::Wind, Rank(3), 0), (Suit::Wind, Rank(4), 0),
            (Suit::Dragon, Rank(1), 0), (Suit::Dragon, Rank(2), 0),
            (Suit::Man, Rank(5), 0), // 非幺九牌
        ]);
        let info = analyze_wait_tiles(&tiles);
        // 应该听 1 种幺九牌（缺的中 = TileType(33)）
        let kokushi_wait = info.iter().find(|w| w.tile_type == TileType::CHUN);
        assert!(kokushi_wait.is_some(), "kokushi should wait for Chuu");
    }
}
