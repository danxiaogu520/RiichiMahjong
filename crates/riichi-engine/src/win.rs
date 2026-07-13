use riichi_core::game::GameEvent;
use riichi_core::hand::Hand;
use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use riichi_logic::types::WinContext;
use riichi_logic::win_check;

use crate::game::GameState;

fn is_call_event(event: &GameEvent) -> bool {
    matches!(
        event,
        GameEvent::PlayerCalledPon { .. }
            | GameEvent::PlayerCalledChi { .. }
            | GameEvent::PlayerCalledMinkan { .. }
            | GameEvent::PlayerCalledAnkan { .. }
            | GameEvent::PlayerCalledKakan { .. }
    )
}

fn is_ippatsu_active(events: &[GameEvent], player: PlayerId) -> bool {
    let Some(index) = events.iter().rposition(
        |event| matches!(event, GameEvent::PlayerDeclaredRiichi { player: pid } if *pid == player),
    ) else {
        return false;
    };
    let declaration_discard_before = index > 0
        && matches!(
            &events[index - 1],
            GameEvent::PlayerDiscarded { player: pid, .. } if *pid == player
        );
    let after = &events[index + 1..];
    let own_discards_after = after
        .iter()
        .filter(|event| {
            matches!(event, GameEvent::PlayerDiscarded { player: pid, .. } if *pid == player)
        })
        .count();
    !after.iter().any(is_call_event)
        && if declaration_discard_before {
            own_discards_after == 0
        } else {
            own_discards_after == 1
        }
}

fn is_double_riichi_active(events: &[GameEvent], player: PlayerId) -> bool {
    let has_riichi = events.iter().any(
        |event| matches!(event, GameEvent::PlayerDeclaredRiichi { player: pid } if *pid == player),
    );
    let discard_count = events
        .iter()
        .filter(|event| matches!(event, GameEvent::PlayerDiscarded { .. }))
        .count();
    let own_discard_count = events
        .iter()
        .filter(|event| {
            matches!(event, GameEvent::PlayerDiscarded { player: pid, .. } if *pid == player)
        })
        .count();
    has_riichi && discard_count <= 4 && own_discard_count == 1 && !events.iter().any(is_call_event)
}

impl GameState {
    /// 判断指定等待牌是否至少存在一种有役和牌方式。
    pub fn wait_has_yaku(&self, player: PlayerId, tile_type: TileType) -> bool {
        let p = &self.players[player.0];
        let winning_tile = Tile::from_type_index(tile_type.0, 0);
        let mut all_tiles = p.hand.tiles().to_vec();
        for meld in &p.melds {
            all_tiles.extend_from_slice(&meld.tiles);
        }
        all_tiles.push(winning_tile);
        let mut hand_tile_types: Vec<TileType> =
            p.hand.tiles().iter().map(|t| t.tile_type()).collect();
        hand_tile_types.push(tile_type);

        for is_tsumo in [true, false] {
            let mut ctx = self.make_win_context(player, is_tsumo, winning_tile, false);
            ctx.loser = (!is_tsumo).then_some((player.0 + 1) % 4);
            if win_check::check_win(&all_tiles, &hand_tile_types, &ctx, false, winning_tile)
                .is_some()
            {
                return true;
            }
        }
        false
    }

    /// 检查自摸和（只读检查，不消耗自摸牌）
    ///
    /// 模拟 hand + drawn_tile 合并后的 14 张手牌进行判定
    /// 返回 None 表示不能和，Some((点数变化, 役名列表)) 表示可以和
    pub fn check_tsumo(&self, player: PlayerId) -> Option<([i32; 4], Vec<String>)> {
        let winning_tile = self.drawn_tile?;
        let hand = &self.players[player.0].hand;
        self.check_win_with_hand(player, true, winning_tile, None, hand, false)
    }

    /// 构建和了评估上下文
    ///
    /// 包含判断役、计算点数所需的所有信息：
    /// - 自摸/荣和
    /// - 立直/双立直/一发
    /// - 岭上/抢杠
    /// - 海底/河底
    /// - 自风/场风
    /// - 宝牌信息
    /// - 副露信息
    /// - 本场/立直棒
    fn make_win_context(
        &self,
        player: PlayerId,
        is_tsumo: bool,
        _winning_tile: Tile,
        is_chankan: bool,
    ) -> WinContext {
        let p = &self.players[player.0];
        let no_tiles_left = self.remaining_tiles() == 0;

        // 一发从立直宣言牌之后开始计算：立直宣言牌本身不打断一发，
        // 任何玩家的吃、碰、明杠、暗杠、加杠都会打断一发。
        let is_ippatsu = is_ippatsu_active(&self.events, player);

        // 双立直必须发生在当前局第一巡：立直者在本局只打出宣言牌，
        // 且宣言前没有鸣牌、全桌弃牌数仍不超过一轮四张。
        let is_double_riichi = is_double_riichi_active(&self.events, player);

        let has_call = self.events.iter().any(|event| {
            matches!(
                event,
                GameEvent::PlayerCalledPon { .. }
                    | GameEvent::PlayerCalledChi { .. }
                    | GameEvent::PlayerCalledMinkan { .. }
                    | GameEvent::PlayerCalledAnkan { .. }
                    | GameEvent::PlayerCalledKakan { .. }
            )
        });
        let has_any_discard = self
            .events
            .iter()
            .any(|event| matches!(event, GameEvent::PlayerDiscarded { .. }));
        let has_player_discard = self.events.iter().any(|event| {
            matches!(
                event,
                GameEvent::PlayerDiscarded { player: pid, .. } if *pid == player
            )
        });

        let is_rinshan = is_tsumo && self.is_rinshan_tile(_winning_tile);

        WinContext {
            is_tsumo,
            is_riichi: p.is_riichi,
            is_double_riichi,
            is_ippatsu,
            is_rinshan: false, // 由调用方设置
            is_chankan,
            is_haitei: no_tiles_left && is_tsumo && !is_rinshan,
            is_houtei: no_tiles_left && !is_tsumo,
            is_tenhou: is_tsumo && player == self.get_dealer() && !has_any_discard && !has_call,
            is_chiihou: is_tsumo && player != self.get_dealer() && !has_player_discard && !has_call,
            red_fives: crate::rules::RED_FIVES,
            kuitan: crate::rules::KUITAN,
            atozuke: true,
            allow_double_yakuman: crate::rules::ALLOW_DOUBLE_YAKUMAN,
            seat_wind: p.wind,
            field_wind: self.wind,
            dora_indicators: self.dora_indicators.clone(),
            ura_dora_indicators: self.ura_dora_indicators.clone(),
            melds: p.melds.clone(),
            dealer: self.get_dealer().0,
            winner: player.0,
            loser: None,
            pao_target: self.pao_targets[player.0],
            honba: self.honba,
            riichi_sticks: self.riichi_sticks,
        }
    }

    /// 检查和了（从玩家手牌读取）
    ///
    /// 返回 None 表示不能和，Some((点数变化, 役名列表)) 表示可以和
    pub(crate) fn check_win(
        &self,
        player: PlayerId,
        is_tsumo: bool,
        winning_tile: Tile,
        loser: Option<PlayerId>,
        is_chankan: bool,
    ) -> Option<([i32; 4], Vec<String>)> {
        let hand = &self.players[player.0].hand;
        self.check_win_with_hand(player, is_tsumo, winning_tile, loser, hand, is_chankan)
    }

    /// 检查和了（使用指定手牌，支持模拟 hand + drawn_tile）
    ///
    /// 支持三种和了形态：标准形、七对子、国士无双
    ///
    /// 返回 None 表示不能和，Some((点数变化, 役名列表)) 表示可以和
    /// 点数变化是 [i32; 4] 数组，表示每个玩家的点数增减
    fn check_win_with_hand(
        &self,
        player: PlayerId,
        is_tsumo: bool,
        winning_tile: Tile,
        loser: Option<PlayerId>,
        hand: &Hand,
        is_chankan: bool,
    ) -> Option<([i32; 4], Vec<String>)> {
        // 构建 all_tiles = 手牌 + 副露 + 和了牌（用于宝牌/赤宝牌计算）
        let mut all_tiles: Vec<Tile> = hand.tiles().to_vec();
        for meld in &self.players[player.0].melds {
            all_tiles.extend_from_slice(&meld.tiles);
        }
        all_tiles.push(winning_tile);

        // 门清部分 TileType（手牌 + 和了牌，用于判形和拆解）
        let mut hand_tile_types: Vec<TileType> =
            hand.tiles().iter().map(|t| t.tile_type()).collect();
        hand_tile_types.push(winning_tile.tile_type());

        // 构建上下文
        let mut ctx = self.make_win_context(player, is_tsumo, winning_tile, is_chankan);
        ctx.loser = loser.map(|id| id.0);
        ctx.is_rinshan = is_tsumo && self.is_rinshan_tile(winning_tile);

        // 检查和了
        let is_furiten = self.players[player.0].furiten.is_furiten();
        let result =
            win_check::check_win(&all_tiles, &hand_tile_types, &ctx, is_furiten, winning_tile)?;
        let mut yaku_names: Vec<String> = result
            .yaku_results
            .iter()
            .map(|y| format!("{:?}（{}翻）", y.yaku, y.han))
            .collect();
        yaku_names.push(format!("合计：{}翻 {}符", result.total_han, result.fu));
        Some((result.points, yaku_names))
    }
}

#[cfg(test)]
mod context_tests {
    use super::{is_double_riichi_active, is_ippatsu_active};
    use riichi_core::game::GameEvent;
    use riichi_core::player::PlayerId;
    use riichi_core::tile::Tile;

    fn discard(player: PlayerId) -> GameEvent {
        GameEvent::PlayerDiscarded {
            player,
            tile: Tile::from_raw(0),
        }
    }

    #[test]
    fn ippatsu_expires_on_the_next_own_discard() {
        let player = PlayerId(0);
        let riichi = GameEvent::PlayerDeclaredRiichi { player };
        assert!(is_ippatsu_active(
            &[discard(player), riichi.clone()],
            player
        ));
        assert!(!is_ippatsu_active(
            &[discard(player), riichi, discard(player)],
            player
        ));
    }

    #[test]
    fn ippatsu_is_cancelled_by_any_call() {
        let player = PlayerId(0);
        let riichi = GameEvent::PlayerDeclaredRiichi { player };
        let call = GameEvent::PlayerCalledPon {
            player: PlayerId(1),
            tiles: vec![Tile::from_raw(0); 3],
            from_player: player,
        };
        assert!(!is_ippatsu_active(&[discard(player), riichi, call], player));
    }

    #[test]
    fn double_riichi_requires_the_first_discard_cycle() {
        let player = PlayerId(0);
        let riichi = GameEvent::PlayerDeclaredRiichi { player };
        assert!(is_double_riichi_active(
            &[discard(player), riichi.clone()],
            player
        ));
        assert!(!is_double_riichi_active(
            &[
                discard(player),
                discard(PlayerId(1)),
                discard(player),
                riichi
            ],
            player
        ));
    }
}
