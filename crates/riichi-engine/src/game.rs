use std::collections::HashSet;

use rand::Rng;
use riichi_core::hand::Hand;
use riichi_core::meld::{Meld, MeldKind};
use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use riichi_core::wall::Wall;
use riichi_logic::analysis::{analyze_wait_tiles, is_standard_win};
use riichi_logic::shanten::ShantenCalculator;
use riichi_logic::types::{TileCounts, WinContext};
use riichi_logic::win_check;
use serde::{Deserialize, Serialize};

use crate::action::{CallOption, CallType, GameEvent, ResponseAction, RoundEndReason, TurnAction};
use crate::player::{wind_from_index, FuritenState, Player};

use riichi_core::game_types::GameError::{InvalidAction, WallExhausted};
pub use riichi_core::game_types::{extract_kuikae_tiles, GameError, GamePhase};

/// 游戏状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameState {
    /// 玩家列表
    pub players: [Player; 4],
    /// 当前的场风，是东场还是南场，一般只做半庄的实现，即游戏流程只有东一局→...→东四局→南一局→...→南四局（→西一局...如果启用了西入）
    pub wind: TileType,
    /// 当前为东X局/南X局
    pub round: u32,
    /// 当前为X本场
    pub honba: u32,
    /// 当前场上未被拿走的立直棒数量
    pub riichi_sticks: u32,
    /// 正在执行操作的玩家
    pub current_player: PlayerId,
    /// 自摸牌缓冲区：刚从牌山/岭上摸到、尚未进手的牌。
    /// 摸牌后存在于缓冲区中，手牌保持 3n+1 不变。
    /// 玩家行动时决定去向：打出（不进手）、自摸/暗杠/加杠（先提交到手牌再操作）。
    pub drawn_tile: Option<Tile>,
    /// 牌山
    pub wall: Wall,
    /// 宝牌集合，需要写两个函数，一个函数在每局游戏开始/大明杠/加杠/暗杠时调用，向vec里面push一个新宝牌牌类别，操作是从宝牌指示牌的第一张开始，如果指示牌的牌面数字是x，那么宝牌就是x的循环自增，例如指示牌是7m时，宝牌为8m，如果指示牌是9p，那么宝牌是1p，如果指示牌是东，那么宝牌是南，如果指示牌是白，那么宝牌是发。
    pub dora: Vec<TileType>,
    /// 宝牌指示牌列表
    pub dora_indicators: Vec<TileType>,
    /// 里宝牌指示牌列表
    pub ura_dora_indicators: Vec<TileType>,
    /// 游戏事件
    pub events: Vec<GameEvent>,
    /// 游戏阶段
    pub phase: GamePhase,
}

// ═══════════════════════════════════════════════════════════════
//  初始化 & 玩家访问
// ═══════════════════════════════════════════════════════════════

impl GameState {
    pub fn new() -> Self {
        Self {
            players: [
                Player::new(PlayerId(0), wind_from_index(0)),
                Player::new(PlayerId(1), wind_from_index(1)),
                Player::new(PlayerId(2), wind_from_index(2)),
                Player::new(PlayerId(3), wind_from_index(3)),
            ],
            current_player: PlayerId(0),
            wind: TileType::EAST,
            events: Vec::new(),
            phase: GamePhase::ActionPhase,
            drawn_tile: None,
            round: 0,
            honba: 0,
            riichi_sticks: 0,
            wall: Wall::empty(),
            dora: Vec::new(),
            dora_indicators: Vec::new(),
            ura_dora_indicators: Vec::new(),
        }
    }

    /// 庄家（自风为东风的玩家），从 round 推算
    pub fn get_dealer(&self) -> PlayerId {
        PlayerId((self.round.saturating_sub(1) as usize) % 4)
    }

    /// 统计杠的总数，海底牌指针偏移量
    pub fn get_kan_count(&self) -> usize {
        self.players
            .iter()
            .map(|player| {
                player
                    .melds
                    .iter()
                    .filter(|meld| {
                        meld.kind == MeldKind::Ankan
                            || meld.kind == MeldKind::Kakan
                            || meld.kind == MeldKind::Minkan
                    })
                    .count()
            })
            .sum()
    }

    /// 宝牌指示牌 → 宝牌（循环自增）
    fn dora_from_indicator(indicator: TileType) -> TileType {
        if indicator.is_number() {
            let rank = indicator.rank().0;
            if rank < 9 {
                TileType(indicator.0 + 1)
            } else {
                TileType(indicator.0 - 8) // 9 → 1
            }
        } else {
            // 字牌：风牌 27-30 循环，三元牌 31-33 循环
            let base = if indicator.is_wind() { 27 } else { 31 };
            let size = if indicator.is_wind() { 4 } else { 3 };
            TileType(base + (indicator.0 - base + 1) % size)
        }
    }

    /// 杠后翻新的宝牌指示牌
    fn reveal_dora_indicator(&mut self) {
        let kan_count = self.get_kan_count();
        if kan_count > 0 && kan_count <= 5 && self.dora.len() < 5 {
            let indicator = self.wall.dora_indicator(kan_count).tile_type();
            self.dora_indicators.push(indicator);
            self.dora.push(Self::dora_from_indicator(indicator));
            // 里宝牌指示牌
            self.ura_dora_indicators
                .push(self.wall.ura_dora_indicator(kan_count).tile_type());
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  回合流程：配牌 → 摸牌 → 打牌 → 响应 → 循环
// ═══════════════════════════════════════════════════════════════

impl GameState {
    // ─── 配牌 ──────────────────────────────────────────────

    pub fn start_round(&mut self, rng: &mut impl Rng) {
        self.wall = Wall::new(rng);
        self.drawn_tile = None;
        self.dora.clear();
        self.dora_indicators.clear();
        self.ura_dora_indicators.clear();
        // 翻第一张宝牌指示牌
        let indicator = self.wall.dora_indicator(0).tile_type();
        self.dora_indicators.push(indicator);
        self.dora.push(Self::dora_from_indicator(indicator));
        // 里宝牌指示牌
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
        // 庄家摸第14张牌 → 进入自摸牌缓冲区（不进手）
        self.current_player = self.get_dealer();
        let tile = self.wall.draw().unwrap();
        self.drawn_tile = Some(tile);
        self.phase = GamePhase::ActionPhase;

        self.events.push(GameEvent::RoundStarted {
            round_number: self.round,
            dealer: self.get_dealer(),
        });
    }

    // ─── 摸牌 ──────────────────────────────────────────────

    /// 从牌山摸一张牌，进入行动阶段。牌山耗尽时自动荒牌流局。
    /// 摸到的牌进入自摸牌缓冲区（drawn_tile），不进手牌。
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

    /// 岭上补摸，进入行动阶段。摸到的牌进入自摸牌缓冲区。
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

    // ─── 自摸牌缓冲区操作 ──────────────────────────────────────

    /// 将自摸牌从缓冲区提交到手牌。
    /// 仅在需要操作手牌时调用（打手牌、自摸、暗杠、加杠）。
    fn insert_tile(&mut self) {
        if let Some(tile) = self.drawn_tile.take() {
            self.players[self.current_player.0].hand.add(tile);
        }
    }

    // ─── 打牌 ──────────────────────────────────────────────

    pub fn discard(&mut self, tile: Tile) -> Result<(), GameError> {
        let cp = self.current_player.0;

        // 食替检查
        if self.players[cp].forbidden.contains(&tile.tile_type()) {
            return Err(GameError::InvalidAction(format!(
                "食替：{} 不能立刻打出",
                tile
            )));
        }

        // 立直后只能打出摸到的牌
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
            // ── 打出自摸牌：直接从缓冲区消耗，不进手 ──
            self.drawn_tile = None;
        } else {
            // ── 打出手牌：先提交自摸牌到手牌，再从手牌移除 ──
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

        // 不落河！牌暂存在 ResponsePhase 的 discarded_tile 中，
        // 等所有人响应完毕后才决定去向（落河 / 被吃碰杠荣和）。

        // 立直宣言牌：如果是立直后的第一次打牌，记录宣言牌
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

        // 进入响应阶段（牌暂存在这里，不在 discards 中）
        self.phase = GamePhase::ResponsePhase {
            discarded_tile: tile,
            discarder: self.current_player,
        };

        Ok(())
    }

    // ─── 行动（自摸/立直/暗杠/加杠/打牌）─────────────────────────

    /// 执行行动阶段的操作
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
                // 立直条件检查（此时 hand=13, drawn_tile=Some）
                if !self.can_declare_riichi(self.current_player) {
                    return Err(GameError::InvalidAction("不满足立直条件".to_string()));
                }
                // 提交自摸牌到手牌（hand 13→14），以便做听牌检查
                self.insert_tile();
                // 检查牌在手中
                if !self.players[self.current_player.0].hand.contains(tile) {
                    return Err(GameError::TileNotInHand(tile));
                }
                // 检查打出后是否听牌（hand 有 14 张，打一张剩 13 张）
                let mut simulated = self.players[self.current_player.0].hand.clone();
                simulated
                    .remove(tile)
                    .map_err(|_| GameError::TileNotInHand(tile))?;
                if analyze_wait_tiles(simulated.tiles()).is_empty() {
                    return Err(GameError::InvalidAction(
                        "立直宣言牌必须使手牌听牌".to_string(),
                    ));
                }
                // 宣告立直（合并为单次可变借用）
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
                // 打出宣言牌
                self.discard(tile)?;
                self.players[self.current_player.0].has_made_first_action = true;

                // 四风连打检查（立直宣言牌也参与判定）
                if matches!(self.phase, GamePhase::ResponsePhase { .. })
                    && self.check_suufon_renda()
                {
                    self.resolve_round_end(RoundEndReason::SuufonRenda);
                }
                // 四家立直检查（第四家立直宣言后，且未被荣和取消）
                else if matches!(self.phase, GamePhase::ResponsePhase { .. })
                    && self.check_suucha_riichi()
                {
                    self.resolve_round_end(RoundEndReason::SuuchaRiichi);
                }
            }

            TurnAction::Tsumo => {
                // 先取出自摸牌（commit 会清空 drawn_tile）
                let winning_tile = self.drawn_tile.ok_or_else(|| {
                    GameError::InvalidAction("没有摸到的牌，无法自摸".to_string())
                })?;
                // 不要先 insert_tile，让 check_win 自己构建 tiles
                let result = self.check_win(self.current_player, true, winning_tile, None, false);
                if let Some((changes, yaku_names)) = result {
                    // 自摸成立，提交自摸牌到手牌
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
                // 提交自摸牌到手牌（暗杠需 4 张在手）
                self.insert_tile();
                let events = self.execute_ankan(self.current_player, tile)?;
                new_events.extend(events);
            }

            TurnAction::Kakan(meld_index, tile) => {
                // 提交自摸牌到手牌（加杠需手牌中有第 4 张）
                self.insert_tile();
                let events = self.execute_kakan(self.current_player, meld_index, tile)?;
                new_events.extend(events);
            }
        }

        self.events.extend(new_events.clone());
        Ok(new_events)
    }

    // ─── 响应（吃/碰/杠/荣和/过）─────────────────────────────

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
                // 抢杠荣和：仅检测荣和，不检测吃/碰/杠
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

    /// 处理普通响应阶段（吃/碰/杠/荣和）
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
                // 牌直接从 ResponsePhase 取走，无需 pop
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
                // 牌直接从 ResponsePhase 取走，无需 pop
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

    /// 处理抢杠荣和响应阶段（仅荣和/过）
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
                // 所有人 Pass → 杠成立，摸岭上牌，进入行动阶段
                self.current_player = kakan_player;
                self.draw_rinshan()?;

                if self.check_four_kan_abort() {
                    self.resolve_round_end(RoundEndReason::SuuKantsu);
                } else {
                    self.phase = GamePhase::ActionPhase;
                }
            }
            ResponseAction::Ron => {
                // 抢杠荣和：此杠不成立，副露恢复为碰
                {
                    let meld = &mut self.players[kakan_player.0].melds[meld_index];
                    debug_assert!(meld.kind == MeldKind::Kakan);
                    // 移除第 4 张牌（加杠的那张），恢复为碰
                    meld.tiles.pop();
                    meld.kind = MeldKind::Pon;
                }

                self.clear_ippatsu();
                // 不要先加牌到手牌，让 check_win 自己构建 tiles

                let result = self.check_win(player, false, kakan_tile, Some(kakan_player), true);
                if let Some((changes, yaku_names)) = result {
                    // 抢杠荣和成立，将牌加入手牌
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

    // ─── 杠（暗杠/加杠）─────────────────────────────────────

    /// 检测当前玩家可执行的暗杠选项。
    /// 考虑手牌（3n+1）与自摸牌缓冲区中的牌。
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
        // 自摸牌可能与手牌 3 张组合成暗杠（3+1=4）
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

    /// 执行暗杠：移除手中 4 张相同牌，创建暗杠副露，岭上补摸
    pub fn execute_ankan(
        &mut self,
        player: PlayerId,
        tile: Tile,
    ) -> Result<Vec<GameEvent>, GameError> {
        let tt = tile.tile_type();
        if self.players[player.0].hand.count_type(tt.0) < 4 {
            return Err(GameError::InvalidAction("手中没有 4 张相同牌".to_string()));
        }

        // 四杠限制
        if !self.can_declare_kan(player) {
            return Err(GameError::InvalidAction(
                "四杠限制：不能继续开杠".to_string(),
            ));
        }

        // 立直后暗杠限制
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

    /// 检测当前玩家可执行的加杠选项。
    /// 考虑手牌（3n+1）与自摸牌缓冲区中的牌。
    pub fn get_kakan_options(&self, player: PlayerId) -> Vec<(usize, Tile)> {
        let p = &self.players[player.0];
        let mut options = Vec::new();
        for (i, meld) in p.melds.iter().enumerate() {
            if meld.kind == riichi_core::meld::MeldKind::Pon {
                let tt = meld.tiles[0].tile_type();
                if let Some(&tile) = p.hand.tiles().iter().find(|t| t.tile_type() == tt) {
                    options.push((i, tile));
                }
                // 自摸牌也可能匹配碰副露
                if let Some(drawn) = self.drawn_tile {
                    if drawn.tile_type() == tt {
                        options.push((i, drawn));
                    }
                }
            }
        }
        options
    }

    /// 执行加杠：将碰副露升级为加杠，进入抢杠荣和响应阶段
    pub fn execute_kakan(
        &mut self,
        player: PlayerId,
        meld_index: usize,
        tile: Tile,
    ) -> Result<Vec<GameEvent>, GameError> {
        // 验证阶段：只读访问
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

        // 四杠限制
        if !self.can_declare_kan(player) {
            return Err(GameError::InvalidAction(
                "四杠限制：不能继续开杠".to_string(),
            ));
        }

        // 执行阶段：一次可变借用完成所有操作
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

        // 进入抢杠荣和响应阶段（不立即摸岭上牌）
        self.phase = GamePhase::ChankanResponse {
            kakan_tile: tile,
            kakan_player: player,
            meld_index,
        };

        self.events.extend(new_events.clone());
        Ok(new_events)
    }
}

// ═══════════════════════════════════════════════════════════════
//  立直 & 听牌
// ═══════════════════════════════════════════════════════════════

impl GameState {
    /// 检测玩家是否听牌（考虑 drawn_tile 缓冲区）
    pub fn is_tenpai(&self, player: PlayerId) -> bool {
        let calc = ShantenCalculator::new();
        let hand = &self.players[player.0].hand;
        let mut counts = riichi_logic::types::TileCounts::from_tiles(hand.tiles());
        if let Some(drawn) = self.drawn_tile {
            if player == self.current_player {
                counts.inc(drawn.tile_type());
            }
        }
        calc.lookup(&counts) == 0
    }

    /// 获取玩家可以导致听牌的打牌列表（手牌 14 张时调用，含 drawn_tile）
    pub fn get_tenpai_discard_options(&self, player: PlayerId) -> Vec<Tile> {
        let calc = ShantenCalculator::new();
        let hand = &self.players[player.0].hand;
        let mut full_tiles: Vec<Tile> = hand.tiles().to_vec();
        if let Some(drawn) = self.drawn_tile {
            full_tiles.push(drawn);
        }
        let full_counts = riichi_logic::types::TileCounts::from_tiles(&full_tiles);
        let mut options = Vec::new();
        for &tile in &full_tiles {
            let mut after = full_counts;
            after.dec(tile.tile_type());
            if calc.lookup(&after) == 0 {
                options.push(tile);
            }
        }
        options
    }

    /// 检测玩家是否可以听牌（手牌 14 张时调用）
    pub fn can_tenpai(&self, player: PlayerId) -> bool {
        let calc = ShantenCalculator::new();
        let hand = &self.players[player.0].hand;
        for &tile in hand.tiles() {
            let mut after = riichi_logic::types::TileCounts::from_tiles(hand.tiles());
            after.dec(tile.tile_type());
            if calc.lookup(&after) == 0 {
                return true;
            }
        }
        false
    }

    /// 获取玩家听牌列表（手牌 13 张时调用）
    pub fn get_waiting_tiles(&self, player: PlayerId) -> Vec<TileType> {
        analyze_wait_tiles(self.players[player.0].hand.tiles())
            .iter()
            .map(|w| w.tile_type)
            .collect()
    }

    /// 检测玩家是否可以宣告立直
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
        self.is_tenpai(player)
    }

    /// 宣告立直：扣 1000 点，设置立直标记
    pub fn execute_riichi(&mut self, player: PlayerId) -> Result<(), GameError> {
        if !self.can_declare_riichi(player) {
            return Err(GameError::InvalidAction("不满足立直条件".to_string()));
        }

        {
            let p = &mut self.players[player.0];
            p.points -= 1000;
            p.is_riichi = true;
        }
        self.riichi_sticks += 1;

        self.events.push(GameEvent::PlayerDeclaredRiichi { player });
        Ok(())
    }

    /// 立直后可用的暗杠选项
    ///
    /// 立直后暗杠必须满足：
    /// 1. 暗杠的 4 张牌包含摸到的牌
    /// 2. 暗杠后听牌种类不变
    ///
    /// 实现：比较暗杠前（13张手牌）的听牌与暗杠后（10张手牌）的听牌。
    /// 暗杠后的听牌通过逐一尝试添加每种牌型、检查是否构成和了形来计算。
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

        // 必须手牌 3 张 + drawn_tile 1 张 = 4 张
        if hand_count != 3 {
            return vec![];
        }

        // waits_before：13 张手牌的听牌
        let waits_before: std::collections::HashSet<TileType> = analyze_wait_tiles(hand.tiles())
            .iter()
            .map(|w| w.tile_type)
            .collect();

        if waits_before.is_empty() {
            return vec![];
        }

        // 模拟暗杠后的 10 张手牌
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

        // waits_after：逐一尝试添加每种牌型，检查是否构成和了形。
        // 注意：10 张 + 1 张 = 11 张 = 雀头(2) + 面子(3×3)。
        // is_standard_win 会自动从总张数推算面子数。
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

// ═══════════════════════════════════════════════════════════════
//  和了判定 & 计分
// ═══════════════════════════════════════════════════════════════

impl GameState {
    /// 检查自摸和：模拟 hand + drawn_tile 合并后的 14 张手牌进行判定。
    /// drawn_tile 不会被消耗（此函数为只读检查）。
    pub fn check_tsumo(&self, player: PlayerId) -> Option<([i32; 4], Vec<String>)> {
        let winning_tile = self.drawn_tile?;
        // 传入原始手牌（不含摸到的牌），winning_tile 单独传递
        let hand = &self.players[player.0].hand;
        self.check_win_with_hand(player, true, winning_tile, None, hand, false)
    }

    /// 构建和了评估上下文
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
            is_rinshan: false, // 由调用方设置
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

    /// 检查和了（从玩家手牌读取）
    fn check_win(
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

    /// 检查和了（使用指定手牌，支持模拟 hand + drawn_tile）
    ///
    /// 支持三种和了形态：标准形、七对子、国士无双。
    fn check_win_with_hand(
        &self,
        player: PlayerId,
        is_tsumo: bool,
        winning_tile: Tile,
        loser: Option<PlayerId>,
        hand: &Hand,
        is_chankan: bool,
    ) -> Option<([i32; 4], Vec<String>)> {
        // 构建 all_tiles = 手牌 + 副露 + 和了牌（实体牌，用于宝牌/赤宝牌计算）
        let mut all_tiles: Vec<Tile> = hand.tiles().to_vec();
        for meld in &self.players[player.0].melds {
            all_tiles.extend_from_slice(&meld.tiles);
        }
        all_tiles.push(winning_tile);

        // 门清部分 TileType（手牌 + 和了牌，用于判形和拆解）
        let mut hand_tile_types: Vec<TileType> =
            hand.tiles().iter().map(|t| t.tile_type()).collect();
        if !hand_tile_types.contains(&winning_tile.tile_type()) {
            hand_tile_types.push(winning_tile.tile_type());
        } else {
            // tsumo: winning_tile already in hand, add one more copy for 14-tile check
            hand_tile_types.push(winning_tile.tile_type());
        }

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

// ═══════════════════════════════════════════════════════════════
//  回合管理
// ═══════════════════════════════════════════════════════════════

impl GameState {
    pub fn advance_turn(&mut self) {
        self.current_player = self.current_player.next();
    }

    /// 判断指定的牌是否来自岭上（wall[132..=135]）
    pub fn is_rinshan_tile(&self, tile: Tile) -> bool {
        self.wall.is_rinshan_tile(tile)
    }

    /// 清除所有玩家的一发状态
    pub fn clear_ippatsu(&mut self) {
        for player in &mut self.players {
            player.is_ippatsu = false;
        }
    }

    fn get_waiting_tile_types(&self, player: PlayerId) -> HashSet<TileType> {
        analyze_wait_tiles(self.players[player.0].hand.tiles())
            .iter()
            .map(|w| w.tile_type)
            .collect()
    }

    fn update_discard_furiten(&mut self, player: PlayerId) {
        let waiting = self.get_waiting_tile_types(player);
        let discarded = &self.players[player.0].all_discarded_types;
        self.players[player.0].furiten.discard = waiting.iter().any(|tt| discarded.contains(tt));
    }

    fn update_all_discard_furiten(&mut self) {
        for idx in 0..4 {
            self.update_discard_furiten(PlayerId(idx));
        }
    }

    pub fn is_round_over(&self) -> bool {
        matches!(self.phase, GamePhase::RoundOver) || self.remaining_tiles() == 0
    }

    /// 正常摸牌区剩余可摸牌数
    pub fn remaining_tiles(&self) -> usize {
        self.wall.remaining()
    }

    pub fn take_events(&mut self) -> Vec<GameEvent> {
        std::mem::take(&mut self.events)
    }

    /// 构建指定玩家视角的 VisibleTiles（用于向听/进张分析）
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

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════
//  事件查询辅助方法
// ═══════════════════════════════════════════════════════════════

impl GameState {
    /// 本局是否有过吃/碰/杠（立直不算）
    fn any_call_made(&self) -> bool {
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

    /// 当前是否仍在第一巡（每人最多打出过一张牌）
    fn is_first_turn(&self) -> bool {
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

    /// 获取第一巡中每位玩家打出的第一张牌类型
    fn first_turn_discards(&self) -> Vec<TileType> {
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

    /// 当前已立直的玩家数
    fn riichi_count(&self) -> u8 {
        self.events
            .iter()
            .filter(|e| matches!(e, GameEvent::PlayerDeclaredRiichi { .. }))
            .count() as u8
    }
}

// ═══════════════════════════════════════════════════════════════
//  流局检测
// ═══════════════════════════════════════════════════════════════

impl GameState {
    /// 九种九牌判定：第一巡 + 无吃碰杠 + 手牌+自摸牌中幺九牌种类 ≥ 9
    pub fn can_declare_kyuushu(&self, player: PlayerId) -> bool {
        if self.any_call_made() {
            return false;
        }
        if !self.is_first_turn() {
            return false;
        }
        let hand = &self.players[player.0].hand;
        // 手牌 13 张 + 自摸牌缓冲区 1 张 = 14 张
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

    /// 检查是否触发四风连打（在第四张打出牌后调用）
    fn check_suufon_renda(&self) -> bool {
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

    /// 检查是否触发四家立直（在每次立直宣言后调用）
    fn check_suucha_riichi(&self) -> bool {
        self.riichi_count() >= 4
    }

    /// 判断当前玩家是否可以开杠（暗杠/加杠/大明杠通用）
    ///
    /// 当总杠数 ≥ 4 时：
    /// - 一人四杠 → 只有该玩家可继续（保留四杠子机会）
    /// - 多人四杠 → 已流局，此处不应再被调用
    pub fn can_declare_kan(&self, player: PlayerId) -> bool {
        let total = self.get_kan_count();
        if total < 4 {
            return true;
        }
        // total == 4: 只有拥有全部四杠的玩家可继续
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

    /// 四杠散了检查（在杠操作完成后调用）
    ///
    /// - 总杠 ≥ 4 且由 ≥ 2 名玩家开出 → 流局
    /// - 总杠 ≥ 4 且由 1 名玩家开出 → 不流局
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

// ═══════════════════════════════════════════════════════════════
//  局结束处理 & 连庄/过庄
// ═══════════════════════════════════════════════════════════════

impl GameState {
    /// 荒牌流局结算：计算不听罚符，更新点棒
    pub fn resolve_exhaustive_draw(&mut self) {
        let tenpai: [bool; 4] = [
            self.is_tenpai(PlayerId(0)),
            self.is_tenpai(PlayerId(1)),
            self.is_tenpai(PlayerId(2)),
            self.is_tenpai(PlayerId(3)),
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

    /// 根据局结束原因处理连庄/过庄，更新 round、honba、场风
    ///
    /// - 和了：和牌者是庄家 → 连庄
    /// - 荒牌流局：庄家听牌 → 连庄
    /// - 途中流局（九种九牌/四风连打/四家立直/四杠散了）：一律连庄
    /// - 过庄：round += 1, honba = 0
    /// - 连庄：round 不变, honba += 1
    pub fn advance_round(&mut self, reason: &RoundEndReason) {
        let dealer_continues = match reason {
            RoundEndReason::Win { winner, .. } => *winner == self.get_dealer(),
            RoundEndReason::ExhaustiveDraw => self.is_tenpai(self.get_dealer()),
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
        self.round > 8
    }

    /// 统一处理局结束：荒牌罚符 + 连庄/过庄 + 设置 RoundOver
    pub fn resolve_round_end(&mut self, reason: RoundEndReason) {
        // 荒牌流局需要先结算罚符
        if matches!(reason, RoundEndReason::ExhaustiveDraw) {
            self.resolve_exhaustive_draw();
        }

        self.advance_round(&reason);

        self.events.push(GameEvent::RoundEnded { reason });
        self.phase = GamePhase::RoundOver;
    }
}

// ═══════════════════════════════════════════════════════════════
//  测试
// ═══════════════════════════════════════════════════════════════
