use riichi_core::tile::TileType;

use crate::action::GameEvent;
use crate::game::GameState;

impl GameState {
    pub(crate) fn any_call_made(&self) -> bool {
        self.events.iter().any(|e| {
            matches!(
                e,
                GameEvent::PlayerCalledPon { .. }
                    | GameEvent::PlayerCalledChi { .. }
                    | GameEvent::PlayerCalledMinkan { .. }
                    | GameEvent::PlayerCalledAnkan { .. }
                    | GameEvent::PlayerCalledKakan { .. }
            )
        })
    }

    pub(crate) fn is_first_turn(&self) -> bool {
        let mut max_discards = 0usize;
        let mut counts = [0usize; 4];
        for e in &self.events {
            if let GameEvent::PlayerDiscarded { player, .. } = e {
                counts[player.0] += 1;
                if counts[player.0] > max_discards {
                    max_discards = counts[player.0];
                }
            }
        }
        max_discards <= 1
    }

    pub(crate) fn first_turn_discards(&self) -> Vec<TileType> {
        let mut first = [None::<TileType>; 4];
        for e in &self.events {
            if let GameEvent::PlayerDiscarded { player, tile } = e {
                if first[player.0].is_none() {
                    first[player.0] = Some(tile.tile_type());
                }
            }
        }
        first.iter().filter_map(|o| *o).collect()
    }

    pub(crate) fn riichi_count(&self) -> u8 {
        self.events
            .iter()
            .filter(|e| matches!(e, GameEvent::PlayerDeclaredRiichi { .. }))
            .count() as u8
    }
}
