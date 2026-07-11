use rand::Rng;
use riichi_core::game::GameError::{InvalidAction, WallExhausted};
use riichi_core::game::{GameEvent, RoundEndReason};
use riichi_core::hand::Hand;
use riichi_core::player::FuritenState;
use riichi_core::tile::Tile;
use riichi_core::wall::Wall;

use crate::game::{GameError, GamePhase, GameState};

impl GameState {
    /// 开始新的一局
    ///
    /// 流程：
    /// 1. 创建新牌山并洗牌
    /// 2. 翻第一组宝牌指示牌
    /// 3. 重置所有玩家状态（手牌/副露/立直/振听等）
    /// 4. 配牌：每人 12 张（3 轮 × 4 张），再各摸 1 张 = 13 张
    /// 5. 庄家摸第 14 张牌进入自摸牌缓冲区
    /// 6. 进入行动阶段
    pub fn start_round(&mut self, rng: &mut impl Rng) {
        // 事件历史属于单局上下文。跨局保留事件会污染一发、双立直、
        // 四家立直和首巡等状态判断；完整对局回放应由外部日志保存。
        self.events.clear();
        self.kuikae_forbidden = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];

        // 创建新牌山
        self.wall = Wall::new(rng);
        self.drawn_tile = None;
        self.dora.clear();
        self.dora_indicators.clear();
        self.ura_dora_indicators.clear();

        // 翻第一组宝牌指示牌
        let indicator = self.wall.dora_indicator(0).tile_type();
        self.dora_indicators.push(indicator);
        self.dora.push(Self::dora_from_indicator(indicator));
        self.ura_dora_indicators
            .push(self.wall.ura_dora_indicator(0).tile_type());

        // 重置所有玩家状态，并根据当前庄家重新分配座风。
        let dealer = self.get_dealer();
        for (idx, player) in self.players.iter_mut().enumerate() {
            let relative_seat = (idx + 4 - dealer.0) % 4;
            player.wind = riichi_core::player::wind_from_index(relative_seat);
            player.hand = Hand::new();
            player.discards.clear();
            player.melds.clear();
            player.is_riichi = false;
            player.furiten = FuritenState::default();
            player.all_discarded_types.clear();
        }

        // 配牌：每人 12 张（3 轮 × 4 张）
        for _ in 0..3 {
            for player in self.players.iter_mut() {
                for _ in 0..4 {
                    let tile = self.wall.draw().unwrap();
                    player.hand.add(tile);
                }
            }
        }

        // 再各摸 1 张 = 13 张
        for player in self.players.iter_mut() {
            let tile = self.wall.draw().unwrap();
            player.hand.add(tile);
        }

        // 庄家摸第 14 张牌进入自摸牌缓冲区（不进手）
        self.current_player = self.get_dealer();
        let tile = self.wall.draw().unwrap();
        self.drawn_tile = Some(tile);
        self.phase = GamePhase::ActionPhase;

        // 记录局开始事件
        self.record_event(GameEvent::RoundStarted {
            round_number: self.round,
            dealer: self.get_dealer(),
        });
    }

    /// 从牌山摸一张牌
    ///
    /// 摸到的牌进入自摸牌缓冲区（drawn_tile），不进手牌
    /// 如果牌山耗尽，自动触发荒牌流局
    pub fn draw(&mut self) -> Result<Tile, GameError> {
        if self.remaining_tiles() == 0 {
            self.resolve_round_end(RoundEndReason::ExhaustiveDraw);
            return Err(WallExhausted);
        }
        let tile = self.wall.draw().ok_or(WallExhausted)?;
        self.drawn_tile = Some(tile);
        self.update_discard_furiten(self.current_player);
        self.record_event(GameEvent::PlayerDrew {
            player: self.current_player,
            tile,
        });
        self.phase = GamePhase::ActionPhase;
        Ok(tile)
    }

    /// 岭上补摸（杠后从岭上摸牌）
    ///
    /// 从牌山末尾的岭上区摸牌
    /// 四杠已开时不能继续摸岭上牌
    pub fn draw_rinshan(&mut self) -> Result<Tile, GameError> {
        if self.get_kan_count() > 4 {
            return Err(InvalidAction("不能在四杠已开时继续摸岭上牌".to_string()));
        }
        let tile = self
            .wall
            .draw_rinshan()
            .ok_or(InvalidAction("岭上牌已耗尽".to_string()))?;
        self.drawn_tile = Some(tile);
        self.update_discard_furiten(self.current_player);
        self.record_event(GameEvent::PlayerDrew {
            player: self.current_player,
            tile,
        });
        self.phase = GamePhase::ActionPhase;
        Ok(tile)
    }

    /// 将自摸牌从缓冲区提交到手牌
    ///
    /// 仅在需要操作手牌时调用（自摸/暗杠/加杠）
    pub fn insert_tile(&mut self) {
        if let Some(tile) = self.drawn_tile.take() {
            self.players[self.current_player.0].hand.add(tile);
        }
    }

    /// 打出一张牌
    ///
    /// 流程：
    /// 1. 食替检查：副露后不能立刻打出同类型的牌
    /// 2. 立直后只能打出摸到的牌
    /// 3. 如果打出的是自摸牌，直接从缓冲区消耗
    /// 4. 如果打出手牌，先提交自摸牌到手牌，再从手牌移除
    /// 5. 记录立直宣言牌（立直后第一次打牌）
    /// 6. 清除食替禁打、记录舍牌类型、清除振听
    /// 7. 进入响应阶段（等待其他人吃/碰/杠/荣和）
    pub fn discard(&mut self, tile: Tile) -> Result<(), GameError> {
        let cp = self.current_player.0;

        if self.kuikae_forbidden[cp].contains(&tile.tile_type()) {
            return Err(GameError::InvalidAction(format!(
                "食替：{} 不能立刻打出",
                tile
            )));
        }

        // 立直后只能打出摸到的牌
        if self.players[cp].is_riichi {
            if let Some(drawn) = self.drawn_tile {
                if tile != drawn {
                    return Err(GameError::InvalidAction(
                        "立直后只能打出摸到的牌".to_string(),
                    ));
                }
            }
        }

        if Some(tile) == self.drawn_tile {
            // 打出自摸牌：直接从缓冲区消耗，不进手
            self.drawn_tile = None;
        } else {
            // 打出手牌：先提交自摸牌到手牌，再从手牌移除
            if let Some(drawn) = self.drawn_tile.take() {
                self.players[cp].hand.add(drawn);
            }
            let player = &mut self.players[cp];
            if !player.hand.contains(tile) {
                return Err(GameError::TileNotInHand(tile));
            }
            player
                .hand
                .remove(tile)
                .map_err(|_| GameError::TileNotInHand(tile))?;
        }

        // 更新玩家状态
        let player = &mut self.players[cp];
        // 检查是否已有立直事件，如果没有则记录立直宣言牌
        let has_riichi_event = self.events.iter().any(|e| matches!(e, GameEvent::PlayerDeclaredRiichi { player: pid } if *pid == self.current_player));
        if player.is_riichi && !has_riichi_event {
            // 立直宣言牌通过事件记录，这里不需要额外操作
        }
        player.all_discarded_types.insert(tile.tile_type()); // 记录舍牌类型
        player.furiten.clear_round(); // 清除本轮振听
        self.kuikae_forbidden[cp].clear();

        // 记录打牌事件
        self.record_event(GameEvent::PlayerDiscarded {
            player: self.current_player,
            tile,
        });

        // 进入响应阶段（等待其他人吃/碰/杠/荣和）
        self.phase = GamePhase::ResponsePhase {
            discarded_tile: tile,
            discarder: self.current_player,
        };

        Ok(())
    }
}
