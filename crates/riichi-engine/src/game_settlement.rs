use riichi_core::tile::TileType;
use riichi_logic::shanten::ShantenCalculator;

use crate::action::{GameEvent, RoundEndReason};
use crate::game::{GamePhase, GameState};

impl GameState {
    pub fn resolve_exhaustive_draw(&mut self) {
        let calc = ShantenCalculator::new();
        let tenpai: [bool; 4] = [
            calc.lookup(&riichi_logic::types::TileCounts::from_tiles(
                self.players[0].hand.tiles(),
            )) == 0,
            calc.lookup(&riichi_logic::types::TileCounts::from_tiles(
                self.players[1].hand.tiles(),
            )) == 0,
            calc.lookup(&riichi_logic::types::TileCounts::from_tiles(
                self.players[2].hand.tiles(),
            )) == 0,
            calc.lookup(&riichi_logic::types::TileCounts::from_tiles(
                self.players[3].hand.tiles(),
            )) == 0,
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

        #[allow(clippy::needless_range_loop)]
        for i in 0..4 {
            self.players[i].points += payments[i];
        }

        self.events
            .push(GameEvent::ExhaustiveDrawResult { tenpai, payments });
    }

    pub fn advance_round(&mut self, reason: &RoundEndReason) {
        let dealer_continues = match reason {
            RoundEndReason::Win { winner, .. } => *winner == self.get_dealer(),
            RoundEndReason::ExhaustiveDraw => {
                let calc = ShantenCalculator::new();
                calc.lookup(&riichi_logic::types::TileCounts::from_tiles(
                    self.players[self.get_dealer().0].hand.tiles(),
                )) == 0
            }
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
            self.wind = if self.round <= 4 {
                TileType::EAST
            } else {
                TileType::SOUTH
            };
        }
    }

    pub fn is_game_over(&self) -> bool {
        self.round > 8
    }

    pub fn resolve_round_end(&mut self, reason: RoundEndReason) {
        if matches!(reason, RoundEndReason::ExhaustiveDraw) {
            self.resolve_exhaustive_draw();
        }

        self.advance_round(&reason);

        self.events.push(GameEvent::RoundEnded { reason });
        self.phase = GamePhase::RoundOver;
    }
}
