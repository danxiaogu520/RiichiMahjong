use super::*;

impl GameState {
    pub fn get_waiting_tiles(&self, player: PlayerId) -> Vec<TileType> {
        analyze_wait_tiles(self.players[player.0].hand.tiles())
            .iter()
            .map(|w| w.tile_type)
            .collect()
    }

    pub fn can_declare_riichi(&self, player: PlayerId) -> bool {
        let p = &self.players[player.0];
        if p.is_riichi {
            return false;
        }
        if !p.is_menzen() {
            return false;
        }
        if p.points < 1000 {
            return false;
        }
        if self.remaining_tiles() < 4 {
            return false;
        }
        let calc = ShantenCalculator::new();
        let mut tiles: Vec<Tile> = self.players[player.0].hand.tiles().to_vec();
        if let Some(t) = self.drawn_tile {
            tiles.push(t);
        }
        let counts = riichi_logic::types::TileCounts::from_tiles(&tiles);
        tiles.iter().any(|tile| {
            let mut after = counts;
            after.dec(tile.tile_type());
            calc.lookup(&after) == 0
        })
    }

    pub fn execute_riichi(&mut self, player: PlayerId) -> Result<(), GameError> {
        if !self.can_declare_riichi(player) {
            return Err(GameError::InvalidAction("不满足立直条件".to_string()));
        }
        let p = &mut self.players[player.0];
        p.points -= 1000;
        p.is_riichi = true;
        self.riichi_sticks += 1;
        self.events.push(GameEvent::PlayerDeclaredRiichi { player });
        Ok(())
    }

    pub fn get_riichi_ankan_options(&self, player: PlayerId) -> Vec<Tile> {
        let p = &self.players[player.0];
        if !p.is_riichi {
            return vec![];
        }
        let drawn = match self.drawn_tile {
            Some(t) => t,
            None => return vec![],
        };
        let drawn_tt = drawn.tile_type();

        let hand = &p.hand;
        let hand_count = hand.count_type(drawn_tt.0);

        if hand_count != 3 {
            return vec![];
        }

        let waits_before: std::collections::HashSet<TileType> = analyze_wait_tiles(hand.tiles())
            .iter()
            .map(|w| w.tile_type)
            .collect();

        if waits_before.is_empty() {
            return vec![];
        }

        let mut hand_after = hand.clone();
        let tiles_to_remove: Vec<Tile> = hand
            .tiles()
            .iter()
            .filter(|t| t.tile_type() == drawn_tt)
            .take(3)
            .copied()
            .collect();
        for t in &tiles_to_remove {
            hand_after.remove(*t).ok();
        }

        let base_counts = TileCounts::from_tiles(hand_after.tiles());
        let waits_after: std::collections::HashSet<TileType> = (0..34u8)
            .map(TileType)
            .filter(|&tt| {
                if base_counts.get(tt) >= 4 {
                    return false;
                }
                let mut counts = base_counts;
                counts.inc(tt);
                is_standard_win(&mut counts)
            })
            .collect();

        if waits_before == waits_after {
            vec![drawn]
        } else {
            vec![]
        }
    }
}
