use super::*;

impl GameState {
    pub fn start_round(&mut self, rng: &mut impl Rng) {
        self.wall = Wall::new(rng);
        self.drawn_tile = None;
        self.dora.clear();
        self.dora_indicators.clear();
        self.ura_dora_indicators.clear();
        let indicator = self.wall.dora_indicator(0).tile_type();
        self.dora_indicators.push(indicator);
        self.dora.push(Self::dora_from_indicator(indicator));
        self.ura_dora_indicators
            .push(self.wall.ura_dora_indicator(0).tile_type());

        for player in &mut self.players {
            player.hand = Hand::new();
            player.discards.clear();
            player.melds.clear();
            player.is_riichi = false;
            player.is_ippatsu = false;
            player.forbidden.clear();
            player.riichi_declaration_tile = None;
            player.has_made_first_action = false;
            player.is_double_riichi = false;
            player.furiten = FuritenState::default();
            player.all_discarded_types.clear();
        }

        for _ in 0..3 {
            for player in self.players.iter_mut() {
                for _ in 0..4 {
                    let tile = self.wall.draw().unwrap();
                    player.hand.add(tile);
                }
            }
        }

        for player in self.players.iter_mut() {
            let tile = self.wall.draw().unwrap();
            player.hand.add(tile);
        }
        self.current_player = self.get_dealer();
        let tile = self.wall.draw().unwrap();
        self.drawn_tile = Some(tile);
        self.phase = GamePhase::ActionPhase;

        self.events.push(GameEvent::RoundStarted {
            round_number: self.round,
            dealer: self.get_dealer(),
        });
    }

    pub fn draw(&mut self) -> Result<Tile, GameError> {
        if self.remaining_tiles() == 0 {
            self.resolve_round_end(RoundEndReason::ExhaustiveDraw);
            return Err(WallExhausted);
        }
        let tile = self.wall.draw().ok_or(WallExhausted)?;
        self.drawn_tile = Some(tile);
        self.update_discard_furiten(self.current_player);
        self.events.push(GameEvent::PlayerDrew {
            player: self.current_player,
            tile,
        });
        self.phase = GamePhase::ActionPhase;
        Ok(tile)
    }

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
        self.events.push(GameEvent::PlayerDrew {
            player: self.current_player,
            tile,
        });
        self.phase = GamePhase::ActionPhase;
        Ok(tile)
    }

    pub(super) fn insert_tile(&mut self) {
        if let Some(tile) = self.drawn_tile.take() {
            self.players[self.current_player.0].hand.add(tile);
        }
    }

    pub fn discard(&mut self, tile: Tile) -> Result<(), GameError> {
        let cp = self.current_player.0;

        if self.players[cp].forbidden.contains(&tile.tile_type()) {
            return Err(GameError::InvalidAction(format!(
                "食替：{} 不能立刻打出",
                tile
            )));
        }

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
            self.drawn_tile = None;
        } else {
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

        {
            let player = &mut self.players[cp];
            if player.is_riichi && player.riichi_declaration_tile.is_none() {
                player.riichi_declaration_tile = Some(tile);
            }
            player.forbidden.clear();
            player.all_discarded_types.insert(tile.tile_type());
            player.furiten.clear_round();
        }

        self.events.push(GameEvent::PlayerDiscarded {
            player: self.current_player,
            tile,
        });

        self.phase = GamePhase::ResponsePhase {
            discarded_tile: tile,
            discarder: self.current_player,
        };

        Ok(())
    }
}
