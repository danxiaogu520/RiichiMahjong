use std::collections::HashSet;

use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use riichi_logic::analysis::analyze_wait_tiles;

use crate::action::GameEvent;
use crate::game::{GamePhase, GameState};

impl GameState {
    pub fn advance_turn(&mut self) {
        self.current_player = self.current_player.next();
    }

    pub fn is_rinshan_tile(&self, tile: Tile) -> bool {
        self.wall.is_rinshan_tile(tile)
    }

    pub fn clear_ippatsu(&mut self) {
        for player in &mut self.players {
            player.is_ippatsu = false;
        }
    }

    pub(crate) fn get_waiting_tile_types(&self, player: PlayerId) -> HashSet<TileType> {
        analyze_wait_tiles(self.players[player.0].hand.tiles())
            .iter()
            .map(|w| w.tile_type)
            .collect()
    }

    pub(crate) fn update_discard_furiten(&mut self, player: PlayerId) {
        let waiting = self.get_waiting_tile_types(player);
        let discarded = &self.players[player.0].all_discarded_types;
        self.players[player.0].furiten.discard = waiting.iter().any(|tt| discarded.contains(tt));
    }

    pub(crate) fn update_all_discard_furiten(&mut self) {
        for idx in 0..4 {
            self.update_discard_furiten(PlayerId(idx));
        }
    }

    pub fn is_round_over(&self) -> bool {
        matches!(self.phase, GamePhase::RoundOver) || self.remaining_tiles() == 0
    }

    pub fn remaining_tiles(&self) -> usize {
        self.wall.remaining()
    }

    pub fn take_events(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.events)
    }

    pub fn build_visible_tiles(&self, player: PlayerId) -> riichi_logic::acceptance::VisibleTiles {
        let mut visible = riichi_logic::acceptance::VisibleTiles::new();

        for meld in &self.players[player.0].melds {
            for t in &meld.tiles {
                visible.hand_melds.inc(t.tile_type());
            }
        }

        for i in 0..4 {
            let pid = PlayerId(i);
            if pid == player {
                continue;
            }
            for meld in &self.players[i].melds {
                for t in &meld.tiles {
                    visible.all_melds.inc(t.tile_type());
                }
            }
        }

        for i in 0..4 {
            for &t in &self.players[i].discards {
                visible.all_discards.inc(t.tile_type());
            }
        }

        for &tt in &self.dora_indicators {
            visible.dora_indicators.inc(tt);
        }

        visible
    }
}
