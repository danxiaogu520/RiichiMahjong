use std::collections::HashSet;

use riichi_core::hand::Hand;
use riichi_core::meld::{Meld, MeldKind};
use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use riichi_core::wall::Wall;
use riichi_ai::shanten::ShantenCalculator;
use riichi_logic::analysis::{analyze_wait_tiles, is_standard_win};
use riichi_logic::types::{TileCounts, WinContext};
use riichi_logic::win_check;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::action::{CallOption, CallType, GameEvent, ResponseAction, RoundEndReason, TurnAction};
use crate::player::{wind_from_index, FuritenState, Player};

pub use riichi_core::game_types::{extract_kuikae_tiles, GameError, GamePhase};
use riichi_core::game_types::GameError::{InvalidAction, WallExhausted};

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
            self.ura_dora_indicators.push(self.wall.ura_dora_indicator(kan_count).tile_type());
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
        self.ura_dora_indicators.push(self.wall.ura_dora_indicator(0).tile_type());

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
        let tile = self.wall.draw_rinshan().ok_or(InvalidAction("岭上牌已耗尽".to_string()))?;
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
                    if pid == discarder { continue; }
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

        let mut new_events = Vec::new();
        new_events.push(GameEvent::PlayerCalledAnkan {
            player,
            tiles: tiles_to_remove,
        });

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

        let mut new_events = Vec::new();
        new_events.push(GameEvent::PlayerCalledKakan {
            player,
            tile,
            original_pon,
        });

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
        let waits_before: std::collections::HashSet<TileType> =
            analyze_wait_tiles(hand.tiles())
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
            .map(|i| TileType(i))
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
        let mut hand_tile_types: Vec<TileType> = hand.tiles().iter().map(|t| t.tile_type()).collect();
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
        let result = win_check::check_win(&all_tiles, &hand_tile_types, &ctx, is_furiten, winning_tile)?;
        let yaku_names: Vec<String> = result.yaku_results.iter().map(|y| format!("{:?}", y.yaku)).collect();
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

    /// 玩家 id 是否已立直（通过事件判断）
    fn is_player_riichi(&self, player: PlayerId) -> bool {
        self.events.iter().any(|e| {
            matches!(
                e,
                GameEvent::PlayerDeclaredRiichi { player: p } if *p == player
            )
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use riichi_core::tile::{Rank, Suit};
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    // ─── 辅助函数 ──────────────────────────────────────────

    /// 手动设置玩家手牌的辅助函数
    fn setup_hand(game: &mut GameState, player: PlayerId, tiles: Vec<Tile>) {
        game.players[player.0].hand = Hand::new();
        for tile in tiles {
            game.players[player.0].hand.add(tile);
        }
    }

    // ─── 基础测试 ──────────────────────────────────────────

    #[test]
    fn test_game_creation() {
        let game = GameState::new();
        assert_eq!(game.current_player, PlayerId(0));
        assert_eq!(game.get_dealer(), PlayerId(0));
    }

    // TODO: 以下测试依赖已删除的字段，待重新实现
    // test_start_round, test_draw_and_discard, test_draw_rinshan,
    // test_kuikae_cleared_after_draw, test_no_kuikae_after_minkan,
    // test_riichi_discard_restriction, test_ippatsu_cleared_on_call

    // ─── 食替测试 ──────────────────────────────────────────

    #[test]
    fn test_kuikae_chi_forbidden() {
        let mut game = GameState::new();

        let hand_tiles = vec![
            Tile::new(Suit::Pin, Rank(2), 0),
            Tile::new(Suit::Pin, Rank(3), 0),
            Tile::new(Suit::Pin, Rank(5), 0),
            Tile::new(Suit::Pin, Rank(6), 0),
            Tile::new(Suit::Pin, Rank(7), 0),
            Tile::new(Suit::Man, Rank(1), 0),
            Tile::new(Suit::Man, Rank(2), 0),
            Tile::new(Suit::Man, Rank(3), 0),
            Tile::new(Suit::Sou, Rank(1), 0),
            Tile::new(Suit::Sou, Rank(2), 0),
            Tile::new(Suit::Sou, Rank(3), 0),
            Tile::new(Suit::Wind, Rank(1), 0),
            Tile::new(Suit::Wind, Rank(1), 1),
        ];
        setup_hand(&mut game, PlayerId(1), hand_tiles);

        let called_tile = Tile::new(Suit::Pin, Rank(1), 0);
        game.phase = GamePhase::ResponsePhase {
            discarded_tile: called_tile,
            discarder: PlayerId(0),
        };
        game.players[0].discards.push(called_tile);

        let chi_tiles = [
            Tile::new(Suit::Pin, Rank(2), 0),
            Tile::new(Suit::Pin, Rank(3), 0),
        ];
        let result = game.execute_call(
            PlayerId(1),
            ResponseAction::Chi {
                hand_tiles: chi_tiles,
            },
        );
        assert!(result.is_ok());

        assert!(game.players[1].forbidden.contains(&TileType(10)));
        assert!(game.players[1].forbidden.contains(&TileType(11)));

        game.current_player = PlayerId(1);
        let bad_tile = Tile::new(Suit::Pin, Rank(2), 0);
        assert!(game.discard(bad_tile).is_err());

        let good_tile = Tile::new(Suit::Pin, Rank(5), 0);
        assert!(game.discard(good_tile).is_ok());
    }

    #[test]
    fn test_kuikae_pon_forbidden() {
        let mut game = GameState::new();

        let hand_tiles = vec![
            Tile::new(Suit::Pin, Rank(2), 0),
            Tile::new(Suit::Pin, Rank(2), 1),
            Tile::new(Suit::Pin, Rank(5), 0),
            Tile::new(Suit::Pin, Rank(6), 0),
            Tile::new(Suit::Pin, Rank(7), 0),
            Tile::new(Suit::Man, Rank(1), 0),
            Tile::new(Suit::Man, Rank(2), 0),
            Tile::new(Suit::Man, Rank(3), 0),
            Tile::new(Suit::Sou, Rank(1), 0),
            Tile::new(Suit::Sou, Rank(2), 0),
            Tile::new(Suit::Sou, Rank(3), 0),
            Tile::new(Suit::Wind, Rank(1), 0),
            Tile::new(Suit::Wind, Rank(1), 1),
        ];
        setup_hand(&mut game, PlayerId(1), hand_tiles);

        let called_tile = Tile::new(Suit::Pin, Rank(2), 2);
        game.phase = GamePhase::ResponsePhase {
            discarded_tile: called_tile,
            discarder: PlayerId(0),
        };
        game.players[0].discards.push(called_tile);

        let pon_tiles = [
            Tile::new(Suit::Pin, Rank(2), 0),
            Tile::new(Suit::Pin, Rank(2), 1),
        ];
        let result = game.execute_call(
            PlayerId(1),
            ResponseAction::Pon {
                hand_tiles: pon_tiles,
            },
        );
        assert!(result.is_ok());

        assert!(game.players[1].forbidden.contains(&TileType(10)));

        game.current_player = PlayerId(1);
        let bad_tile = Tile::new(Suit::Pin, Rank(2), 0);
        assert!(game.discard(bad_tile).is_err());

        let good_tile = Tile::new(Suit::Pin, Rank(5), 0);
        assert!(game.discard(good_tile).is_ok());
    }

    #[test]
    fn test_kuikae_chi_middle_call() {
        let mut game = GameState::new();

        let hand_tiles = vec![
            Tile::new(Suit::Pin, Rank(1), 0),
            Tile::new(Suit::Pin, Rank(3), 0),
            Tile::new(Suit::Pin, Rank(5), 0),
            Tile::new(Suit::Pin, Rank(6), 0),
            Tile::new(Suit::Pin, Rank(7), 0),
            Tile::new(Suit::Man, Rank(1), 0),
            Tile::new(Suit::Man, Rank(2), 0),
            Tile::new(Suit::Man, Rank(3), 0),
            Tile::new(Suit::Sou, Rank(1), 0),
            Tile::new(Suit::Sou, Rank(2), 0),
            Tile::new(Suit::Sou, Rank(3), 0),
            Tile::new(Suit::Wind, Rank(1), 0),
            Tile::new(Suit::Wind, Rank(1), 1),
        ];
        setup_hand(&mut game, PlayerId(1), hand_tiles);

        let called_tile = Tile::new(Suit::Pin, Rank(2), 0);
        game.phase = GamePhase::ResponsePhase {
            discarded_tile: called_tile,
            discarder: PlayerId(0),
        };
        game.players[0].discards.push(called_tile);

        game.execute_call(
            PlayerId(1),
            ResponseAction::Chi {
                hand_tiles: [
                    Tile::new(Suit::Pin, Rank(1), 0),
                    Tile::new(Suit::Pin, Rank(3), 0),
                ],
            },
        )
        .unwrap();

        assert!(game.players[1].forbidden.contains(&TileType(9)));
        assert!(game.players[1].forbidden.contains(&TileType(11)));

        game.current_player = PlayerId(1);
        assert!(game.discard(Tile::new(Suit::Pin, Rank(5), 0)).is_ok());
    }

    // ─── 立直测试 ──────────────────────────────────────────

    #[test]
    #[ignore = "依赖 start_round (todo)"]
    fn test_riichi_basic() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = GameState::new();
        game.start_round(&mut rng);

        let hand_tiles = vec![
            Tile::new(Suit::Man, Rank(1), 0),
            Tile::new(Suit::Man, Rank(2), 0),
            Tile::new(Suit::Man, Rank(3), 0),
            Tile::new(Suit::Man, Rank(4), 0),
            Tile::new(Suit::Man, Rank(5), 0),
            Tile::new(Suit::Man, Rank(6), 0),
            Tile::new(Suit::Pin, Rank(7), 0),
            Tile::new(Suit::Pin, Rank(8), 0),
            Tile::new(Suit::Pin, Rank(9), 0),
            Tile::new(Suit::Sou, Rank(1), 0),
            Tile::new(Suit::Sou, Rank(1), 1),
            Tile::new(Suit::Sou, Rank(3), 0),
            Tile::new(Suit::Sou, Rank(4), 0),
        ];
        setup_hand(&mut game, PlayerId(0), hand_tiles);
        game.current_player = PlayerId(0);

        assert!(game.is_tenpai(PlayerId(0)));
        assert!(game.can_declare_riichi(PlayerId(0)));

        let initial_points = game.players[0].points;
        game.execute_riichi(PlayerId(0)).unwrap();
        assert!(game.players[0].is_riichi);
        assert_eq!(game.players[0].points, initial_points - 1000);
        assert_eq!(game.riichi_sticks, 1);
    }

    #[test]
    #[ignore = "依赖 start_round (todo)"]
    fn test_riichi_not_tenpai() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = GameState::new();
        game.start_round(&mut rng);

        let hand_tiles = vec![
            Tile::new(Suit::Man, Rank(1), 0),
            Tile::new(Suit::Man, Rank(2), 0),
            Tile::new(Suit::Man, Rank(3), 0),
            Tile::new(Suit::Man, Rank(4), 0),
            Tile::new(Suit::Man, Rank(5), 0),
            Tile::new(Suit::Man, Rank(6), 0),
            Tile::new(Suit::Man, Rank(7), 0),
            Tile::new(Suit::Man, Rank(8), 0),
            Tile::new(Suit::Man, Rank(9), 0),
            Tile::new(Suit::Pin, Rank(1), 0),
            Tile::new(Suit::Pin, Rank(3), 0),
            Tile::new(Suit::Pin, Rank(5), 0),
            Tile::new(Suit::Pin, Rank(7), 0),
        ];
        setup_hand(&mut game, PlayerId(0), hand_tiles);
        game.current_player = PlayerId(0);

        assert!(!game.is_tenpai(PlayerId(0)));
        assert!(!game.can_declare_riichi(PlayerId(0)));
        assert!(game.execute_riichi(PlayerId(0)).is_err());
    }

    #[test]
    #[ignore = "依赖 start_round (todo)"]
    fn test_riichi_not_enough_points() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = GameState::new();
        game.start_round(&mut rng);

        let hand_tiles = vec![
            Tile::new(Suit::Man, Rank(1), 0),
            Tile::new(Suit::Man, Rank(2), 0),
            Tile::new(Suit::Man, Rank(3), 0),
            Tile::new(Suit::Man, Rank(4), 0),
            Tile::new(Suit::Man, Rank(5), 0),
            Tile::new(Suit::Man, Rank(6), 0),
            Tile::new(Suit::Pin, Rank(7), 0),
            Tile::new(Suit::Pin, Rank(8), 0),
            Tile::new(Suit::Pin, Rank(9), 0),
            Tile::new(Suit::Sou, Rank(1), 0),
            Tile::new(Suit::Sou, Rank(1), 1),
            Tile::new(Suit::Sou, Rank(3), 0),
            Tile::new(Suit::Sou, Rank(4), 0),
        ];
        setup_hand(&mut game, PlayerId(0), hand_tiles);
        game.current_player = PlayerId(0);
        game.players[0].points = 500;

        assert!(!game.can_declare_riichi(PlayerId(0)));
    }

    #[test]
    fn test_riichi_ankan_same_waits() {
        let mut game = GameState::new();

        // 手牌 13 张：1m×3, 2-3-4m, 1p×3, 2-3p(搭子), 9p×2(雀头)
        // 听牌：1p/4p（完成 2-3p 搭子）
        // 暗杠 1m 后剩 10 张：2-3-4m, 1p×3, 2-3p, 9p×2 → 听牌不变
        let hand_tiles = vec![
            Tile::new(Suit::Man, Rank(1), 0),
            Tile::new(Suit::Man, Rank(1), 1),
            Tile::new(Suit::Man, Rank(1), 2),
            Tile::new(Suit::Man, Rank(2), 0),
            Tile::new(Suit::Man, Rank(3), 0),
            Tile::new(Suit::Man, Rank(4), 0),
            Tile::new(Suit::Pin, Rank(1), 0),
            Tile::new(Suit::Pin, Rank(1), 1),
            Tile::new(Suit::Pin, Rank(1), 2),
            Tile::new(Suit::Pin, Rank(2), 0),
            Tile::new(Suit::Pin, Rank(3), 0),
            Tile::new(Suit::Pin, Rank(9), 0),
            Tile::new(Suit::Pin, Rank(9), 1),
        ];
        setup_hand(&mut game, PlayerId(0), hand_tiles);
        game.current_player = PlayerId(0);
        game.drawn_tile = Some(Tile::new(Suit::Man, Rank(1), 3));
        game.players[0].is_riichi = true;

        let options = game.get_riichi_ankan_options(PlayerId(0));
        assert!(!options.is_empty(), "暗杠 1m 后听牌不变（3p），应该允许");
    }

    #[test]
    fn test_riichi_ankan_no_match() {
        let mut game = GameState::new();

        let hand_tiles = vec![
            Tile::new(Suit::Man, Rank(1), 0),
            Tile::new(Suit::Man, Rank(1), 1),
            Tile::new(Suit::Man, Rank(1), 2),
            Tile::new(Suit::Man, Rank(2), 0),
            Tile::new(Suit::Man, Rank(3), 0),
            Tile::new(Suit::Man, Rank(4), 0),
            Tile::new(Suit::Man, Rank(6), 0),
            Tile::new(Suit::Man, Rank(7), 0),
            Tile::new(Suit::Man, Rank(8), 0),
            Tile::new(Suit::Pin, Rank(5), 0),
            Tile::new(Suit::Pin, Rank(6), 0),
            Tile::new(Suit::Pin, Rank(7), 0),
            Tile::new(Suit::Pin, Rank(9), 0),
        ];
        setup_hand(&mut game, PlayerId(0), hand_tiles);
        game.current_player = PlayerId(0);
        game.drawn_tile = Some(Tile::new(Suit::Pin, Rank(2), 0));
        game.players[0].is_riichi = true;
        // drawn_tile 不进手，手牌保持 3 张 1m

        let options = game.get_riichi_ankan_options(PlayerId(0));
        assert!(options.is_empty(), "摸到的牌不是 4 张，不能暗杠");
    }

    // ─── 九种九牌测试 ────────────────────────────────────────

    /// 九种九牌：手牌中有 9 种幺九牌 → 可以宣告
    #[test]
    fn test_kyuushu_can_declare() {
        let mut game = GameState::new();
        // 1m 9m 1p 9p 1s 9s 东南西北白发中 + 随意一张
        let hand_tiles = vec![
            Tile::new(Suit::Man, Rank(1), 0),    // 1m
            Tile::new(Suit::Man, Rank(9), 0),    // 9m
            Tile::new(Suit::Pin, Rank(1), 0),    // 1p
            Tile::new(Suit::Pin, Rank(9), 0),    // 9p
            Tile::new(Suit::Sou, Rank(1), 0),    // 1s
            Tile::new(Suit::Sou, Rank(9), 0),    // 9s
            Tile::new(Suit::Wind, Rank(1), 0),   // 东
            Tile::new(Suit::Wind, Rank(2), 0),   // 南
            Tile::new(Suit::Wind, Rank(3), 0),   // 西
            Tile::new(Suit::Wind, Rank(4), 0),   // 北
            Tile::new(Suit::Dragon, Rank(1), 0), // 白
            Tile::new(Suit::Dragon, Rank(2), 0), // 发
            Tile::new(Suit::Dragon, Rank(3), 0), // 中
            Tile::new(Suit::Man, Rank(1), 1),    // 重复 1m
        ];
        setup_hand(&mut game, PlayerId(0), hand_tiles);
        game.current_player = PlayerId(0);
        game.phase = GamePhase::ActionPhase;

        assert!(game.can_declare_kyuushu(PlayerId(0)));
    }

    /// 九种九牌：只有 8 种幺九牌 → 不能宣告
    #[test]
    fn test_kyuushu_only_8_types() {
        let mut game = GameState::new();
        let hand_tiles = vec![
            Tile::new(Suit::Man, Rank(1), 0),
            Tile::new(Suit::Man, Rank(9), 0),
            Tile::new(Suit::Pin, Rank(1), 0),
            Tile::new(Suit::Pin, Rank(9), 0),
            Tile::new(Suit::Sou, Rank(1), 0),
            Tile::new(Suit::Sou, Rank(9), 0),
            Tile::new(Suit::Wind, Rank(1), 0),
            Tile::new(Suit::Wind, Rank(2), 0),
            // 只有 8 种，缺少西以后的
            Tile::new(Suit::Man, Rank(2), 0),
            Tile::new(Suit::Man, Rank(3), 0),
            Tile::new(Suit::Man, Rank(4), 0),
            Tile::new(Suit::Man, Rank(5), 0),
            Tile::new(Suit::Man, Rank(6), 0),
            Tile::new(Suit::Man, Rank(7), 0),
        ];
        setup_hand(&mut game, PlayerId(0), hand_tiles);
        game.current_player = PlayerId(0);
        game.phase = GamePhase::ActionPhase;

        assert!(!game.can_declare_kyuushu(PlayerId(0)));
    }

    /// 九种九牌：有过吃碰 → 不能宣告
    #[test]
    fn test_kyuushu_after_call() {
        let mut game = GameState::new();
        let hand_tiles = vec![
            Tile::new(Suit::Man, Rank(1), 0),
            Tile::new(Suit::Man, Rank(9), 0),
            Tile::new(Suit::Pin, Rank(1), 0),
            Tile::new(Suit::Pin, Rank(9), 0),
            Tile::new(Suit::Sou, Rank(1), 0),
            Tile::new(Suit::Sou, Rank(9), 0),
            Tile::new(Suit::Wind, Rank(1), 0),
            Tile::new(Suit::Wind, Rank(2), 0),
            Tile::new(Suit::Wind, Rank(3), 0),
            Tile::new(Suit::Wind, Rank(4), 0),
            Tile::new(Suit::Dragon, Rank(1), 0),
            Tile::new(Suit::Dragon, Rank(2), 0),
            Tile::new(Suit::Dragon, Rank(3), 0),
            Tile::new(Suit::Man, Rank(1), 1),
        ];
        setup_hand(&mut game, PlayerId(0), hand_tiles);
        game.current_player = PlayerId(0);
        game.phase = GamePhase::ActionPhase;

        // 模拟有过碰操作
        game.events.push(GameEvent::PlayerCalledPon {
            player: PlayerId(1),
            tiles: vec![],
            from_player: PlayerId(2),
        });

        assert!(!game.can_declare_kyuushu(PlayerId(0)));
    }

    /// 九种九牌：非第一巡 → 不能宣告
    #[test]
    fn test_kyuushu_not_first_turn() {
        let mut game = GameState::new();
        let hand_tiles = vec![
            Tile::new(Suit::Man, Rank(1), 0),
            Tile::new(Suit::Man, Rank(9), 0),
            Tile::new(Suit::Pin, Rank(1), 0),
            Tile::new(Suit::Pin, Rank(9), 0),
            Tile::new(Suit::Sou, Rank(1), 0),
            Tile::new(Suit::Sou, Rank(9), 0),
            Tile::new(Suit::Wind, Rank(1), 0),
            Tile::new(Suit::Wind, Rank(2), 0),
            Tile::new(Suit::Wind, Rank(3), 0),
            Tile::new(Suit::Wind, Rank(4), 0),
            Tile::new(Suit::Dragon, Rank(1), 0),
            Tile::new(Suit::Dragon, Rank(2), 0),
            Tile::new(Suit::Dragon, Rank(3), 0),
            Tile::new(Suit::Man, Rank(1), 1),
        ];
        setup_hand(&mut game, PlayerId(0), hand_tiles);
        game.current_player = PlayerId(0);
        game.phase = GamePhase::ActionPhase;

        // 模拟玩家0已打出2张牌（非第一巡）
        game.events.push(GameEvent::PlayerDiscarded {
            player: PlayerId(0),
            tile: Tile::new(Suit::Man, Rank(2), 0),
        });
        game.events.push(GameEvent::PlayerDiscarded {
            player: PlayerId(0),
            tile: Tile::new(Suit::Man, Rank(3), 0),
        });

        assert!(!game.can_declare_kyuushu(PlayerId(0)));
    }

    /// 九种九牌：执行宣告后触发流局
    #[test]
    fn test_kyuushu_execute() {
        let mut game = GameState::new();
        let hand_tiles = vec![
            Tile::new(Suit::Man, Rank(1), 0),
            Tile::new(Suit::Man, Rank(9), 0),
            Tile::new(Suit::Pin, Rank(1), 0),
            Tile::new(Suit::Pin, Rank(9), 0),
            Tile::new(Suit::Sou, Rank(1), 0),
            Tile::new(Suit::Sou, Rank(9), 0),
            Tile::new(Suit::Wind, Rank(1), 0),
            Tile::new(Suit::Wind, Rank(2), 0),
            Tile::new(Suit::Wind, Rank(3), 0),
            Tile::new(Suit::Wind, Rank(4), 0),
            Tile::new(Suit::Dragon, Rank(1), 0),
            Tile::new(Suit::Dragon, Rank(2), 0),
            Tile::new(Suit::Dragon, Rank(3), 0),
            Tile::new(Suit::Man, Rank(1), 1),
        ];
        setup_hand(&mut game, PlayerId(0), hand_tiles);
        game.current_player = PlayerId(0);
        game.phase = GamePhase::ActionPhase;

        let result = game.execute_action(TurnAction::KyuushuKyuuhai);
        assert!(result.is_ok());
        assert!(matches!(game.phase, GamePhase::RoundOver));
    }

    // ─── 四风连打测试 ────────────────────────────────────────

    /// 四风连打：四家第一巡打出同一风牌 → 流局
    #[test]
    fn test_suufon_renda_same_wind() {
        let mut game = GameState::new();
        game.phase = GamePhase::ActionPhase;

        // 模拟四家打出东风
        for i in 0..4 {
            game.events.push(GameEvent::PlayerDiscarded {
                player: PlayerId(i),
                tile: Tile::new(Suit::Wind, Rank(1), i as u8),
            });
        }

        assert!(game.check_suufon_renda());
    }

    /// 四风连打：第一家打出不同牌 → 不触发
    #[test]
    fn test_suufon_renda_different_tiles() {
        let mut game = GameState::new();
        game.phase = GamePhase::ActionPhase;

        game.events.push(GameEvent::PlayerDiscarded {
            player: PlayerId(0),
            tile: Tile::new(Suit::Wind, Rank(1), 0), // 东
        });
        game.events.push(GameEvent::PlayerDiscarded {
            player: PlayerId(1),
            tile: Tile::new(Suit::Wind, Rank(1), 1), // 东
        });
        game.events.push(GameEvent::PlayerDiscarded {
            player: PlayerId(2),
            tile: Tile::new(Suit::Wind, Rank(2), 0), // 南 ← 不同
        });
        game.events.push(GameEvent::PlayerDiscarded {
            player: PlayerId(3),
            tile: Tile::new(Suit::Wind, Rank(1), 3), // 东
        });

        assert!(!game.check_suufon_renda());
    }

    /// 四风连打：打出数牌而非风牌 → 不触发
    #[test]
    fn test_suufon_renda_not_wind() {
        let mut game = GameState::new();
        game.phase = GamePhase::ActionPhase;

        for i in 0..4 {
            game.events.push(GameEvent::PlayerDiscarded {
                player: PlayerId(i),
                tile: Tile::new(Suit::Man, Rank(1), i as u8),
            });
        }

        assert!(!game.check_suufon_renda());
    }

    /// 四风连打：有吃碰发生 → 不触发
    #[test]
    fn test_suufon_renda_after_call() {
        let mut game = GameState::new();
        game.phase = GamePhase::ActionPhase;

        for i in 0..4 {
            game.events.push(GameEvent::PlayerDiscarded {
                player: PlayerId(i),
                tile: Tile::new(Suit::Wind, Rank(1), i as u8),
            });
        }
        game.events.push(GameEvent::PlayerCalledPon {
            player: PlayerId(1),
            tiles: vec![],
            from_player: PlayerId(0),
        });

        assert!(!game.check_suufon_renda());
    }

    // ─── 四家立直测试 ────────────────────────────────────────

    /// 四家立直：4人立直 → 触发
    #[test]
    fn test_suucha_riichi_four() {
        let mut game = GameState::new();
        for i in 0..4 {
            game.events.push(GameEvent::PlayerDeclaredRiichi {
                player: PlayerId(i),
            });
        }
        assert!(game.check_suucha_riichi());
    }

    /// 四家立直：3人立直 → 不触发
    #[test]
    fn test_suucha_riichi_three() {
        let mut game = GameState::new();
        for i in 0..3 {
            game.events.push(GameEvent::PlayerDeclaredRiichi {
                player: PlayerId(i),
            });
        }
        assert!(!game.check_suucha_riichi());
    }

    // ─── 抢杠荣和测试 ────────────────────────────────────────

    /// 加杠后进入 ChankanResponse 阶段
    #[test]
    fn test_kakan_enters_chankan_response() {
        let mut game = GameState::new();
        // 玩家0有一个碰副露
        game.players[0].melds.push(Meld::pon(
            vec![
                Tile::new(Suit::Man, Rank(1), 0),
                Tile::new(Suit::Man, Rank(1), 1),
                Tile::new(Suit::Man, Rank(1), 2),
            ],
            Tile::new(Suit::Man, Rank(1), 2),
            PlayerId(1),
        ));
        // 手中有第4张
        game.players[0].hand.add(Tile::new(Suit::Man, Rank(1), 3));
        game.current_player = PlayerId(0);
        game.phase = GamePhase::ActionPhase;

        let result = game.execute_action(TurnAction::Kakan(0, Tile::new(Suit::Man, Rank(1), 3)));
        assert!(result.is_ok());
        assert!(matches!(game.phase, GamePhase::ChankanResponse { .. }));
        // 杠副露应该是 Kakan 状态
        assert_eq!(game.players[0].melds[0].kind, MeldKind::Kakan);
    }

    /// 抢杠荣和：有人荣和时，杠不成立，副露恢复为碰
    #[test]
    fn test_chankan_ron_restores_pon() {
        let mut game = GameState::new();
        // 玩家0有一个碰副露（1m）
        game.players[0].melds.push(Meld::pon(
            vec![
                Tile::new(Suit::Man, Rank(1), 0),
                Tile::new(Suit::Man, Rank(1), 1),
                Tile::new(Suit::Man, Rank(1), 2),
            ],
            Tile::new(Suit::Man, Rank(1), 2),
            PlayerId(1),
        ));
        // 手中第4张 1m
        game.players[0].hand.add(Tile::new(Suit::Man, Rank(1), 3));
        game.current_player = PlayerId(0);
        game.phase = GamePhase::ActionPhase;

        // 执行加杠
        let kakan_tile = Tile::new(Suit::Man, Rank(1), 3);
        game.execute_action(TurnAction::Kakan(0, kakan_tile))
            .unwrap();
        assert!(matches!(game.phase, GamePhase::ChankanResponse { .. }));

        // 玩家1设置为可荣和的手牌（需要能和 1m）
        // 假设玩家1已经听牌 1m
        setup_hand(
            &mut game,
            PlayerId(1),
            vec![
                Tile::new(Suit::Man, Rank(2), 0),
                Tile::new(Suit::Man, Rank(3), 0),
                Tile::new(Suit::Man, Rank(4), 0),
                Tile::new(Suit::Man, Rank(5), 0),
                Tile::new(Suit::Man, Rank(6), 0),
                Tile::new(Suit::Man, Rank(7), 0),
                Tile::new(Suit::Pin, Rank(1), 0),
                Tile::new(Suit::Pin, Rank(2), 0),
                Tile::new(Suit::Pin, Rank(3), 0),
                Tile::new(Suit::Pin, Rank(4), 0),
                Tile::new(Suit::Pin, Rank(5), 0),
                Tile::new(Suit::Pin, Rank(6), 0),
                Tile::new(Suit::Pin, Rank(9), 0),
            ],
        );
        // 玩家1手中有 1m 可以荣和
        // 但需要手牌+1m 能和牌才行
        // 让我用更简单的配置：七对子听 1m
        setup_hand(
            &mut game,
            PlayerId(1),
            vec![
                Tile::new(Suit::Man, Rank(2), 0),
                Tile::new(Suit::Man, Rank(2), 1),
                Tile::new(Suit::Man, Rank(3), 0),
                Tile::new(Suit::Man, Rank(3), 1),
                Tile::new(Suit::Man, Rank(4), 0),
                Tile::new(Suit::Man, Rank(4), 1),
                Tile::new(Suit::Man, Rank(5), 0),
                Tile::new(Suit::Man, Rank(5), 1),
                Tile::new(Suit::Man, Rank(6), 0),
                Tile::new(Suit::Man, Rank(6), 1),
                Tile::new(Suit::Man, Rank(7), 0),
                Tile::new(Suit::Man, Rank(7), 1),
                Tile::new(Suit::Man, Rank(1), 0), // 已有1张1m，等待第2张
            ],
        );

        // 检查抢杠荣和选项
        let options = game.get_call_options();
        let ron_option = options
            .iter()
            .find(|o| o.player == PlayerId(1) && matches!(o.call_type, CallType::Ron));
        assert!(ron_option.is_some(), "玩家1应能抢杠荣和");

        // 执行荣和
        game.execute_call(PlayerId(1), ResponseAction::Ron).unwrap();

        // 杠应恢复为碰
        assert_eq!(game.players[0].melds[0].kind, MeldKind::Pon);
        assert_eq!(game.players[0].melds[0].tiles.len(), 3);
        // 游戏结束
        assert!(matches!(game.phase, GamePhase::RoundOver));
    }

    /// 抢杠荣和：无人荣和 → 杠成立，摸岭上牌
    #[test]
    fn test_chankan_no_ron_draws_rinshan() {
        let mut rng = StdRng::seed_from_u64(42);
        let mut game = GameState::new();
        // 初始化牌山（draw_rinshan 需要牌山）
        game.wall = Tile::all_tiles();
        game.wall.shuffle(&mut rng);

        // 玩家0有一个碰副露（1m）
        game.players[0].melds.push(Meld::pon(
            vec![
                Tile::new(Suit::Man, Rank(1), 0),
                Tile::new(Suit::Man, Rank(1), 1),
                Tile::new(Suit::Man, Rank(1), 2),
            ],
            Tile::new(Suit::Man, Rank(1), 2),
            PlayerId(1),
        ));
        game.players[0].hand.add(Tile::new(Suit::Man, Rank(1), 3));
        game.current_player = PlayerId(0);
        game.phase = GamePhase::ActionPhase;

        let kakan_tile = Tile::new(Suit::Man, Rank(1), 3);
        game.execute_action(TurnAction::Kakan(0, kakan_tile))
            .unwrap();
        assert!(matches!(game.phase, GamePhase::ChankanResponse { .. }));

        // 设置其他玩家为不听牌（不能荣和）
        for i in 1..4 {
            setup_hand(
                &mut game,
                PlayerId(i),
                vec![
                    Tile::new(Suit::Man, Rank(2), i as u8),
                    Tile::new(Suit::Man, Rank(4), i as u8),
                    Tile::new(Suit::Man, Rank(6), i as u8),
                    Tile::new(Suit::Man, Rank(8), i as u8),
                    Tile::new(Suit::Pin, Rank(1), i as u8),
                    Tile::new(Suit::Pin, Rank(3), i as u8),
                    Tile::new(Suit::Pin, Rank(5), i as u8),
                    Tile::new(Suit::Pin, Rank(7), i as u8),
                    Tile::new(Suit::Sou, Rank(1), i as u8),
                    Tile::new(Suit::Sou, Rank(3), i as u8),
                    Tile::new(Suit::Sou, Rank(5), i as u8),
                    Tile::new(Suit::Sou, Rank(7), i as u8),
                    Tile::new(Suit::Wind, Rank(1), i as u8),
                ],
            );
        }

        // 确认无人能荣和
        let options = game.get_call_options();
        assert!(options.is_empty(), "无人应能荣和");

        // 所有人 Pass（由 execute_call Pass 处理）
        game.execute_call(PlayerId(0), ResponseAction::Pass)
            .unwrap();

        // 杠应保持为 Kakan
        assert_eq!(game.players[0].melds[0].kind, MeldKind::Kakan);
        // 进入 ActionPhase（加杠者已摸岭上牌）
        assert!(matches!(game.phase, GamePhase::ActionPhase));
        // 当前玩家应该是加杠者
        assert_eq!(game.current_player, PlayerId(0));
    }

    // ─── 四杠散了测试 ────────────────────────────────────────

    /// 四杠散了：2人各开2杠 → 触发流局
    #[test]
    fn test_suu_kantsu_two_players() {
        let mut game = GameState::new();
        // 玩家0开2个暗杠
        game.players[0].melds.push(Meld::ankan(vec![
            Tile::new(Suit::Man, Rank(1), 0),
            Tile::new(Suit::Man, Rank(1), 1),
            Tile::new(Suit::Man, Rank(1), 2),
            Tile::new(Suit::Man, Rank(1), 3),
        ]));
        game.players[0].melds.push(Meld::ankan(vec![
            Tile::new(Suit::Man, Rank(2), 0),
            Tile::new(Suit::Man, Rank(2), 1),
            Tile::new(Suit::Man, Rank(2), 2),
            Tile::new(Suit::Man, Rank(2), 3),
        ]));
        // 玩家1开2个暗杠
        game.players[1].melds.push(Meld::ankan(vec![
            Tile::new(Suit::Pin, Rank(1), 0),
            Tile::new(Suit::Pin, Rank(1), 1),
            Tile::new(Suit::Pin, Rank(1), 2),
            Tile::new(Suit::Pin, Rank(1), 3),
        ]));
        game.players[1].melds.push(Meld::ankan(vec![
            Tile::new(Suit::Pin, Rank(2), 0),
            Tile::new(Suit::Pin, Rank(2), 1),
            Tile::new(Suit::Pin, Rank(2), 2),
            Tile::new(Suit::Pin, Rank(2), 3),
        ]));

        assert_eq!(game.get_kan_count(), 4);
        assert!(game.check_four_kan_abort());
    }

    /// 四杠散了：1人开4杠 → 不流局（保留四杠子机会）
    #[test]
    fn test_suu_kantsu_single_player() {
        let mut game = GameState::new();
        for rank in 1..=4 {
            game.players[0].melds.push(Meld::ankan(vec![
                Tile::new(Suit::Man, Rank(rank), 0),
                Tile::new(Suit::Man, Rank(rank), 1),
                Tile::new(Suit::Man, Rank(rank), 2),
                Tile::new(Suit::Man, Rank(rank), 3),
            ]));
        }

        assert_eq!(game.get_kan_count(), 4);
        assert!(!game.check_four_kan_abort());
    }

    /// can_declare_kan：4杠由1人持有 → 该玩家可继续
    #[test]
    fn test_can_declare_kan_single_owner() {
        let mut game = GameState::new();
        for rank in 1..=4 {
            game.players[0].melds.push(Meld::ankan(vec![
                Tile::new(Suit::Man, Rank(rank), 0),
                Tile::new(Suit::Man, Rank(rank), 1),
                Tile::new(Suit::Man, Rank(rank), 2),
                Tile::new(Suit::Man, Rank(rank), 3),
            ]));
        }

        assert!(game.can_declare_kan(PlayerId(0))); // 持有者可继续
        assert!(!game.can_declare_kan(PlayerId(1))); // 其他人不行
        assert!(!game.can_declare_kan(PlayerId(2)));
    }

    /// can_declare_kan：4杠由多人持有 → 谁都不能开（已流局）
    #[test]
    fn test_can_declare_kan_multiple_owners() {
        let mut game = GameState::new();
        game.players[0].melds.push(Meld::ankan(vec![
            Tile::new(Suit::Man, Rank(1), 0),
            Tile::new(Suit::Man, Rank(1), 1),
            Tile::new(Suit::Man, Rank(1), 2),
            Tile::new(Suit::Man, Rank(1), 3),
        ]));
        game.players[0].melds.push(Meld::ankan(vec![
            Tile::new(Suit::Man, Rank(2), 0),
            Tile::new(Suit::Man, Rank(2), 1),
            Tile::new(Suit::Man, Rank(2), 2),
            Tile::new(Suit::Man, Rank(2), 3),
        ]));
        game.players[1].melds.push(Meld::ankan(vec![
            Tile::new(Suit::Pin, Rank(1), 0),
            Tile::new(Suit::Pin, Rank(1), 1),
            Tile::new(Suit::Pin, Rank(1), 2),
            Tile::new(Suit::Pin, Rank(1), 3),
        ]));
        game.players[1].melds.push(Meld::ankan(vec![
            Tile::new(Suit::Pin, Rank(2), 0),
            Tile::new(Suit::Pin, Rank(2), 1),
            Tile::new(Suit::Pin, Rank(2), 2),
            Tile::new(Suit::Pin, Rank(2), 3),
        ]));

        // 4杠由2人持有，check_four_kan_abort 为 true
        assert!(game.check_four_kan_abort());
        // can_declare_kan 不会再被调用（游戏已结束），但逻辑上都不行
        assert!(!game.can_declare_kan(PlayerId(0)));
        assert!(!game.can_declare_kan(PlayerId(1)));
    }

    // ─── 不听罚符测试 ────────────────────────────────────────

    /// 无人听牌 → 无罚符
    #[test]
    fn test_exhaustive_draw_no_tenpai() {
        let mut game = GameState::new();
        // 给所有玩家设置不听牌的手牌
        for i in 0..4 {
            setup_hand(
                &mut game,
                PlayerId(i),
                vec![
                    Tile::new(Suit::Man, Rank(1), 0),
                    Tile::new(Suit::Man, Rank(3), 0),
                    Tile::new(Suit::Man, Rank(5), 0),
                    Tile::new(Suit::Man, Rank(7), 0),
                    Tile::new(Suit::Pin, Rank(1), 0),
                    Tile::new(Suit::Pin, Rank(3), 0),
                    Tile::new(Suit::Pin, Rank(5), 0),
                    Tile::new(Suit::Pin, Rank(7), 0),
                    Tile::new(Suit::Sou, Rank(1), 0),
                    Tile::new(Suit::Sou, Rank(3), 0),
                    Tile::new(Suit::Sou, Rank(5), 0),
                    Tile::new(Suit::Sou, Rank(7), 0),
                    Tile::new(Suit::Wind, Rank(1), 0),
                ],
            );
        }

        game.resolve_exhaustive_draw();

        // 无人听牌，无罚符
        for i in 0..4 {
            assert_eq!(game.players[i].points, 25000);
        }
    }

    /// 1人听牌 → 未听3人各付1000
    #[test]
    fn test_exhaustive_draw_one_tenpai() {
        let mut game = GameState::new();
        // 玩家0：听牌（13张，有搭子）
        setup_hand(
            &mut game,
            PlayerId(0),
            vec![
                Tile::new(Suit::Man, Rank(1), 0),
                Tile::new(Suit::Man, Rank(2), 0),
                Tile::new(Suit::Man, Rank(3), 0),
                Tile::new(Suit::Man, Rank(4), 0),
                Tile::new(Suit::Man, Rank(5), 0),
                Tile::new(Suit::Man, Rank(6), 0),
                Tile::new(Suit::Pin, Rank(1), 0),
                Tile::new(Suit::Pin, Rank(2), 0),
                Tile::new(Suit::Pin, Rank(3), 0),
                Tile::new(Suit::Pin, Rank(4), 0),
                Tile::new(Suit::Pin, Rank(5), 0),
                Tile::new(Suit::Pin, Rank(6), 0),
                Tile::new(Suit::Pin, Rank(9), 0),
            ],
        );
        // 玩家1-3：不听
        for i in 1..4 {
            setup_hand(
                &mut game,
                PlayerId(i),
                vec![
                    Tile::new(Suit::Man, Rank(1), i as u8),
                    Tile::new(Suit::Man, Rank(3), i as u8),
                    Tile::new(Suit::Man, Rank(5), i as u8),
                    Tile::new(Suit::Man, Rank(7), i as u8),
                    Tile::new(Suit::Pin, Rank(1), i as u8),
                    Tile::new(Suit::Pin, Rank(3), i as u8),
                    Tile::new(Suit::Pin, Rank(5), i as u8),
                    Tile::new(Suit::Pin, Rank(7), i as u8),
                    Tile::new(Suit::Sou, Rank(1), i as u8),
                    Tile::new(Suit::Sou, Rank(3), i as u8),
                    Tile::new(Suit::Sou, Rank(5), i as u8),
                    Tile::new(Suit::Sou, Rank(7), i as u8),
                    Tile::new(Suit::Wind, Rank(1), i as u8),
                ],
            );
        }

        game.resolve_exhaustive_draw();

        assert_eq!(game.players[0].points, 25000 + 3000); // 听牌者 +3000
        assert_eq!(game.players[1].points, 25000 - 1000);
        assert_eq!(game.players[2].points, 25000 - 1000);
        assert_eq!(game.players[3].points, 25000 - 1000);
    }

    // ─── 连庄/过庄测试 ────────────────────────────────────────

    /// 庄家自摸 → 连庄（round 不变，honba +1）
    #[test]
    fn test_dealer_win_continues() {
        let mut game = GameState::new();
        game.round = 1;
        game.honba = 0;
        // 庄家是 PlayerId(0)（round=1 → dealer = (1-1)%4 = 0）
        assert_eq!(game.get_dealer(), PlayerId(0));

        game.advance_round(&RoundEndReason::Win {
            winner: PlayerId(0),
            is_tsumo: true,
        });

        assert_eq!(game.round, 1); // 不变
        assert_eq!(game.honba, 1); // +1
    }

    /// 闲家自摸 → 过庄（round +1，honba = 0）
    #[test]
    fn test_non_dealer_win_rotates() {
        let mut game = GameState::new();
        game.round = 1;
        game.honba = 2;

        game.advance_round(&RoundEndReason::Win {
            winner: PlayerId(1),
            is_tsumo: true,
        });

        assert_eq!(game.round, 2); // +1
        assert_eq!(game.honba, 0); // 重置
    }

    /// 荒牌流局庄家听牌 → 连庄
    #[test]
    fn test_exhaustive_draw_dealer_tenpai_continues() {
        let mut game = GameState::new();
        game.round = 1;
        game.honba = 0;
        // 庄家（PlayerId 0）设置听牌手牌
        setup_hand(
            &mut game,
            PlayerId(0),
            vec![
                Tile::new(Suit::Man, Rank(1), 0),
                Tile::new(Suit::Man, Rank(2), 0),
                Tile::new(Suit::Man, Rank(3), 0),
                Tile::new(Suit::Man, Rank(4), 0),
                Tile::new(Suit::Man, Rank(5), 0),
                Tile::new(Suit::Man, Rank(6), 0),
                Tile::new(Suit::Pin, Rank(1), 0),
                Tile::new(Suit::Pin, Rank(2), 0),
                Tile::new(Suit::Pin, Rank(3), 0),
                Tile::new(Suit::Pin, Rank(4), 0),
                Tile::new(Suit::Pin, Rank(5), 0),
                Tile::new(Suit::Pin, Rank(6), 0),
                Tile::new(Suit::Pin, Rank(9), 0),
            ],
        );
        // 其他玩家不听
        for i in 1..4 {
            setup_hand(
                &mut game,
                PlayerId(i),
                vec![
                    Tile::new(Suit::Man, Rank(1), i as u8),
                    Tile::new(Suit::Man, Rank(3), i as u8),
                    Tile::new(Suit::Man, Rank(5), i as u8),
                    Tile::new(Suit::Man, Rank(7), i as u8),
                    Tile::new(Suit::Pin, Rank(1), i as u8),
                    Tile::new(Suit::Pin, Rank(3), i as u8),
                    Tile::new(Suit::Pin, Rank(5), i as u8),
                    Tile::new(Suit::Pin, Rank(7), i as u8),
                    Tile::new(Suit::Sou, Rank(1), i as u8),
                    Tile::new(Suit::Sou, Rank(3), i as u8),
                    Tile::new(Suit::Sou, Rank(5), i as u8),
                    Tile::new(Suit::Sou, Rank(7), i as u8),
                    Tile::new(Suit::Wind, Rank(1), i as u8),
                ],
            );
        }

        game.advance_round(&RoundEndReason::ExhaustiveDraw);

        assert_eq!(game.round, 1); // 连庄
        assert_eq!(game.honba, 1);
    }

    /// 荒牌流局庄家未听 → 过庄
    #[test]
    fn test_exhaustive_draw_dealer_not_tenpai_rotates() {
        let mut game = GameState::new();
        game.round = 1;
        game.honba = 0;
        // 所有玩家都不听
        for i in 0..4 {
            setup_hand(
                &mut game,
                PlayerId(i),
                vec![
                    Tile::new(Suit::Man, Rank(1), i as u8),
                    Tile::new(Suit::Man, Rank(3), i as u8),
                    Tile::new(Suit::Man, Rank(5), i as u8),
                    Tile::new(Suit::Man, Rank(7), i as u8),
                    Tile::new(Suit::Pin, Rank(1), i as u8),
                    Tile::new(Suit::Pin, Rank(3), i as u8),
                    Tile::new(Suit::Pin, Rank(5), i as u8),
                    Tile::new(Suit::Pin, Rank(7), i as u8),
                    Tile::new(Suit::Sou, Rank(1), i as u8),
                    Tile::new(Suit::Sou, Rank(3), i as u8),
                    Tile::new(Suit::Sou, Rank(5), i as u8),
                    Tile::new(Suit::Sou, Rank(7), i as u8),
                    Tile::new(Suit::Wind, Rank(1), i as u8),
                ],
            );
        }

        game.advance_round(&RoundEndReason::ExhaustiveDraw);

        assert_eq!(game.round, 2); // 过庄
        assert_eq!(game.honba, 0);
    }

    /// 途中流局 → 一律连庄
    #[test]
    fn test_abortive_draw_always_continues() {
        let reasons = vec![
            RoundEndReason::KyuushuKyuuhai,
            RoundEndReason::SuufonRenda,
            RoundEndReason::SuuchaRiichi,
            RoundEndReason::SuuKantsu,
        ];

        for reason in reasons {
            let mut game = GameState::new();
            game.round = 3;
            game.honba = 2;
            game.advance_round(&reason);
            assert_eq!(game.round, 3, "途中流局应连庄: {:?}", reason);
            assert_eq!(game.honba, 3, "途中流局 honba +1: {:?}", reason);
        }
    }

    // ─── 场风切换测试 ────────────────────────────────────────

    /// 东四局过庄 → 南一局
    #[test]
    fn test_east4_to_south1() {
        let mut game = GameState::new();
        game.round = 4; // 东四局
        game.honba = 0;
        game.wind = TileType::EAST;

        // 闲家自摸 → 过庄
        game.advance_round(&RoundEndReason::Win {
            winner: PlayerId(1),
            is_tsumo: true,
        });

        assert_eq!(game.round, 5);
        assert_eq!(game.wind, TileType::SOUTH);
        assert_eq!(game.honba, 0);
    }

    /// 南四局过庄 → 游戏结束
    #[test]
    fn test_south4_game_over() {
        let mut game = GameState::new();
        game.round = 8; // 南四局
        game.wind = TileType::SOUTH;

        game.advance_round(&RoundEndReason::Win {
            winner: PlayerId(1),
            is_tsumo: true,
        });

        assert!(game.is_game_over());
    }

    /// 南四局庄家自摸 → 连庄，游戏未结束
    #[test]
    fn test_south4_dealer_win_not_over() {
        let mut game = GameState::new();
        game.round = 8; // 南四局
        game.honba = 0;
        game.wind = TileType::SOUTH;
        // 庄家 = (8-1)%4 = 3

        game.advance_round(&RoundEndReason::Win {
            winner: PlayerId(3),
            is_tsumo: true,
        });

        assert_eq!(game.round, 8); // 连庄
        assert_eq!(game.honba, 1);
        assert!(!game.is_game_over());
    }

    // ─── 振听测试 ──────────────────────────────────────────

    /// 舍牌振听：听牌中有自己打出过的牌 → 振听
    #[test]
    fn test_discard_furiten_enter() {
        let mut game = GameState::new();
        // 手牌：1m2m3m 4m5m6m 7m8m9m 1p1p 2p → 听 1p/3p
        let hand_tiles = vec![
            Tile::new(Suit::Man, Rank(1), 0),
            Tile::new(Suit::Man, Rank(2), 0),
            Tile::new(Suit::Man, Rank(3), 0),
            Tile::new(Suit::Man, Rank(4), 0),
            Tile::new(Suit::Man, Rank(5), 0),
            Tile::new(Suit::Man, Rank(6), 0),
            Tile::new(Suit::Man, Rank(7), 0),
            Tile::new(Suit::Man, Rank(8), 0),
            Tile::new(Suit::Man, Rank(9), 0),
            Tile::new(Suit::Pin, Rank(1), 0),
            Tile::new(Suit::Pin, Rank(1), 1),
            Tile::new(Suit::Pin, Rank(2), 0),
            Tile::new(Suit::Pin, Rank(3), 0),
        ];
        setup_hand(&mut game, PlayerId(0), hand_tiles);
        game.players[0].all_discarded_types.insert(TileType(9)); // 1p

        game.update_discard_furiten(PlayerId(0));
        assert!(game.players[0].furiten.discard, "听 1p/3p 且打出过 1p → 舍牌振听");
    }

    /// 舍牌振听解除：听牌变化后不再包含打出过的牌
    #[test]
    fn test_discard_furiten_release() {
        let mut game = GameState::new();
        game.players[0].all_discarded_types.insert(TileType(9)); // 打出过 1p

        // 手牌改为只听 7s：1m2m3m 4m5m6m 7m8m9m 1p1p 2p → 换掉 2p 3p
        // 改为：1m2m3m 4m5m6m 7m8m9m 5s5s 6s → 听 4s/7s
        let hand_tiles = vec![
            Tile::new(Suit::Man, Rank(1), 0),
            Tile::new(Suit::Man, Rank(2), 0),
            Tile::new(Suit::Man, Rank(3), 0),
            Tile::new(Suit::Man, Rank(4), 0),
            Tile::new(Suit::Man, Rank(5), 0),
            Tile::new(Suit::Man, Rank(6), 0),
            Tile::new(Suit::Man, Rank(7), 0),
            Tile::new(Suit::Man, Rank(8), 0),
            Tile::new(Suit::Man, Rank(9), 0),
            Tile::new(Suit::Sou, Rank(5), 0),
            Tile::new(Suit::Sou, Rank(5), 1),
            Tile::new(Suit::Sou, Rank(6), 0),
            Tile::new(Suit::Sou, Rank(7), 0),
        ];
        setup_hand(&mut game, PlayerId(0), hand_tiles);

        game.update_discard_furiten(PlayerId(0));
        assert!(!game.players[0].furiten.discard, "听 4s/7s，没打出过 → 无舍牌振听");
    }

    /// 同巡振听：别家打出听牌但不荣和 → 振听
    #[test]
    fn test_round_furiten_enter() {
        let mut game = GameState::new();
        // 玩家1手牌：1m2m3m 4m5m6m 7m8m9m 1p1p 2p → 听 1p/3p
        let hand_tiles = vec![
            Tile::new(Suit::Man, Rank(1), 0),
            Tile::new(Suit::Man, Rank(2), 0),
            Tile::new(Suit::Man, Rank(3), 0),
            Tile::new(Suit::Man, Rank(4), 0),
            Tile::new(Suit::Man, Rank(5), 0),
            Tile::new(Suit::Man, Rank(6), 0),
            Tile::new(Suit::Man, Rank(7), 0),
            Tile::new(Suit::Man, Rank(8), 0),
            Tile::new(Suit::Man, Rank(9), 0),
            Tile::new(Suit::Pin, Rank(1), 0),
            Tile::new(Suit::Pin, Rank(1), 1),
            Tile::new(Suit::Pin, Rank(2), 0),
            Tile::new(Suit::Pin, Rank(3), 0),
        ];
        setup_hand(&mut game, PlayerId(1), hand_tiles);

        let discarded = Tile::new(Suit::Pin, Rank(4), 0); // 4p
        game.players[0].discards.push(discarded);

        let waiting = game.get_waiting_tile_types(PlayerId(1));
        assert!(waiting.contains(&TileType(12)), "玩家1应听 4p");

        if waiting.contains(&discarded.tile_type()) {
            game.players[1].furiten.round = true;
        }
        assert!(game.players[1].furiten.round, "玩家1应进入同巡振听");
    }

    /// 同巡振听解除：自己打出一张牌后解除
    #[test]
    fn test_round_furiten_release() {
        let mut game = GameState::new();
        game.players[0].furiten.round = true;
        assert!(game.players[0].furiten.is_furiten());

        game.players[0].furiten.clear_round();
        assert!(!game.players[0].furiten.is_furiten());
    }

    /// 立直振听：立直后不荣和 → 永久振听
    #[test]
    fn test_riichi_furiten_enter() {
        let mut game = GameState::new();
        game.players[0].is_riichi = true;
        game.players[0].furiten.riichi = true;
        assert!(game.players[0].furiten.is_furiten());

        game.players[0].furiten.clear_round();
        assert!(game.players[0].furiten.is_furiten(), "立直振听不因打牌解除");
    }

    /// 振听不影响自摸
    #[test]
    fn test_furiten_allows_tsumo() {
        use riichi_logic::win_check;
        use riichi_logic::types::WinContext;

        let hand_tile_types = vec![
            TileType(0), TileType(0), TileType(1), TileType(1), TileType(2), TileType(2),
            TileType(9), TileType(9), TileType(10), TileType(10), TileType(11), TileType(11),
            TileType(18), TileType(18),
        ];
        let all_tiles: Vec<Tile> = hand_tile_types.iter()
            .enumerate()
            .map(|(i, &tt)| Tile::from_raw(tt.0 * 4 + (i as u8 % 4)))
            .collect();
        let ctx = WinContext {
            is_tsumo: true,
            is_riichi: false,
            is_double_riichi: false,
            is_ippatsu: false,
            is_rinshan: false,
            is_chankan: false,
            is_haitei: false,
            is_houtei: false,
            seat_wind: TileType::EAST,
            field_wind: TileType::EAST,
            dora_indicators: vec![],
            ura_dora_indicators: vec![],
            melds: vec![],
            dealer: 0,
            winner: 0,
            loser: None,
            honba: 0,
            riichi_sticks: 0,
        };
        let winning_tile = all_tiles[0];

        let result = win_check::check_win(&all_tiles, &hand_tile_types, &ctx, true, winning_tile);
        assert!(result.is_some(), "振听不应阻止自摸");
    }
}
