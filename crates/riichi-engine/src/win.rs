use riichi_core::game::GameEvent;
use riichi_core::hand::Hand;
use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use riichi_logic::types::WinContext;
use riichi_logic::win_check;

use crate::game::GameState;

impl GameState {
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

        let riichi_index = self.events.iter().rposition(
            |e| matches!(e, GameEvent::PlayerDeclaredRiichi { player: pid } if *pid == player),
        );

        // 一发从立直宣言牌之后开始计算：立直宣言牌本身不打断一发，
        // 任何玩家的吃、碰、明杠、暗杠、加杠都会打断一发。
        let is_ippatsu = riichi_index.is_some_and(|index| {
            let after = &self.events[index + 1..];
            let own_discards = after
                .iter()
                .filter(|event| {
                    matches!(event, GameEvent::PlayerDiscarded { player: pid, .. } if *pid == player)
                })
                .count();
            own_discards == 1
                && !after.iter().any(|event| {
                    matches!(
                        event,
                        GameEvent::PlayerCalledPon { .. }
                            | GameEvent::PlayerCalledChi { .. }
                            | GameEvent::PlayerCalledMinkan { .. }
                            | GameEvent::PlayerCalledAnkan { .. }
                            | GameEvent::PlayerCalledKakan { .. }
                    )
                })
        });

        // 双立直必须发生在当前局第一巡：立直者在本局只打出宣言牌，
        // 且宣言前没有鸣牌、全桌弃牌数仍不超过一轮四张。
        let is_double_riichi = riichi_index.is_some_and(|index| {
            let before_or_at = &self.events[..=index];
            let discard_count = before_or_at
                .iter()
                .filter(|event| matches!(event, GameEvent::PlayerDiscarded { .. }))
                .count();
            let own_discard_count = before_or_at
                .iter()
                .filter(|event| {
                    matches!(event, GameEvent::PlayerDiscarded { player: pid, .. } if *pid == player)
                })
                .count();
            discard_count <= 4
                && own_discard_count == 1
                && !before_or_at.iter().any(|event| {
                    matches!(
                        event,
                        GameEvent::PlayerCalledPon { .. }
                            | GameEvent::PlayerCalledChi { .. }
                            | GameEvent::PlayerCalledMinkan { .. }
                            | GameEvent::PlayerCalledAnkan { .. }
                            | GameEvent::PlayerCalledKakan { .. }
                    )
                })
        });

        WinContext {
            is_tsumo,
            is_riichi: p.is_riichi,
            is_double_riichi,
            is_ippatsu,
            is_rinshan: false, // 由调用方设置
            is_chankan,
            is_haitei: no_tiles_left && is_tsumo,
            is_houtei: no_tiles_left && !is_tsumo,
            red_fives: self.rules.red_fives,
            kuitan: self.rules.kuitan,
            atozuke: self.rules.atozuke,
            seat_wind: p.wind,
            field_wind: self.wind,
            dora_indicators: self.dora_indicators.clone(),
            ura_dora_indicators: self.ura_dora_indicators.clone(),
            melds: p.melds.clone(),
            dealer: self.get_dealer().0,
            winner: player.0,
            loser: None,
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
        ctx.is_rinshan = self.is_rinshan_tile(winning_tile);

        // 检查和了
        let is_furiten = self.players[player.0].furiten.is_furiten();
        let result =
            win_check::check_win(&all_tiles, &hand_tile_types, &ctx, is_furiten, winning_tile)?;
        let yaku_names: Vec<String> = result
            .yaku_results
            .iter()
            .map(|y| format!("{:?}", y.yaku))
            .collect();
        Some((result.points, yaku_names))
    }
}
