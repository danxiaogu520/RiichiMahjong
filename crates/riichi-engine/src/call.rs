use riichi_core::game::{CallOption, CallType};
use riichi_core::player::Player;
use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};

/// 检测所有玩家对某张打出的牌可执行的副露操作
///
/// 返回所有可能的副露选项，按优先级排序：
/// - 荣和（最优先）
/// - 大明杠
/// - 碰
/// - 吃（仅下家可用，仅数牌）
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
            continue; // 不能对自己的牌副露
        }

        let hand = &player.hand;

        // 荣和粗筛：只对门清部分和和了牌判形，副露数量作为固定面子传入。
        // 暗杠包含 4 张实体牌，但规则上只占 1 组面子，不能把它按普通
        // 手牌牌张直接塞进 15 张牌里做 14 张和牌判断。
        let mut hand_tile_types: Vec<TileType> =
            hand.tiles().iter().map(|tile| tile.tile_type()).collect();
        hand_tile_types.push(discarded_tile.tile_type());
        if riichi_logic::win_check::is_win_shape_with_open_melds(
            &hand_tile_types,
            player.melds.len(),
        ) && !player.furiten.is_furiten()
        {
            options.push(CallOption {
                player: pid,
                call_type: CallType::Ron,
            });
        }

        // 立直后仍可荣和，但不能再进行吃、碰、大明杠。
        if !player.is_riichi {
            // 大明杠检测：手中有 3 张相同牌
            let count = hand.count_type(tt);
            if count >= 3 {
                let hand_tiles = find_tiles_of_type_3(hand, tt);
                options.push(CallOption {
                    player: pid,
                    call_type: CallType::Minkan { hand_tiles },
                });
            }

            // 碰检测：手中有 2 张相同牌
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
///
/// 吃的三种形式（假设打出的牌为 X）：
/// 1. X-2, X-1, X（左吃）
/// 2. X-1, X, X+1（中吃）
/// 3. X, X+1, X+2（右吃）
fn detect_chi(hand: &riichi_core::hand::Hand, discarded: TileType) -> Vec<[Tile; 2]> {
    let mut results = Vec::new();
    let rank = discarded.rank().0; // 1-9
    let base = TileType(discarded.0 - (rank - 1)); // 同花色 1 的 type

    // 1) X-2, X-1, X（左吃）
    if rank >= 3 {
        let t1 = TileType(base.0 + rank - 3);
        let t2 = TileType(base.0 + rank - 2);
        if let Some(tiles) = find_chi_pair(hand, t1, t2) {
            results.push(tiles);
        }
    }
    // 2) X-1, X, X+1（中吃）
    if (2..=8).contains(&rank) {
        let t1 = TileType(base.0 + rank - 2);
        let t2 = TileType(base.0 + rank);
        if let Some(tiles) = find_chi_pair(hand, t1, t2) {
            results.push(tiles);
        }
    }
    // 3) X, X+1, X+2（右吃）
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
fn find_chi_pair(
    hand: &riichi_core::hand::Hand,
    type1: TileType,
    type2: TileType,
) -> Option<[Tile; 2]> {
    let tile1 = hand.tiles().iter().find(|t| t.tile_type() == type1)?;
    let tile2 = hand
        .tiles()
        .iter()
        .find(|t| t.tile_type() == type2 && *t != tile1)?;
    Some([*tile1, *tile2])
}

/// 从手牌中找到指定 TileType 的 2 张牌（用于碰）
fn find_tiles_of_type(hand: &riichi_core::hand::Hand, tt: TileType, n: usize) -> [Tile; 2] {
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
fn find_tiles_of_type_3(hand: &riichi_core::hand::Hand, tt: TileType) -> [Tile; 3] {
    let tiles: Vec<Tile> = hand
        .tiles()
        .iter()
        .filter(|t| t.tile_type() == tt)
        .take(3)
        .copied()
        .collect();
    [tiles[0], tiles[1], tiles[2]]
}

#[cfg(test)]
mod tests {
    use super::detect_calls;
    use riichi_core::game::CallType;
    use riichi_core::meld::Meld;
    use riichi_core::player::{Player, PlayerId};
    use riichi_core::tile::Tile;

    #[test]
    fn winning_shape_does_not_hide_optional_pon() {
        let mut players = [
            Player::new(PlayerId(0), riichi_core::tile::TileType::EAST),
            Player::new(PlayerId(1), riichi_core::tile::TileType::SOUTH),
            Player::new(PlayerId(2), riichi_core::tile::TileType::WEST),
            Player::new(PlayerId(3), riichi_core::tile::TileType::NORTH),
        ];
        players[1].hand = riichi_core::hand::Hand::from_tiles(&[
            Tile::from_raw(32),
            Tile::from_raw(33),
            Tile::from_raw(36),
            Tile::from_raw(40),
            Tile::from_raw(44),
            Tile::from_raw(48),
            Tile::from_raw(52),
            Tile::from_raw(56),
            Tile::from_raw(60),
            Tile::from_raw(64),
            Tile::from_raw(68),
            Tile::from_raw(72),
            Tile::from_raw(73),
        ]);

        let options = detect_calls(&players, Tile::from_raw(34), PlayerId(0));
        assert!(options
            .iter()
            .any(|option| matches!(option.call_type, CallType::Ron)));
        assert!(options
            .iter()
            .any(|option| matches!(option.call_type, CallType::Pon { .. })));
    }

    #[test]
    fn open_kan_ron_shape_uses_fixed_meld_count() {
        let mut players = [
            Player::new(PlayerId(0), riichi_core::tile::TileType::EAST),
            Player::new(PlayerId(1), riichi_core::tile::TileType::SOUTH),
            Player::new(PlayerId(2), riichi_core::tile::TileType::WEST),
            Player::new(PlayerId(3), riichi_core::tile::TileType::NORTH),
        ];
        players[1].hand = riichi_core::hand::Hand::from_tiles(&[
            Tile::from_raw(0),
            Tile::from_raw(4),
            Tile::from_raw(8),
            Tile::from_raw(12),
            Tile::from_raw(16),
            Tile::from_raw(20),
            Tile::from_raw(24),
            Tile::from_raw(28),
            Tile::from_raw(32),
            Tile::from_raw(36),
        ]);
        let east = [
            Tile::from_raw(108),
            Tile::from_raw(109),
            Tile::from_raw(110),
            Tile::from_raw(111),
        ];
        players[1].melds.push(Meld::ankan(east.to_vec()));

        let options = detect_calls(&players, Tile::from_raw(37), PlayerId(0));
        assert!(options.iter().any(
            |option| option.player == PlayerId(1) && matches!(option.call_type, CallType::Ron)
        ));
    }
}
