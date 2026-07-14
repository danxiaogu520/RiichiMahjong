use riichi_core::game::GameEvent;
use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use riichi_logic::analysis::analyze_wait_tiles_with_open_melds;
use riichi_logic::shanten::ShantenCalculator;
use riichi_logic::types::TileCounts;

use crate::game::{GameError, GameState};

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
    /// 扣除 1000 点，设置立直标记，放置立直棒
    pub fn execute_riichi(&mut self, player: PlayerId) -> Result<(), GameError> {
        if !self.can_declare_riichi(player) {
            return Err(GameError::InvalidAction("不满足立直条件".to_string()));
        }
        let p = &mut self.players[player.0];
        p.points -= 1000;
        p.is_riichi = true;
        self.riichi_sticks += 1;
        self.record_event(GameEvent::PlayerDeclaredRiichi { player });
        Ok(())
    }

    /// 立直后可用的暗杠选项
    ///
    /// 立直后暗杠必须满足：
    /// 1. 暗杠的 4 张牌包含摸到的牌
    /// 2. 暗杠后听牌种类不变
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

        let mut options = Vec::new();
        for tt in (0..34u8).map(TileType) {
            let hand_count = hand.count_type(tt);
            let drawn_count = self.drawn_tile().is_some_and(|tile| tile.tile_type() == tt) as usize;
            if hand_count + drawn_count < 4 {
                continue;
            }

            let mut hand_after = hand.clone();
            if let Some(drawn) = self.drawn_tile() {
                hand_after
                    .add(drawn)
                    .expect("计算立直选项时手牌不应超过容量");
            }
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
}
