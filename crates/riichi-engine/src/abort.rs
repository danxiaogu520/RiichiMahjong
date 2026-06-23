use std::collections::HashSet;

use riichi_core::meld::MeldKind;
use riichi_core::player::PlayerId;

use crate::game::GameState;

impl GameState {
    pub fn can_declare_kyuushu(&self, player: PlayerId) -> bool {
        if self.any_call_made() {
            return false;
        }
        if !self.is_first_turn() {
            return false;
        }
        let hand = &self.players[player.0].hand;
        let tile_count = hand.len() + if self.drawn_tile.is_some() { 1 } else { 0 };
        if tile_count != 14 {
            return false;
        }
        let mut types = HashSet::new();
        for &tile in hand.tiles() {
            if tile.is_yaochuuhai() {
                types.insert(tile.tile_type());
            }
        }
        if let Some(drawn) = self.drawn_tile {
            if drawn.is_yaochuuhai() {
                types.insert(drawn.tile_type());
            }
        }
        types.len() >= 9
    }

    pub(crate) fn check_suufon_renda(&self) -> bool {
        let discards = self.first_turn_discards();
        if discards.len() < 4 {
            return false;
        }
        if self.any_call_made() {
            return false;
        }
        let first = discards[0];
        if !first.is_wind() {
            return false;
        }
        discards.iter().all(|&d| d == first)
    }

    pub(crate) fn check_suucha_riichi(&self) -> bool {
        self.riichi_count() >= 4
    }

    pub fn can_declare_kan(&self, player: PlayerId) -> bool {
        let total = self.get_kan_count();
        if total < 4 {
            return true;
        }
        let mut kan_owners = HashSet::new();
        for p in &self.players {
            for m in &p.melds {
                if matches!(m.kind, MeldKind::Ankan | MeldKind::Minkan | MeldKind::Kakan) {
                    kan_owners.insert(p.id);
                }
            }
        }
        kan_owners.len() == 1 && kan_owners.contains(&player)
    }

    pub fn check_four_kan_abort(&self) -> bool {
        let total = self.get_kan_count();
        if total < 4 {
            return false;
        }
        let mut kan_owners = HashSet::new();
        for p in &self.players {
            for m in &p.melds {
                if matches!(m.kind, MeldKind::Ankan | MeldKind::Minkan | MeldKind::Kakan) {
                    kan_owners.insert(p.id);
                }
            }
        }
        kan_owners.len() >= 2
    }
}
