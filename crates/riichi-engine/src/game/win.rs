use super::*;

impl GameState {
    pub fn check_tsumo(&self, player: PlayerId) -> Option<([i32; 4], Vec<String>)> {
        let winning_tile = self.drawn_tile?;
        let hand = &self.players[player.0].hand;
        self.check_win_with_hand(player, true, winning_tile, None, hand, false)
    }

    fn make_win_context(
        &self,
        player: PlayerId,
        is_tsumo: bool,
        _winning_tile: Tile,
        is_chankan: bool,
    ) -> WinContext {
        let p = &self.players[player.0];
        let no_tiles_left = self.remaining_tiles() == 0;
        WinContext {
            is_tsumo,
            is_riichi: p.is_riichi,
            is_double_riichi: p.is_double_riichi,
            is_ippatsu: p.is_ippatsu,
            is_rinshan: false,
            is_chankan,
            is_haitei: no_tiles_left && is_tsumo,
            is_houtei: no_tiles_left && !is_tsumo,
            seat_wind: p.wind,
            field_wind: self.wind,
            dora_indicators: self.dora_indicators.clone(),
            ura_dora_indicators: self.ura_dora_indicators.clone(),
            melds: p.melds.clone(),
            dealer: self.get_dealer().0,
            winner: player.0,
            loser: None,
            honba: self.honba,
            riichi_sticks: self.riichi_sticks,
        }
    }

    pub(super) fn check_win(
        &self,
        player: PlayerId,
        is_tsumo: bool,
        winning_tile: Tile,
        loser: Option<PlayerId>,
        is_chankan: bool,
    ) -> Option<([i32; 4], Vec<String>)> {
        let hand = &self.players[player.0].hand;
        self.check_win_with_hand(player, is_tsumo, winning_tile, loser, hand, is_chankan)
    }

    fn check_win_with_hand(
        &self,
        player: PlayerId,
        is_tsumo: bool,
        winning_tile: Tile,
        loser: Option<PlayerId>,
        hand: &Hand,
        is_chankan: bool,
    ) -> Option<([i32; 4], Vec<String>)> {
        let mut all_tiles: Vec<Tile> = hand.tiles().to_vec();
        for meld in &self.players[player.0].melds {
            all_tiles.extend_from_slice(&meld.tiles);
        }
        all_tiles.push(winning_tile);

        let mut hand_tile_types: Vec<TileType> =
            hand.tiles().iter().map(|t| t.tile_type()).collect();
        hand_tile_types.push(winning_tile.tile_type());

        let mut ctx = self.make_win_context(player, is_tsumo, winning_tile, is_chankan);
        ctx.loser = loser.map(|id| id.0);
        ctx.is_rinshan = self.is_rinshan_tile(winning_tile);

        let is_furiten = self.players[player.0].furiten.is_furiten();
        let result =
            win_check::check_win(&all_tiles, &hand_tile_types, &ctx, is_furiten, winning_tile)?;
        let yaku_names: Vec<String> = result
            .yaku_results
            .iter()
            .map(|y| format!("{:?}", y.yaku))
            .collect();
        Some((result.points, yaku_names))
    }
}
