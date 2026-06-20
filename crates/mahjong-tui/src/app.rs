use mahjong_ai::analysis::{analyze_acceptance, analyze_discard, AcceptanceInfo, DiscardOption, VisibleTiles};
use mahjong_ai::shanten::ShantenCalculator;
use mahjong_core::player::PlayerId;
use mahjong_core::tile::{Tile, TileType};
use mahjong_engine::action::{CallOption, CallType, GameEvent, ResponseAction, RoundEndReason, TurnAction};
use mahjong_engine::game::{GamePhase, GameState};
use rand::rngs::StdRng;
use rand::SeedableRng;

pub struct App {
    pub game: GameState,
    pub calc: ShantenCalculator,
    pub rng: StdRng,
    pub should_quit: bool,
    pub show_result: bool,
    pub selected: usize,
    pub call_options: Vec<CallOption>,
    pub call_selected: usize,
    pub messages: Vec<String>,
    pub analysis: Vec<DiscardOption>,
    pub acceptance: Vec<AcceptanceInfo>,
    pub improvement: Vec<AcceptanceInfo>,
    pub acceptance_shanten: i8,
    pub round_end_reason: Option<RoundEndReason>,
}

impl App {
    pub fn new() -> Self {
        let mut game = GameState::new();
        let mut rng = StdRng::from_entropy();
        let mut calc = ShantenCalculator::new();
        game.start_round(&mut rng);
        let analysis = Self::compute_analysis(&mut calc, &game);
        let (acceptance, improvement, acceptance_shanten) = Self::compute_acceptance(&mut calc, &game);

        Self {
            game,
            calc,
            rng,
            should_quit: false,
            show_result: false,
            selected: 0,
            call_options: Vec::new(),
            call_selected: 0,
            messages: Vec::new(),
            analysis,
            acceptance,
            improvement,
            acceptance_shanten,
            round_end_reason: None,
        }
    }

    pub fn is_human_turn(&self) -> bool {
        self.game.current_player == PlayerId(0)
            && matches!(self.game.phase, GamePhase::ActionPhase)
    }

    pub fn needs_human_response(&self) -> bool {
        matches!(
            self.game.phase,
            GamePhase::ResponsePhase { .. } | GamePhase::ChankanResponse { .. }
        ) && !self.call_options.is_empty()
    }

    pub fn hand_tiles(&self) -> Vec<Tile> {
        let mut tiles: Vec<Tile> = self.game.players[0].hand.tiles().to_vec();
        if let Some(drawn) = self.game.drawn_tile {
            tiles.push(drawn);
        }
        tiles
    }

    pub fn execute_ai_turn(&mut self) {
        let player = self.game.current_player;
        match self.game.phase {
            GamePhase::DrawPhase => {
                if self.game.draw().is_err() {
                    self.handle_round_end();
                }
                self.refresh_analysis();
            }
            GamePhase::ActionPhase => {
                if self.game.check_tsumo(player).is_some() {
                    let name = self.player_name(player.0).to_string();
                    match self.game.execute_action(TurnAction::Tsumo) {
                        Ok(events) => {
                            for e in &events {
                                if let GameEvent::PlayerWon { yaku_names, points, .. } = e {
                                    self.messages.push(format!("{} 自摸！ {} 点", name, points.abs()));
                                    for yaku in yaku_names {
                                        self.messages.push(format!("  {}", yaku));
                                    }
                                }
                            }
                            self.handle_round_end();
                        }
                        Err(_) => {
                            self.ai_discard(player);
                        }
                    }
                    return;
                }

                if self.game.can_declare_riichi(player) {
                    let best_tile = self.ai_choose_riichi_tile(player);
                    if let Some(tile) = best_tile {
                        let name = self.player_name(player.0).to_string();
                        match self.game.execute_action(TurnAction::RiichiDiscard(tile)) {
                            Ok(_) => {
                                self.messages.push(format!("{} 立直！打出 {}", name, tile));
                                self.process_ai_responses();
                            }
                            Err(_) => {
                                self.ai_discard(player);
                            }
                        }
                        return;
                    }
                }

                self.ai_discard(player);
                self.process_ai_responses();
            }
            GamePhase::RoundOver => {
                self.handle_round_end();
            }
            _ => {}
        }
    }

    pub fn process_ai_responses(&mut self) {
        loop {
            match self.game.phase {
                GamePhase::ResponsePhase { discarder, .. }
                | GamePhase::ChankanResponse {
                    kakan_player: discarder, ..
                } => {
                    self.refresh_call_options();
                    if self.needs_human_response() {
                        return;
                    }

                    let call_options = self.game.get_call_options();

                    let ai_ron = call_options.iter().find(|o| {
                        o.player != discarder
                            && o.player != PlayerId(0)
                            && matches!(o.call_type, CallType::Ron)
                    });

                    if let Some(r) = ai_ron {
                        let pid = r.player;
                        let name = self.player_name(pid.0).to_string();
                        match self.game.execute_call(pid, ResponseAction::Ron) {
                            Ok(events) => {
                                for e in &events {
                                    if let GameEvent::PlayerWon {
                                        yaku_names, points, ..
                                    } = e
                                    {
                                        self.messages.push(format!(
                                            "{} 荣和！ {} 点",
                                            name,
                                            points.abs()
                                        ));
                                        for yaku in yaku_names {
                                            self.messages.push(format!("  {}", yaku));
                                        }
                                    }
                                }
                                self.handle_round_end();
                                return;
                            }
                            Err(_) => {}
                        }
                    }

                    let human_ron = call_options
                        .iter()
                        .find(|o| o.player == PlayerId(0) && matches!(o.call_type, CallType::Ron));
                    if human_ron.is_some() && self.needs_human_response() {
                        return;
                    }

                    let _ = self.game.execute_call(discarder, ResponseAction::Pass);

                    if matches!(self.game.phase, GamePhase::DrawPhase) {
                        if self.game.draw().is_err() {
                            self.handle_round_end();
                        }
                        self.refresh_analysis();
                        return;
                    }
                    if matches!(self.game.phase, GamePhase::RoundOver) {
                        self.handle_round_end();
                        return;
                    }
                }
                _ => return,
            }
        }
    }

    fn ai_discard(&mut self, player: PlayerId) {
        let visible = self.build_visible_tiles(player);
        let hand = &self.game.players[player.0].hand;
        let analysis = analyze_discard(&mut self.calc, hand.tiles(), &visible);
        let best = analysis.first().cloned();
        if let Some(best) = best {
            match self.game.execute_action(TurnAction::Discard(best.tile)) {
                Ok(_) => {
                    let name = self.player_name(player.0);
                    self.messages.push(format!(
                        "{} 打出 {} (进张:{}种{}张)",
                        name, best.tile, best.acceptance_types, best.acceptance_copies
                    ));
                }
                Err(_) => {
                    let hand = &self.game.players[player.0].hand;
                    if let Some(&tile) = hand.tiles().first() {
                        let _ = self.game.execute_action(TurnAction::Discard(tile));
                    }
                }
            }
        } else {
            let hand = &self.game.players[player.0].hand;
            if let Some(&tile) = hand.tiles().first() {
                let _ = self.game.execute_action(TurnAction::Discard(tile));
            }
        }
    }

    fn ai_choose_riichi_tile(&mut self, player: PlayerId) -> Option<Tile> {
        let hand = &self.game.players[player.0].hand;
        let mut full_hand = hand.clone();
        if let Some(drawn) = self.game.drawn_tile {
            full_hand.add(drawn);
        }

        let options = self.game.get_tenpai_discard_options(player);
        if options.is_empty() {
            return None;
        }

        let mut best_tile = options[0];
        let mut best_wait_count = 0usize;

        for &tile in &options {
            let mut simulated = full_hand.clone();
            simulated.remove(tile).ok();
            let waits = mahjong_yaku::analysis::analyze_wait_tiles(simulated.tiles());
            let wait_count = waits.len();
            if wait_count > best_wait_count {
                best_wait_count = wait_count;
                best_tile = tile;
            }
        }

        Some(best_tile)
    }

    pub fn handle_round_end(&mut self) {
        let reason = self.game.events.iter().rev().find_map(|e| {
            if let GameEvent::RoundEnded { reason } = e {
                Some(reason.clone())
            } else {
                None
            }
        });
        self.round_end_reason = reason.clone();

        if let Some(reason) = &reason {
            match reason {
                RoundEndReason::ExhaustiveDraw => {
                    self.messages.push("═══ 荒牌流局 ═══".to_string());
                    if let Some(GameEvent::ExhaustiveDrawResult { tenpai, payments }) =
                        self.game.events.iter().rev().find(|e| matches!(e, GameEvent::ExhaustiveDrawResult { .. }))
                    {
                        for i in 0..4 {
                            let name = self.player_name(i).to_string();
                            let status = if tenpai[i] { "听牌" } else { "不听" };
                            let pay = payments[i];
                            if pay > 0 {
                                self.messages.push(format!("  {} {} +{}", name, status, pay));
                            } else if pay < 0 {
                                self.messages.push(format!("  {} {} {}", name, status, pay));
                            } else {
                                self.messages.push(format!("  {} {}", name, status));
                            }
                        }
                    }
                }
                RoundEndReason::Win { winner, is_tsumo } => {
                    let name = self.player_name(winner.0).to_string();
                    let win_type = if *is_tsumo { "自摸" } else { "荣和" };
                    if let Some(GameEvent::PlayerWon { yaku_names, points, .. }) =
                        self.game.events.iter().rev().find(|e| matches!(e, GameEvent::PlayerWon { .. }))
                    {
                        self.messages.push(format!("═══ {} {} {}点 ═══", name, win_type, points.abs()));
                        for yaku in yaku_names {
                            self.messages.push(format!("  {}", yaku));
                        }
                    }
                }
                _ => {
                    self.messages.push(format!("═══ {:?} ═══", reason));
                }
            }

            let round = self.game.round;
            let dealer = self.game.get_dealer();
            let dealer_name = self.player_name(dealer.0).to_string();
            self.messages.push(format!(
                "下一局: 庄家 {} ({}局)",
                dealer_name,
                self.round_display_for(round)
            ));
        }

        if self.game.is_game_over() {
            self.show_result = true;
        } else {
            self.game.start_round(&mut self.rng);
            self.analysis = Self::compute_analysis(&mut self.calc, &self.game);
            let (acc, imp, shanten) = Self::compute_acceptance(&mut self.calc, &self.game);
            self.acceptance = acc;
            self.improvement = imp;
            self.acceptance_shanten = shanten;
            self.selected = 0;
        }
    }

    fn round_display_for(&self, round: u32) -> String {
        let wind = if round <= 4 { "东" } else { "南" };
        let num = ((round - 1) % 4) + 1;
        format!("{}{}局", wind, num)
    }

    pub fn execute_discard(&mut self, tile: Tile) {
        match self.game.execute_action(TurnAction::Discard(tile)) {
            Ok(_) => {
                self.messages.push(format!("你打出 {}", tile));
                self.refresh_analysis();
                self.process_ai_responses();
            }
            Err(e) => {
                self.messages.push(format!("错误: {}", e));
            }
        }
    }

    pub fn execute_tsumo(&mut self) {
        match self.game.execute_action(TurnAction::Tsumo) {
            Ok(events) => {
                for e in &events {
                    if let GameEvent::PlayerWon { yaku_names, points, .. } = e {
                        self.messages.push(format!("自摸！ {} 点", points.abs()));
                        for name in yaku_names {
                            self.messages.push(format!("  {}", name));
                        }
                    }
                }
                self.handle_round_end();
            }
            Err(e) => {
                self.messages.push(format!("错误: {}", e));
            }
        }
    }

    pub fn execute_riichi(&mut self) {
        let hand = &self.game.players[0].hand;
        if let Some(drawn) = self.game.drawn_tile {
            let mut full = hand.clone();
            full.add(drawn);
            let options = self.game.get_tenpai_discard_options(PlayerId(0));
            if let Some(&tile) = options.first() {
                match self.game.execute_action(TurnAction::RiichiDiscard(tile)) {
                    Ok(_) => {
                        self.messages.push(format!("立直！打出 {}", tile));
                        self.refresh_analysis();
                        self.process_ai_responses();
                    }
                    Err(e) => {
                        self.messages.push(format!("错误: {}", e));
                    }
                }
            } else {
                self.messages.push("无法立直：没有听牌打牌选项".to_string());
            }
        }
    }

    pub fn execute_call(&mut self, action: ResponseAction) {
        match self.game.execute_call(PlayerId(0), action) {
            Ok(_) => {
                self.messages.push("副露成功".to_string());
                self.call_options.clear();
                self.call_selected = 0;
            }
            Err(e) => {
                self.messages.push(format!("错误: {}", e));
            }
        }
    }

    pub fn pass_call(&mut self) {
        let _ = self.game.execute_call(PlayerId(0), ResponseAction::Pass);
        self.call_options.clear();
        self.call_selected = 0;
        self.process_ai_responses();
        if matches!(self.game.phase, GamePhase::DrawPhase) {
            if self.game.draw().is_err() {
                self.handle_round_end();
            }
            self.refresh_analysis();
        }
    }

    pub fn refresh_call_options(&mut self) {
        self.call_options = self.game.get_call_options();
        self.call_options.retain(|o| o.player == PlayerId(0));
        self.call_selected = 0;
    }

    pub fn refresh_analysis(&mut self) {
        self.analysis = Self::compute_analysis(&mut self.calc, &self.game);
        let (acc, imp, shanten) = Self::compute_acceptance(&mut self.calc, &self.game);
        self.acceptance = acc;
        self.improvement = imp;
        self.acceptance_shanten = shanten;
    }

    fn compute_analysis(calc: &mut ShantenCalculator, game: &GameState) -> Vec<DiscardOption> {
        let player = &game.players[0];
        let hand = &player.hand;
        let mut tiles: Vec<Tile> = hand.tiles().to_vec();
        if let Some(drawn) = game.drawn_tile {
            tiles.push(drawn);
        }
        if tiles.len() < 2 {
            return Vec::new();
        }
        let visible = Self::build_visible_tiles_static(game, PlayerId(0));
        analyze_discard(calc, &tiles, &visible)
    }

    fn compute_acceptance(calc: &mut ShantenCalculator, game: &GameState) -> (Vec<AcceptanceInfo>, Vec<AcceptanceInfo>, i8) {
        let player = &game.players[0];
        let hand = &player.hand;
        if hand.is_empty() {
            return (Vec::new(), Vec::new(), -1);
        }
        let visible = Self::build_visible_tiles_static(game, PlayerId(0));
        analyze_acceptance(calc, hand.tiles(), &visible)
    }

    fn build_visible_tiles(&self, player: PlayerId) -> VisibleTiles {
        Self::build_visible_tiles_static(&self.game, player)
    }

    fn build_visible_tiles_static(game: &GameState, player: PlayerId) -> VisibleTiles {
        let mut visible = VisibleTiles::new();
        for meld in &game.players[player.0].melds {
            for t in &meld.tiles {
                visible.hand_melds.inc(t.tile_type());
            }
        }
        for i in 0..4 {
            let pid = PlayerId(i);
            if pid == player { continue; }
            for meld in &game.players[i].melds {
                for t in &meld.tiles {
                    visible.all_melds.inc(t.tile_type());
                }
            }
        }
        for i in 0..4 {
            for &t in &game.players[i].discards {
                visible.all_discards.inc(t.tile_type());
            }
        }
        for &tt in &game.dora_indicators {
            visible.dora_indicators.inc(tt);
        }
        visible
    }

    pub fn player_name(&self, idx: usize) -> &str {
        match idx {
            0 => "你",
            1 => "AI-南",
            2 => "AI-西",
            3 => "AI-北",
            _ => "?",
        }
    }

    pub fn round_display(&self) -> String {
        let wind_str = match self.game.wind {
            TileType::EAST => "东",
            TileType::SOUTH => "南",
            _ => "?",
        };
        let round_in_wind = ((self.game.round) % 4) + 1;
        format!("{}{}局", wind_str, round_in_wind)
    }
}
