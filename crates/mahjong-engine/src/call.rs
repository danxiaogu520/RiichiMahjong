use mahjong_core::player::PlayerId;
use mahjong_core::tile::{Tile, TileType};

use crate::action::{CallOption, CallType};
use crate::player::Player;

/// 检测所有玩家对某张打出的牌可执行的副露操作
pub fn detect_calls(
    players: &[Player; 4],
    discarded_tile: Tile,
    discarder: PlayerId,
) -> Vec<CallOption> {
    let mut options = Vec::new();
    let tt = discarded_tile.tile_type();

    for (idx, player) in players.iter().enumerate().take(4) {
        let pid = PlayerId(idx);
        if pid == discarder {
            continue;
        }

        let hand = &player.hand;

        // 荣和检测
        let mut test_tiles: Vec<Tile> = hand.tiles().to_vec();
        test_tiles.push(discarded_tile);
        let mut counts = mahjong_yaku::types::TileCounts::from_tiles(&test_tiles);
        if mahjong_yaku::analysis::is_winning(&mut counts) {
            if !player.furiten.is_furiten() {
                options.push(CallOption {
                    player: pid,
                    call_type: CallType::Ron,
                });
            }
            continue;
        }

        // 大明杠检测：手中有 3 张相同牌
        let count = hand.count_type(tt.0);
        if count >= 3 {
            let hand_tiles = find_tiles_of_type_3(hand, tt);
            options.push(CallOption {
                player: pid,
                call_type: CallType::Minkan { hand_tiles },
            });
        }

        // 碰检测
        if count >= 2 {
            let hand_tiles = find_tiles_of_type(hand, tt, 2);
            options.push(CallOption {
                player: pid,
                call_type: CallType::Pon { hand_tiles },
            });
        }

        // 吃检测：仅下家可用，且仅数牌
        let next_player = discarder.next();
        if pid == next_player && tt.is_number() {
            let chi_options = detect_chi(hand, tt);
            for hand_tiles in chi_options {
                options.push(CallOption {
                    player: pid,
                    call_type: CallType::Chi { hand_tiles },
                });
            }
        }
    }

    // 按优先级排序：荣和 > 大明杠 > 碰 > 吃
    options.sort_by_key(|o| match o.call_type {
        CallType::Ron => 0,
        CallType::Minkan { .. } => 1,
        CallType::Pon { .. } => 2,
        CallType::Chi { .. } => 3,
    });

    options
}

/// 检测吃的所有可能组合
fn detect_chi(hand: &mahjong_core::hand::Hand, discarded: TileType) -> Vec<[Tile; 2]> {
    let mut results = Vec::new();
    let rank = discarded.rank().0; // 1-9
    let base = TileType(discarded.0 - (rank - 1)); // 同花色 1 的 type

    // 1) discarded-2, discarded-1, discarded
    if rank >= 3 {
        let t1 = TileType(base.0 + rank - 3);
        let t2 = TileType(base.0 + rank - 2);
        if let Some(tiles) = find_chi_pair(hand, t1, t2) {
            results.push(tiles);
        }
    }
    // 2) discarded-1, discarded, discarded+1
    if (2..=8).contains(&rank) {
        let t1 = TileType(base.0 + rank - 2);
        let t2 = TileType(base.0 + rank);
        if let Some(tiles) = find_chi_pair(hand, t1, t2) {
            results.push(tiles);
        }
    }
    // 3) discarded, discarded+1, discarded+2
    if rank <= 7 {
        let t1 = TileType(base.0 + rank);
        let t2 = TileType(base.0 + rank + 1);
        if let Some(tiles) = find_chi_pair(hand, t1, t2) {
            results.push(tiles);
        }
    }

    results
}

/// 找出手牌中能组成吃的两张牌
fn find_chi_pair(hand: &mahjong_core::hand::Hand, type1: TileType, type2: TileType) -> Option<[Tile; 2]> {
    let tile1 = hand.tiles().iter().find(|t| t.tile_type() == type1)?;
    let tile2 = hand
        .tiles()
        .iter()
        .find(|t| t.tile_type() == type2 && *t != tile1)?;
    Some([*tile1, *tile2])
}

/// 从手牌中找到指定 TileType 的 n 张牌
fn find_tiles_of_type(hand: &mahjong_core::hand::Hand, tt: TileType, n: usize) -> [Tile; 2] {
    let tiles: Vec<Tile> = hand
        .tiles()
        .iter()
        .filter(|t| t.tile_type() == tt)
        .take(n)
        .copied()
        .collect();
    [tiles[0], tiles[1]]
}

/// 从手牌中找到指定 TileType 的 3 张牌（用于大明杠）
fn find_tiles_of_type_3(hand: &mahjong_core::hand::Hand, tt: TileType) -> [Tile; 3] {
    let tiles: Vec<Tile> = hand
        .tiles()
        .iter()
        .filter(|t| t.tile_type() == tt)
        .take(3)
        .copied()
        .collect();
    [tiles[0], tiles[1], tiles[2]]
}
