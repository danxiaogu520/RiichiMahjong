use riichi_core::meld::{Meld, MeldKind};
use riichi_core::player::PlayerId;
use riichi_core::tile::Tile;
use riichi_logic::analysis::analyze_wait_tiles;

use crate::action::{CallOption, CallType, GameEvent, ResponseAction, RoundEndReason, TurnAction};
use crate::game::{extract_kuikae_tiles, GameError, GamePhase, GameState};

impl GameState {
    pub fn execute_action(&mut self, action: TurnAction) -> Result<Vec<GameEvent>, GameError> {
        if !matches!(self.phase, GamePhase::ActionPhase) {
            return Err(GameError::InvalidAction("不在行动阶段".to_string()));
        }

        let mut new_events = Vec::new();

        match action {
            TurnAction::Discard(tile) => {
                self.discard(tile)?;
                self.players[self.current_player.0].has_made_first_action = true;
            }

            TurnAction::RiichiDiscard(tile) => {
                if !self.can_declare_riichi(self.current_player) {
                    return Err(GameError::InvalidAction("不满足立直条件".to_string()));
                }
                self.insert_tile();
                if !self.players[self.current_player.0].hand.contains(tile) {
                    return Err(GameError::TileNotInHand(tile));
                }
                let mut simulated = self.players[self.current_player.0].hand.clone();
                simulated
                    .remove(tile)
                    .map_err(|_| GameError::TileNotInHand(tile))?;
                if analyze_wait_tiles(simulated.tiles()).is_empty() {
                    return Err(GameError::InvalidAction(
                        "立直宣言牌必须使手牌听牌".to_string(),
                    ));
                }
                {
                    let p = &mut self.players[self.current_player.0];
                    let is_double = !p.has_made_first_action;
                    p.points -= 1000;
                    p.is_riichi = true;
                    p.is_double_riichi = is_double;
                    p.riichi_declaration_tile = Some(tile);
                }
                self.riichi_sticks += 1;
                new_events.push(GameEvent::PlayerDeclaredRiichi {
                    player: self.current_player,
                });
                self.discard(tile)?;
                self.players[self.current_player.0].has_made_first_action = true;

                if matches!(self.phase, GamePhase::ResponsePhase { .. })
                    && self.check_suufon_renda()
                {
                    self.resolve_round_end(RoundEndReason::SuufonRenda);
                } else if matches!(self.phase, GamePhase::ResponsePhase { .. })
                    && self.check_suucha_riichi()
                {
                    self.resolve_round_end(RoundEndReason::SuuchaRiichi);
                }
            }

            TurnAction::Tsumo => {
                let winning_tile = self.drawn_tile.ok_or_else(|| {
                    GameError::InvalidAction("没有摸到的牌，无法自摸".to_string())
                })?;
                let result = self.check_win(self.current_player, true, winning_tile, None, false);
                if let Some((changes, yaku_names)) = result {
                    self.insert_tile();
                    new_events.push(GameEvent::PlayerWon {
                        player: self.current_player,
                        is_tsumo: true,
                        points: changes[self.current_player.0],
                        yaku_names,
                    });
                    self.resolve_round_end(RoundEndReason::Win {
                        winner: self.current_player,
                        is_tsumo: true,
                    });
                } else {
                    return Err(GameError::InvalidAction("无法自摸和".to_string()));
                }
            }

            TurnAction::KyuushuKyuuhai => {
                if !self.can_declare_kyuushu(self.current_player) {
                    return Err(GameError::InvalidAction("不满足九种九牌条件".to_string()));
                }
                self.resolve_round_end(RoundEndReason::KyuushuKyuuhai);
            }

            TurnAction::Ankan(tile) => {
                self.insert_tile();
                let events = self.execute_ankan(self.current_player, tile)?;
                new_events.extend(events);
            }

            TurnAction::Kakan(meld_index, tile) => {
                self.insert_tile();
                let events = self.execute_kakan(self.current_player, meld_index, tile)?;
                new_events.extend(events);
            }
        }

        self.events.extend(new_events.clone());
        Ok(new_events)
    }

    pub fn get_call_options(&self) -> Vec<CallOption> {
        match self.phase {
            GamePhase::ResponsePhase {
                discarded_tile,
                discarder,
            } => crate::call::detect_calls(&self.players, discarded_tile, discarder),
            GamePhase::ChankanResponse {
                kakan_tile,
                kakan_player,
                ..
            } => {
                let mut options = Vec::new();
                for idx in 0..4 {
                    let pid = PlayerId(idx);
                    if pid == kakan_player {
                        continue;
                    }
                    let mut test_tiles: Vec<Tile> = self.players[idx].hand.tiles().to_vec();
                    test_tiles.push(kakan_tile);
                    let mut counts = riichi_logic::types::TileCounts::from_tiles(&test_tiles);
                    if riichi_logic::analysis::is_winning(&mut counts) {
                        options.push(CallOption {
                            player: pid,
                            call_type: CallType::Ron,
                        });
                    }
                }
                options
            }
            _ => Vec::new(),
        }
    }

    pub fn execute_call(
        &mut self,
        player: PlayerId,
        action: ResponseAction,
    ) -> Result<Vec<GameEvent>, GameError> {
        let mut new_events = Vec::new();

        match self.phase {
            GamePhase::ResponsePhase {
                discarded_tile,
                discarder,
            } => {
                self.execute_response_call(
                    player,
                    action,
                    discarded_tile,
                    discarder,
                    &mut new_events,
                )?;
            }
            GamePhase::ChankanResponse {
                kakan_tile,
                kakan_player,
                meld_index,
            } => {
                self.execute_chankan_call(
                    player,
                    action,
                    kakan_tile,
                    kakan_player,
                    meld_index,
                    &mut new_events,
                )?;
            }
            _ => return Err(GameError::InvalidAction("不在响应阶段".to_string())),
        }

        self.events.extend(new_events.clone());
        Ok(new_events)
    }

    fn execute_response_call(
        &mut self,
        player: PlayerId,
        action: ResponseAction,
        discarded_tile: Tile,
        discarder: PlayerId,
        new_events: &mut Vec<GameEvent>,
    ) -> Result<(), GameError> {
        match action {
            ResponseAction::Pass => {
                self.players[discarder.0].discards.push(discarded_tile);

                for idx in 0..4 {
                    let pid = PlayerId(idx);
                    if pid == discarder {
                        continue;
                    }
                    let waiting = self.get_waiting_tile_types(pid);
                    if waiting.contains(&discarded_tile.tile_type()) {
                        if self.players[idx].is_riichi {
                            self.players[idx].furiten.riichi = true;
                        } else {
                            self.players[idx].furiten.round = true;
                        }
                    }
                }

                self.update_all_discard_furiten();
                self.advance_turn();
                self.phase = GamePhase::DrawPhase;
            }
            ResponseAction::Ron => {
                self.clear_ippatsu();
                let result = self.check_win(player, false, discarded_tile, Some(discarder), false);
                if let Some((changes, yaku_names)) = result {
                    self.players[player.0].hand.add(discarded_tile);
                    new_events.push(GameEvent::PlayerWon {
                        player,
                        is_tsumo: false,
                        points: changes[player.0],
                        yaku_names,
                    });
                    self.resolve_round_end(RoundEndReason::Win {
                        winner: player,
                        is_tsumo: false,
                    });
                } else {
                    self.players[discarder.0].discards.push(discarded_tile);
                    self.update_all_discard_furiten();
                    self.advance_turn();
                    self.phase = GamePhase::DrawPhase;
                }
            }
            ResponseAction::Pon { hand_tiles } => {
                self.clear_ippatsu();
                {
                    let p = &mut self.players[player.0];
                    for &tile in &hand_tiles {
                        p.hand
                            .remove(tile)
                            .map_err(|_| GameError::TileNotInHand(tile))?;
                    }
                    let mut meld_tiles = hand_tiles.to_vec();
                    meld_tiles.push(discarded_tile);
                    let meld = Meld::pon(meld_tiles, discarded_tile, discarder);
                    let kuikae = extract_kuikae_tiles(&meld);
                    p.melds.push(meld);
                    p.forbidden = kuikae;
                }
                self.current_player = player;
                self.phase = GamePhase::ActionPhase;
                self.update_discard_furiten(player);
                new_events.push(GameEvent::PlayerCalledPon {
                    player,
                    tiles: hand_tiles.to_vec(),
                    from_player: discarder,
                });
            }
            ResponseAction::Chi { hand_tiles } => {
                self.clear_ippatsu();
                {
                    let p = &mut self.players[player.0];
                    for &tile in &hand_tiles {
                        p.hand
                            .remove(tile)
                            .map_err(|_| GameError::TileNotInHand(tile))?;
                    }
                    let mut meld_tiles = hand_tiles.to_vec();
                    meld_tiles.push(discarded_tile);
                    let meld = Meld::chi(meld_tiles, discarded_tile, discarder);
                    let kuikae = extract_kuikae_tiles(&meld);
                    p.melds.push(meld);
                    p.forbidden = kuikae;
                }
                self.current_player = player;
                self.phase = GamePhase::ActionPhase;
                self.update_discard_furiten(player);
                new_events.push(GameEvent::PlayerCalledChi {
                    player,
                    tiles: hand_tiles.to_vec(),
                    from_player: discarder,
                });
            }
            ResponseAction::Minkan { hand_tiles } => {
                if !self.can_declare_kan(player) {
                    return Err(GameError::InvalidAction(
                        "四杠限制：不能继续开杠".to_string(),
                    ));
                }
                self.clear_ippatsu();
                {
                    let p = &mut self.players[player.0];
                    for &tile in &hand_tiles {
                        p.hand
                            .remove(tile)
                            .map_err(|_| GameError::TileNotInHand(tile))?;
                    }
                    let mut meld_tiles = hand_tiles.to_vec();
                    meld_tiles.push(discarded_tile);
                    p.melds
                        .push(Meld::minkan(meld_tiles, discarded_tile, discarder));
                }
                self.current_player = player;
                self.reveal_dora_indicator();
                self.draw_rinshan()?;
                new_events.push(GameEvent::PlayerCalledMinkan {
                    player,
                    tiles: hand_tiles.to_vec(),
                    from_player: discarder,
                });
                if self.check_four_kan_abort() {
                    self.resolve_round_end(RoundEndReason::SuuKantsu);
                }
            }
        }

        Ok(())
    }

    fn execute_chankan_call(
        &mut self,
        player: PlayerId,
        action: ResponseAction,
        kakan_tile: Tile,
        kakan_player: PlayerId,
        meld_index: usize,
        new_events: &mut Vec<GameEvent>,
    ) -> Result<(), GameError> {
        match action {
            ResponseAction::Pass => {
                self.current_player = kakan_player;
                self.draw_rinshan()?;

                if self.check_four_kan_abort() {
                    self.resolve_round_end(RoundEndReason::SuuKantsu);
                } else {
                    self.phase = GamePhase::ActionPhase;
                }
            }
            ResponseAction::Ron => {
                {
                    let meld = &mut self.players[kakan_player.0].melds[meld_index];
                    debug_assert!(meld.kind == MeldKind::Kakan);
                    meld.tiles.pop();
                    meld.kind = MeldKind::Pon;
                }

                self.clear_ippatsu();

                let result = self.check_win(player, false, kakan_tile, Some(kakan_player), true);
                if let Some((changes, yaku_names)) = result {
                    self.players[player.0].hand.add(kakan_tile);
                    new_events.push(GameEvent::PlayerWon {
                        player,
                        is_tsumo: false,
                        points: changes[player.0],
                        yaku_names,
                    });
                    self.resolve_round_end(RoundEndReason::Win {
                        winner: player,
                        is_tsumo: false,
                    });
                }
            }
            _ => {
                return Err(GameError::InvalidAction(
                    "抢杠响应阶段只能荣和或过".to_string(),
                ));
            }
        }
        Ok(())
    }

    pub fn get_ankan_options(&self, player: PlayerId) -> Vec<Tile> {
        let hand = &self.players[player.0].hand;
        let mut seen = std::collections::HashSet::new();
        let mut options = Vec::new();
        for &tile in hand.tiles() {
            let tt = tile.tile_type();
            if seen.insert(tt) && hand.count_type(tt.0) == 4 {
                options.push(tile);
            }
        }
        if let Some(drawn) = self.drawn_tile {
            let drawn_tt = drawn.tile_type();
            if !options.iter().any(|t| t.tile_type() == drawn_tt)
                && hand.count_type(drawn_tt.0) == 3
            {
                options.push(drawn);
            }
        }
        options
    }

    pub fn execute_ankan(
        &mut self,
        player: PlayerId,
        tile: Tile,
    ) -> Result<Vec<GameEvent>, GameError> {
        let tt = tile.tile_type();
        if self.players[player.0].hand.count_type(tt.0) < 4 {
            return Err(GameError::InvalidAction("手中没有 4 张相同牌".to_string()));
        }

        if !self.can_declare_kan(player) {
            return Err(GameError::InvalidAction(
                "四杠限制：不能继续开杠".to_string(),
            ));
        }

        if self.players[player.0].is_riichi {
            let valid_tiles = self.get_riichi_ankan_options(player);
            if !valid_tiles.iter().any(|t| t.tile_type() == tt) {
                return Err(GameError::InvalidAction(
                    "立直后暗杠不改变听牌种类".to_string(),
                ));
            }
        }

        let tiles_to_remove: Vec<Tile> = self.players[player.0]
            .hand
            .tiles()
            .iter()
            .filter(|t| t.tile_type() == tt)
            .take(4)
            .copied()
            .collect();

        {
            let p = &mut self.players[player.0];
            for &t in &tiles_to_remove {
                p.hand.remove(t).map_err(|_| GameError::TileNotInHand(t))?;
            }
            p.melds.push(Meld::ankan(tiles_to_remove.clone()));
        }

        let new_events = vec![GameEvent::PlayerCalledAnkan {
            player,
            tiles: tiles_to_remove,
        }];

        self.reveal_dora_indicator();
        self.current_player = player;
        self.draw_rinshan()?;

        if self.check_four_kan_abort() {
            self.resolve_round_end(RoundEndReason::SuuKantsu);
        }

        self.events.extend(new_events.clone());
        Ok(new_events)
    }

    pub fn get_kakan_options(&self, player: PlayerId) -> Vec<(usize, Tile)> {
        let p = &self.players[player.0];
        let mut options = Vec::new();
        for (i, meld) in p.melds.iter().enumerate() {
            if meld.kind == riichi_core::meld::MeldKind::Pon {
                let tt = meld.tiles[0].tile_type();
                if let Some(&tile) = p.hand.tiles().iter().find(|t| t.tile_type() == tt) {
                    options.push((i, tile));
                }
                if let Some(drawn) = self.drawn_tile {
                    if drawn.tile_type() == tt {
                        options.push((i, drawn));
                    }
                }
            }
        }
        options
    }

    pub fn execute_kakan(
        &mut self,
        player: PlayerId,
        meld_index: usize,
        tile: Tile,
    ) -> Result<Vec<GameEvent>, GameError> {
        {
            let meld = &self.players[player.0].melds[meld_index];
            if meld.kind != riichi_core::meld::MeldKind::Pon {
                return Err(GameError::InvalidAction("该副露不是碰".to_string()));
            }
            let tt = meld.tiles[0].tile_type();
            if tile.tile_type() != tt {
                return Err(GameError::InvalidAction("牌与碰副露不匹配".to_string()));
            }
        }

        if !self.can_declare_kan(player) {
            return Err(GameError::InvalidAction(
                "四杠限制：不能继续开杠".to_string(),
            ));
        }

        let original_pon;
        {
            let p = &mut self.players[player.0];
            p.hand
                .remove(tile)
                .map_err(|_| GameError::TileNotInHand(tile))?;

            let meld = &mut p.melds[meld_index];
            original_pon = meld.tiles.clone();
            let from_player = meld.from_player;
            let called_tile = meld.called_tile;
            let mut new_tiles = original_pon.clone();
            new_tiles.push(tile);
            *meld = Meld {
                kind: riichi_core::meld::MeldKind::Kakan,
                tiles: new_tiles,
                called_tile,
                from_player,
            };
        }

        let new_events = vec![GameEvent::PlayerCalledKakan {
            player,
            tile,
            original_pon,
        }];

        self.current_player = player;
        self.reveal_dora_indicator();

        self.phase = GamePhase::ChankanResponse {
            kakan_tile: tile,
            kakan_player: player,
            meld_index,
        };

        self.events.extend(new_events.clone());
        Ok(new_events)
    }
}
