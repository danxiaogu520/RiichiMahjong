use riichi_core::game::{GameEvent, RoundEndReason};
use riichi_core::tile::TileType;
use crate::game::{GamePhase, GameState};

impl GameState {
    /// 返回终局排名，按点数从高到低排列；同分时按座位编号稳定排序。
    pub fn final_ranking(&self) -> [usize; 4] {
        let mut ranking = [0usize, 1, 2, 3];
        ranking.sort_by_key(|&player| (std::cmp::Reverse(self.players[player].points), player));
        ranking
    }

    /// 荒牌流局结算：计算不听罚符，更新点棒
    ///
    /// 规则：
    /// - 0 人听牌 / 4 人听牌：无收支
    /// - 1 人听牌：3 人不听各付 1000，听牌者收 3000
    /// - 2 人听牌：2 人不听各付 3000，听牌者各收 1500
    /// - 3 人听牌：1 人不听付 3000，听牌者各收 1000
    pub fn resolve_exhaustive_draw(&mut self) {
        let tenpai: [bool; 4] = [
            !self.get_waiting_tiles(riichi_core::player::PlayerId(0)).is_empty(),
            !self.get_waiting_tiles(riichi_core::player::PlayerId(1)).is_empty(),
            !self.get_waiting_tiles(riichi_core::player::PlayerId(2)).is_empty(),
            !self.get_waiting_tiles(riichi_core::player::PlayerId(3)).is_empty(),
        ];
        let tenpai_count = tenpai.iter().filter(|&&t| t).count();

        let mut payments = [0i32; 4];
        match tenpai_count {
            0 | 4 => {}
            1 => {
                let winner = tenpai.iter().position(|&t| t).unwrap();
                for i in 0..4 {
                    if !tenpai[i] {
                        payments[i] -= 1000;
                        payments[winner] += 1000;
                    }
                }
            }
            2 => {
                let winners: Vec<usize> = tenpai
                    .iter()
                    .enumerate()
                    .filter(|(_, &t)| t)
                    .map(|(i, _)| i)
                    .collect();
                for i in 0..4 {
                    if !tenpai[i] {
                        payments[i] -= 1500 * winners.len() as i32;
                        for &w in &winners {
                            payments[w] += 1500;
                        }
                    }
                }
            }
            3 => {
                let loser = tenpai.iter().position(|&t| !t).unwrap();
                for i in 0..4 {
                    if tenpai[i] {
                        payments[loser] -= 1000;
                        payments[i] += 1000;
                    }
                }
            }
            _ => unreachable!(),
        }

        // 应用点数变化
        #[allow(clippy::needless_range_loop)]
        for i in 0..4 {
            self.players[i].points += payments[i];
        }

        self.record_event(GameEvent::ExhaustiveDrawResult { tenpai, payments });
    }

    /// 根据局结束原因处理连庄/过庄，更新 round、honba、场风
    ///
    /// 规则：
    /// - 和了：和牌者是庄家 → 连庄
    /// - 荒牌流局：庄家听牌 → 连庄
    /// - 途中流局（九种九牌/四风连打/四家立直/四杠散了）：一律连庄
    /// - 连庄：round 不变, honba += 1
    /// - 过庄：round += 1, honba = 0
    pub fn advance_round(&mut self, reason: &RoundEndReason) {
        let dealer_continues = match reason {
            RoundEndReason::Win { winner, .. } => *winner == self.get_dealer(),
            RoundEndReason::MultiWin { winners } => winners.contains(&self.get_dealer()),
            RoundEndReason::ExhaustiveDraw => {
                !self
                    .get_waiting_tiles(self.get_dealer())
                    .is_empty()
            }
            // 途中流局一律连庄
            RoundEndReason::KyuushuKyuuhai
            | RoundEndReason::SuufonRenda
            | RoundEndReason::SuuchaRiichi
            | RoundEndReason::SuuKantsu => true,
        };

        if dealer_continues {
            self.honba += 1;
        } else {
            self.round += 1;
            self.honba = 0;
            // 场风更新：round 1-4 = 东场, 5-8 = 南场
            self.wind = if self.round <= 4 {
                TileType::EAST
            } else {
                TileType::SOUTH
            };
        }
    }

    /// 游戏是否结束（南四局过庄后）
    pub fn is_game_over(&self) -> bool {
        self.round > 8 || (self.rules.tobi && self.players.iter().any(|p| p.points < 0))
    }

    /// 统一处理局结束：荒牌罚符 + 连庄/过庄 + 设置 RoundOver
    pub fn resolve_round_end(&mut self, reason: RoundEndReason) {
        // 荒牌流局需要先结算罚符
        if matches!(reason, RoundEndReason::ExhaustiveDraw) {
            self.resolve_exhaustive_draw();
        }

        self.advance_round(&reason);

        self.record_event(GameEvent::RoundEnded { reason });
        self.phase = GamePhase::RoundOver;
    }
}

#[cfg(test)]
mod tests {
    use super::GameState;

    #[test]
    fn final_ranking_orders_points_stably() {
        let mut state = GameState::new();
        state.players[0].points = 20_000;
        state.players[1].points = 35_000;
        state.players[2].points = 35_000;
        state.players[3].points = 10_000;
        assert_eq!(state.final_ranking(), [1, 2, 0, 3]);
    }

    #[test]
    fn tobi_ends_game_when_enabled() {
        let mut state = GameState::new();
        state.rules.tobi = true;
        state.players[2].points = -100;
        assert!(state.is_game_over());
    }
}
