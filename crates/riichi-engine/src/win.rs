use riichi_core::game::{CallKind, GameEvent, GamePhase};
use riichi_core::hand::Hand;
use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use riichi_logic::evaluation;
use riichi_logic::model::{SettlementContext, WinInput, WinSituation};

use crate::game::GameState;

fn is_call_event(event: &GameEvent) -> bool {
    matches!(event, GameEvent::Call { .. })
}

/// 判断一发是否仍然有效。
///
/// `chankan_on_kakan` 为真表示本次和牌是抢加杠：加杠事件本身不打断一发，
/// 一发在抢杠窗口内保持有效（窗口关闭后未抢杠才失效）。
fn is_ippatsu_active(events: &[GameEvent], player: PlayerId, chankan_on_kakan: bool) -> bool {
    let Some(index) = events
        .iter()
        .rposition(|event| matches!(event, GameEvent::Riichi { player: pid } if *pid == player))
    else {
        return false;
    };
    let declaration_discard_before = index > 0
        && matches!(
            &events[index - 1],
            GameEvent::Discard { player: pid, .. } if *pid == player
        );
    let mut after = &events[index + 1..];
    if chankan_on_kakan
        && matches!(
            after.last(),
            Some(GameEvent::Call {
                kind: CallKind::Kakan,
                ..
            })
        )
    {
        after = &after[..after.len() - 1];
    }
    let own_discards_after = after
        .iter()
        .filter(|event| matches!(event, GameEvent::Discard { player: pid, .. } if *pid == player))
        .count();
    !after.iter().any(is_call_event)
        && if declaration_discard_before {
            own_discards_after == 0
        } else {
            own_discards_after == 1
        }
}

/// 判断宣告时是否满足双立直条件。
///
/// 与 Mortal 的 `can_w_riichi` 对应：无任何鸣牌（吃/碰/明杠/暗杠），
/// 且这是本人的第一打。判定发生在立直宣告时，此后宣言牌被鸣走或
/// 发生其他事件都不影响双立直是否成立。
pub(crate) fn can_declare_double_riichi(events: &[GameEvent], player: PlayerId) -> bool {
    !events.iter().any(is_call_event)
        && !events
            .iter()
            .any(|event| matches!(event, GameEvent::Discard { player: pid, .. } if *pid == player))
}

impl GameState {
    /// 判断指定等待牌是否至少存在一种有役和牌方式。
    pub fn wait_has_yaku(&self, player: PlayerId, tile_type: TileType) -> bool {
        let p = &self.players[player.0];
        let winning_tile = Tile::from_type_index(tile_type.0, 0);
        for is_tsumo in [true, false] {
            let situation = self.make_win_situation(player, is_tsumo, winning_tile, false);
            let settlement = self
                .settlement_context(player, (!is_tsumo).then_some(PlayerId((player.0 + 1) % 4)));
            if evaluation::evaluate_win(WinInput {
                concealed_tiles: p.hand.tiles(),
                melds: &p.melds,
                winning_tile,
                dora_indicators: &self.dora_indicators,
                ura_dora_indicators: &self.ura_dora_indicators,
                situation: &situation,
                settlement,
                is_furiten: false,
            })
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
        let winning_tile = self.drawn_tile()?;
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
    fn make_win_situation(
        &self,
        player: PlayerId,
        is_tsumo: bool,
        _winning_tile: Tile,
        is_chankan: bool,
    ) -> WinSituation {
        let p = &self.players[player.0];
        let no_tiles_left = self.remaining_tiles() == 0;

        // 一发从立直宣言牌之后开始计算：立直宣言牌本身不打断一发，
        // 任何玩家的吃、碰、明杠、暗杠、加杠都会打断一发。
        // 例外：抢加杠时，加杠事件本身不打断一发（一发在抢杠窗口内保持有效）。
        let is_ippatsu = is_ippatsu_active(
            &self.events,
            player,
            is_chankan
                && matches!(
                    self.phase,
                    GamePhase::ChankanResponse {
                        kind: CallKind::Kakan,
                        ..
                    }
                ),
        );

        // 双立直在宣告时捕获并持久保存；被鸣走等后续事件不影响判定。
        let is_double_riichi = p.double_riichi && p.is_riichi;

        let has_call = self
            .events
            .iter()
            .any(|event| matches!(event, GameEvent::Call { .. }));
        let has_any_discard = self
            .events
            .iter()
            .any(|event| matches!(event, GameEvent::Discard { .. }));
        let has_player_discard = self.events.iter().any(|event| {
            matches!(
                event,
                GameEvent::Discard { player: pid, .. } if *pid == player
            )
        });

        let is_rinshan = is_tsumo && self.is_rinshan_tile(_winning_tile);

        WinSituation {
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
            seat_wind: p.wind,
            field_wind: self.wind,
        }
    }

    fn settlement_context(&self, player: PlayerId, loser: Option<PlayerId>) -> SettlementContext {
        SettlementContext {
            dealer: self.get_dealer().0,
            winner: player.0,
            loser: loser.map(|id| id.0),
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
        let mut situation = self.make_win_situation(player, is_tsumo, winning_tile, is_chankan);
        situation.is_rinshan = is_tsumo && self.is_rinshan_tile(winning_tile);

        // 检查和了
        let is_furiten = self.players[player.0].furiten.is_furiten();
        let p = &self.players[player.0];
        let result = evaluation::evaluate_win(WinInput {
            concealed_tiles: hand.tiles(),
            melds: &p.melds,
            winning_tile,
            dora_indicators: &self.dora_indicators,
            ura_dora_indicators: &self.ura_dora_indicators,
            situation: &situation,
            settlement: self.settlement_context(player, loser),
            is_furiten,
        })?;
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
    use super::{can_declare_double_riichi, is_ippatsu_active};
    use riichi_core::game::GameEvent;
    use riichi_core::player::PlayerId;
    use riichi_core::tile::Tile;

    fn discard(player: PlayerId) -> GameEvent {
        GameEvent::Discard {
            player,
            tile: Tile::from_raw(0),
            kind: riichi_core::game::DiscardKind::Tedashi,
        }
    }

    #[test]
    fn ippatsu_expires_on_the_next_own_discard() {
        let player = PlayerId(0);
        let riichi = GameEvent::Riichi { player };
        assert!(is_ippatsu_active(
            &[discard(player), riichi.clone()],
            player,
            false
        ));
        assert!(!is_ippatsu_active(
            &[discard(player), riichi, discard(player)],
            player,
            false
        ));
    }

    #[test]
    fn ippatsu_is_cancelled_by_any_call() {
        let player = PlayerId(0);
        let riichi = GameEvent::Riichi { player };
        let call = GameEvent::Call {
            player: PlayerId(1),
            tiles: vec![Tile::from_raw(0); 3],
            kind: riichi_core::game::CallKind::Pon,
            called_tile: Some(Tile::from_raw(0)),
            from_player: Some(player),
            meld_index: None,
        };
        assert!(!is_ippatsu_active(
            &[discard(player), riichi, call],
            player,
            false
        ));
    }

    #[test]
    fn ippatsu_survives_a_robbed_kakan_but_not_other_calls() {
        let player = PlayerId(0);
        let riichi = GameEvent::Riichi { player };
        let kakan = GameEvent::Call {
            player: PlayerId(1),
            tiles: vec![Tile::from_raw(4)],
            kind: riichi_core::game::CallKind::Kakan,
            called_tile: Some(Tile::from_raw(4)),
            from_player: None,
            meld_index: Some(0),
        };
        let pon = GameEvent::Call {
            player: PlayerId(2),
            tiles: vec![Tile::from_raw(0); 2],
            kind: riichi_core::game::CallKind::Pon,
            called_tile: Some(Tile::from_raw(0)),
            from_player: Some(player),
            meld_index: None,
        };
        // 抢加杠：加杠事件本身不打断一发
        assert!(is_ippatsu_active(
            &[discard(player), riichi.clone(), kakan.clone()],
            player,
            true
        ));
        // 已有一发被其他鸣牌打断，抢杠也不能恢复
        assert!(!is_ippatsu_active(
            &[discard(player), riichi.clone(), pon, kakan],
            player,
            true
        ));
        // 普通（非抢杠）和牌时一发仍被加杠打断
        assert!(!is_ippatsu_active(
            &[
                discard(player),
                riichi,
                GameEvent::Call {
                    player: PlayerId(1),
                    tiles: vec![Tile::from_raw(4)],
                    kind: riichi_core::game::CallKind::Kakan,
                    called_tile: Some(Tile::from_raw(4)),
                    from_player: None,
                    meld_index: Some(0),
                }
            ],
            player,
            false
        ));
    }

    #[test]
    fn double_riichi_requires_first_discard_and_no_calls_before_declaration() {
        let player = PlayerId(0);
        // 无鸣牌且无本人先前的弃牌 → 满足双立直条件（庄家/各家第一打）
        assert!(can_declare_double_riichi(&[], player));
        assert!(can_declare_double_riichi(
            &[discard(PlayerId(1))],
            PlayerId(2)
        ));
        assert!(can_declare_double_riichi(
            &[
                discard(PlayerId(0)),
                discard(PlayerId(1)),
                discard(PlayerId(2))
            ],
            PlayerId(3)
        ));
        // 已有本人的弃牌 → 不是双立直（第二巡起）
        assert!(!can_declare_double_riichi(&[discard(player)], player));
        // 有任何鸣牌 → 不是双立直
        let call = GameEvent::Call {
            player: PlayerId(1),
            tiles: vec![Tile::from_raw(0); 3],
            kind: riichi_core::game::CallKind::Pon,
            called_tile: Some(Tile::from_raw(0)),
            from_player: Some(player),
            meld_index: None,
        };
        assert!(!can_declare_double_riichi(
            &[discard(PlayerId(0)), call],
            PlayerId(1)
        ));
    }
}
