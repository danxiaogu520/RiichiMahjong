use riichi_core::game::GameEvent;
use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use riichi_logic::model::TileCounts;
use riichi_logic::shanten::ShantenCalculator;
use riichi_logic::shape::analyze_wait_tiles_with_open_melds;

use crate::game::{GameError, GameState};
use crate::win::can_declare_double_riichi;

impl GameState {
    /// 获取玩家的听牌列表（手牌 13 张时调用）
    ///
    /// 返回所有能和的牌类型
    pub fn get_waiting_tiles(&self, player: PlayerId) -> Vec<TileType> {
        let p = &self.players[player.0];
        analyze_wait_tiles_with_open_melds(p.hand.tiles(), p.melds.len())
            .iter()
            .map(|w| w.tile_type)
            .collect()
    }

    /// 检测玩家是否可以宣告立直
    ///
    /// 立直条件：
    /// 1. 尚未立直
    /// 2. 门前清（无副露）
    /// 3. 点数 >= 1000
    /// 4. 剩余牌 >= 4
    /// 5. 打出任意一张后能听牌（向听数 = 0）
    pub fn can_declare_riichi(&self, player: PlayerId) -> bool {
        let p = &self.players[player.0];
        if p.is_riichi {
            return false; // 已经立直
        }
        if !p.is_menzen() {
            return false; // 非门前清
        }
        if p.points < 1000 {
            return false; // 点数不足
        }
        if self.remaining_tiles() < 4 {
            return false; // 剩余牌不足
        }
        !self.get_riichi_discard_options(player).is_empty()
    }

    /// 获取所有实际打出后仍能听牌的立直弃牌。
    ///
    /// 返回实体牌而不是牌型，能够正确区分赤五与普通五（以及网络动作
    /// 校验所需的具体牌副本）。
    pub fn get_riichi_discard_options(&self, player: PlayerId) -> Vec<Tile> {
        let p = &self.players[player.0];
        if p.is_riichi || !p.is_menzen() || p.points < 1000 || self.remaining_tiles() < 4 {
            return vec![];
        }
        let calc = ShantenCalculator::new();
        let mut tiles: Vec<Tile> = self.players[player.0].hand.tiles().to_vec();
        if let Some(t) = self.drawn_tile() {
            tiles.push(t);
        }
        let counts = TileCounts::from_tiles(&tiles);
        tiles
            .into_iter()
            .filter(|tile| {
                let mut after = counts;
                after.dec(tile.tile_type());
                calc.lookup(&after) == 0
            })
            .collect()
    }

    /// 宣告立直（仅宣告，不打牌）
    ///
    /// 宣告本身不扣除点数；宣言牌通过响应窗口（未被荣和）后
    /// 由受理流程扣除 1000 点并放置立直棒。
    pub fn execute_riichi(&mut self, player: PlayerId) -> Result<(), GameError> {
        if !self.can_declare_riichi(player) {
            return Err(GameError::InvalidAction("不满足立直条件".to_string()));
        }
        self.apply_riichi_event(player)?;
        self.record_event(GameEvent::Riichi { player });
        // 无宣言牌的独立宣告没有响应窗口，宣告即受理。
        self.accept_riichi(player);
        Ok(())
    }

    /// 应用立直宣告事件（仅标记，不扣分）。
    ///
    /// 与 Mortal 的 `reach()` 对应：记录宣告状态，并在宣告时捕获
    /// 双立直条件（之后宣言牌被鸣走或发生其他事件不影响该判定）。
    pub(crate) fn apply_riichi_event(&mut self, player: PlayerId) -> Result<(), GameError> {
        if self.players[player.0].riichi_declared || self.players[player.0].is_riichi {
            return Err(GameError::InvalidAction("玩家已经立直".to_string()));
        }
        if self.players[player.0].points < 1000 {
            return Err(GameError::InvalidAction("立直点数不足".to_string()));
        }
        // 双立直在宣告时捕获：无任何鸣牌且这是本人的第一打。
        self.players[player.0].double_riichi = can_declare_double_riichi(&self.events, player);
        self.players[player.0].riichi_declared = true;
        Ok(())
    }

    /// 立直受理：宣言牌通过响应窗口（未被荣和）时扣除 1000 点并放置立直棒。
    ///
    /// 与 Mortal 的 `reach_accepted()` 一致：宣告本身不扣分不放棒，
    /// 宣言牌被荣和或途中流局时立直不成立。
    pub(crate) fn accept_riichi(&mut self, player: PlayerId) {
        let p = &mut self.players[player.0];
        if p.is_riichi || !p.riichi_declared {
            return;
        }
        p.points -= 1000;
        p.is_riichi = true;
        self.riichi_sticks += 1;
    }

    /// 立直后可用的暗杠选项
    ///
    /// 立直后暗杠必须满足：
    /// 1. 暗杠必须包含摸到的牌（送り槓不可）
    /// 2. 暗杠后听牌种类与杠前完全相等
    ///
    /// 实现：比较暗杠前（13张手牌）的听牌与暗杠后（10张手牌）的听牌
    pub fn get_riichi_ankan_options(&self, player: PlayerId) -> Vec<Tile> {
        let p = &self.players[player.0];
        if !p.is_riichi {
            return vec![];
        }
        let hand = &p.hand;
        let waits_before: std::collections::HashSet<TileType> =
            analyze_wait_tiles_with_open_melds(hand.tiles(), p.melds.len())
                .iter()
                .map(|w| w.tile_type)
                .collect();

        if waits_before.is_empty() {
            return vec![];
        }

        let Some(drawn) = self.drawn_tile() else {
            return vec![];
        };

        let mut options = Vec::new();
        for tt in (0..34u8).map(TileType) {
            // 送り槓不可：暗杠必须是“手中 3 张 + 刚摸到的第 4 张”。
            if drawn.tile_type() != tt || hand.count_type(tt) < 3 {
                continue;
            }

            let mut hand_after = hand.clone();
            hand_after
                .add(drawn)
                .expect("计算立直选项时手牌不应超过容量");
            let mut removed = 0;
            for tile in hand_after.tiles().to_vec() {
                if tile.tile_type() == tt && removed < 4 {
                    hand_after.remove(tile).ok();
                    removed += 1;
                }
            }
            if removed != 4 {
                continue;
            }

            let waits_after: std::collections::HashSet<TileType> =
                analyze_wait_tiles_with_open_melds(hand_after.tiles(), p.melds.len() + 1)
                    .iter()
                    .map(|w| w.tile_type)
                    .collect();
            if waits_before == waits_after {
                if let Some(tile) = hand.tiles().iter().find(|tile| tile.tile_type() == tt) {
                    options.push(*tile);
                } else if let Some(drawn) = self.drawn_tile().filter(|tile| tile.tile_type() == tt)
                {
                    options.push(drawn);
                }
            }
        }
        options
    }
}

#[cfg(test)]
mod tests {
    use super::GameState;
    use rand::SeedableRng;
    use riichi_core::hand::Hand;
    use riichi_core::player::PlayerId;
    use riichi_core::tile::Tile;
    use riichi_core::wall::Wall;

    #[test]
    fn riichi_options_only_contain_discards_that_keep_tenpai() {
        let mut state = GameState::new();
        let mut rng = rand::rngs::StdRng::seed_from_u64(23);
        state.wall = Wall::new(&mut rng);
        state.players[0].hand = Hand::from_tiles(&[
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
            Tile::from_raw(37),
            Tile::from_raw(40),
            Tile::from_raw(44),
        ]);
        state.phase = riichi_core::game::GamePhase::ActionPhase {
            player: PlayerId(0),
            drawn_tile: Some(Tile::from_raw(104)),
        };

        let options = state.get_riichi_discard_options(PlayerId(0));
        assert!(options.contains(&Tile::from_raw(104)));
        assert!(!options.contains(&Tile::from_raw(0)));
    }

    #[test]
    fn riichi_ankan_is_forbidden_when_the_kan_skips_the_drawn_tile() {
        // 1111m 23m 456p 789p 5s + 摸 6s：手中已有 4 张 1m，但摸牌不是 1m —— 送り槓不可
        let mut state = GameState::new();
        let mut rng = rand::rngs::StdRng::seed_from_u64(23);
        state.wall = Wall::new(&mut rng);
        state.players[0].hand = Hand::from_tiles(&[
            Tile::from_raw(0),
            Tile::from_raw(1),
            Tile::from_raw(2),
            Tile::from_raw(3), // 1111m
            Tile::from_raw(4),
            Tile::from_raw(8), // 23m
            Tile::from_raw(36),
            Tile::from_raw(40),
            Tile::from_raw(44), // 456p
            Tile::from_raw(48),
            Tile::from_raw(52),
            Tile::from_raw(56), // 789p
            Tile::from_raw(88), // 5s
        ]);
        state.players[0].is_riichi = true;
        state.phase = riichi_core::game::GamePhase::ActionPhase {
            player: PlayerId(0),
            drawn_tile: Some(Tile::from_raw(92)), // 6s
        };

        assert!(state.get_riichi_ankan_options(PlayerId(0)).is_empty());
    }

    #[test]
    fn riichi_ankan_is_allowed_when_the_drawn_tile_completes_the_kan() {
        // 111m 23m 44m 456p 789p + 摸 1m：听 1m/4m；暗杠 1m 后仍听 1m/4m（听牌完全相等）
        let mut state = GameState::new();
        let mut rng = rand::rngs::StdRng::seed_from_u64(23);
        state.wall = Wall::new(&mut rng);
        state.players[0].hand = Hand::from_tiles(&[
            Tile::from_raw(0),
            Tile::from_raw(1),
            Tile::from_raw(2), // 111m
            Tile::from_raw(4),
            Tile::from_raw(8), // 23m
            Tile::from_raw(12),
            Tile::from_raw(13), // 44m
            Tile::from_raw(48),
            Tile::from_raw(52),
            Tile::from_raw(56), // 456p
            Tile::from_raw(60),
            Tile::from_raw(64),
            Tile::from_raw(68), // 789p
        ]);
        state.players[0].is_riichi = true;
        state.phase = riichi_core::game::GamePhase::ActionPhase {
            player: PlayerId(0),
            drawn_tile: Some(Tile::from_raw(3)), // 第 4 张 1m
        };

        let options = state.get_riichi_ankan_options(PlayerId(0));
        assert!(options
            .iter()
            .any(|t| t.tile_type() == riichi_core::tile::TileType(0)));
    }
}
