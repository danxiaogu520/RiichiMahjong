use rand::Rng;
use riichi_ai::DiscardOption;
use riichi_core::game::CallOption;
use riichi_core::meld::Meld;
use riichi_core::player::PlayerId;
use riichi_core::tile::{Tile, TileType};
use riichi_engine::{game::GamePhase, TenpaiInfo};
use riichi_logic::shanten::ShantenCalculator;
use riichi_logic::visibility::VisibleTiles;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

use riichi_ai::{choose_discard, decide_call, decide_riichi};
use riichi_session::{
    CallResponse, ClientHandle, PlayerAction, PlayerCommand, SessionEvent, TurnAction,
};

pub struct App {
    pub event_rx: mpsc::Receiver<SessionEvent>,
    pub action_tx: mpsc::Sender<PlayerCommand>,

    pub phase: GamePhase,
    pub current_player: PlayerId,
    pub pending_discard: Option<(PlayerId, Tile)>,
    pub hand_tiles: Vec<Tile>,
    pub drawn_tile: Option<Tile>,
    pub hand_count: usize,
    pub hand_counts: [usize; 4],
    pub points: [i32; 4],
    pub discards: [Vec<Tile>; 4],
    pub melds_count: [usize; 4],
    pub melds: [Vec<Meld>; 4],
    pub dora: Vec<TileType>,
    pub remaining_tiles: usize,
    pub round: u32,
    pub honba: u32,
    pub riichi_sticks: u32,

    pub can_tsumo: bool,
    pub can_riichi: bool,
    pub riichi_options: Vec<Tile>,
    pub ankan_options: Vec<Tile>,
    pub kakan_options: Vec<(usize, Tile)>,
    pub can_kyuushu: bool,
    pub call_options: Vec<CallOption>,
    pub analysis_options: Vec<DiscardOption>,
    pub tenpai_info: Option<TenpaiInfo>,
    pub discard_options: Vec<Tile>,
    pub show_analysis: bool,
    pub show_messages: bool,

    pub messages: Vec<String>,
    pub selected: usize,
    pub call_selected: usize,
    /// 立直选择模式下的当前合法弃牌。
    pub riichi_selected: usize,
    pub riichi_selecting: bool,
    pub should_quit: bool,
    pub show_result: bool,
    pub game_over: bool,
    pub scores: [i32; 4],
    pub ranking: [usize; 4],
    pub auto_play: bool,
    ai_decision_deadline: Option<Instant>,
    ai_action_in_flight: bool,
    ai_calculator: ShantenCalculator,
}

impl App {
    pub fn new(handle: ClientHandle) -> Self {
        Self {
            event_rx: handle.event_rx,
            action_tx: handle.action_tx,
            phase: GamePhase::ActionPhase {
                player: PlayerId(0),
                drawn_tile: None,
            },
            current_player: PlayerId(0),
            pending_discard: None,
            hand_tiles: Vec::new(),
            drawn_tile: None,
            hand_count: 0,
            hand_counts: [0; 4],
            points: [25000; 4],
            discards: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            melds_count: [0; 4],
            melds: [Vec::new(), Vec::new(), Vec::new(), Vec::new()],
            dora: Vec::new(),
            remaining_tiles: 0,
            round: 1,
            honba: 0,
            riichi_sticks: 0,
            can_tsumo: false,
            can_riichi: false,
            riichi_options: Vec::new(),
            ankan_options: Vec::new(),
            kakan_options: Vec::new(),
            can_kyuushu: false,
            call_options: Vec::new(),
            analysis_options: Vec::new(),
            tenpai_info: None,
            discard_options: Vec::new(),
            show_analysis: true,
            show_messages: true,
            messages: Vec::new(),
            selected: 0,
            call_selected: 0,
            riichi_selected: 0,
            riichi_selecting: false,
            should_quit: false,
            show_result: false,
            game_over: false,
            scores: [0; 4],
            ranking: [0, 1, 2, 3],
            auto_play: false,
            ai_decision_deadline: None,
            ai_action_in_flight: false,
            ai_calculator: ShantenCalculator::new(),
        }
    }

    pub async fn process_server_events(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            match event {
                SessionEvent::StateUpdate {
                    phase,
                    pending_discard,
                    hand_tiles,
                    hand_count,
                    hand_counts,
                    points,
                    discards,
                    melds_count,
                    melds,
                    dora,
                    remaining_tiles,
                    round,
                    honba,
                    riichi_sticks,
                    tenpai_info,
                    ..
                } => {
                    self.phase = phase;
                    self.current_player = match self.phase {
                        GamePhase::DrawPhase { player, .. }
                        | GamePhase::ActionPhase { player, .. }
                        | GamePhase::ResponsePhase { player, .. }
                        | GamePhase::ChankanResponse { player, .. } => player,
                        GamePhase::RoundOver => PlayerId(0),
                    };
                    self.pending_discard = pending_discard;
                    self.drawn_tile = match self.phase {
                        GamePhase::ActionPhase { drawn_tile, .. } => drawn_tile,
                        _ => None,
                    };
                    self.hand_tiles = hand_tiles;
                    self.hand_count = hand_count;
                    self.hand_counts = hand_counts;
                    self.points = points;
                    self.discards = discards;
                    self.melds_count = melds_count;
                    self.melds = melds;
                    self.dora = dora;
                    self.remaining_tiles = remaining_tiles;
                    self.round = round;
                    self.honba = honba;
                    self.riichi_sticks = riichi_sticks;
                    self.tenpai_info = tenpai_info;
                    self.analysis_options = crate::analysis::analyze_discards(
                        &self.hand_tiles,
                        &self.discards,
                        &self.melds,
                        &self.dora,
                        self.pending_discard,
                    );
                    self.ai_action_in_flight = false;
                    // 每个状态快照都代表新的权威牌局状态；响应选项只对
                    // 生成它的那一张弃牌有效，不能跨响应窗口保留。
                    self.call_options.clear();
                    self.call_selected = 0;
                    if self.selected >= self.hand_tiles.len()
                        || !self.tile_is_discardable(self.hand_tiles[self.selected])
                    {
                        self.selected = self.selectable_indices().first().copied().unwrap_or(0);
                    }
                }
                SessionEvent::ActionRequired {
                    can_tsumo,
                    can_riichi,
                    riichi_options,
                    ankan_options,
                    kakan_options,
                    can_kyuushu,
                    discard_options,
                    ..
                } => {
                    self.can_tsumo = can_tsumo;
                    self.can_riichi = can_riichi;
                    self.riichi_options = riichi_options;
                    self.ankan_options = ankan_options;
                    self.kakan_options = kakan_options;
                    self.can_kyuushu = can_kyuushu;
                    self.discard_options = discard_options;
                    self.riichi_selecting = false;
                    self.call_options.clear();
                    self.ai_action_in_flight = false;
                    if self.auto_play {
                        self.schedule_ai_decision();
                    }
                }
                SessionEvent::CallRequired { options } => {
                    self.call_options = options;
                    self.call_selected = 0;
                    self.ai_action_in_flight = false;
                    if self.auto_play {
                        self.schedule_ai_decision();
                    }
                }
                SessionEvent::RoundResult {
                    reason,
                    win_details,
                    point_changes,
                    scores,
                } => {
                    self.messages.push(format!("本局结束：{}", reason));
                    for detail in win_details {
                        self.messages.push(format!("和牌明细：{}", detail));
                    }
                    self.messages.push(format!(
                        "点棒变化：东{:+} 南{:+} 西{:+} 北{:+}",
                        point_changes[0], point_changes[1], point_changes[2], point_changes[3]
                    ));
                    self.messages.push(format!(
                        "结算点数：东{} 南{} 西{} 北{}",
                        scores[0], scores[1], scores[2], scores[3]
                    ));
                }
                SessionEvent::GameOver { scores, ranking } => {
                    self.scores = scores;
                    self.ranking = ranking;
                    self.game_over = true;
                    self.show_result = true;
                }
                SessionEvent::Error(message) => {
                    self.messages.push(message);
                }
                SessionEvent::GameEvent { .. } => {}
            }
        }
    }

    pub fn is_human_turn(&self) -> bool {
        self.current_player == PlayerId(0)
            && matches!(self.phase, GamePhase::ActionPhase { .. })
            && self.call_options.is_empty()
            && !self.auto_play
    }

    pub fn tile_is_discardable(&self, tile: Tile) -> bool {
        self.discard_options.is_empty() || self.discard_options.contains(&tile)
    }

    pub fn selectable_indices(&self) -> Vec<usize> {
        self.hand_tiles
            .iter()
            .enumerate()
            .filter_map(|(index, tile)| self.tile_is_discardable(*tile).then_some(index))
            .collect()
    }

    pub fn needs_human_response(&self) -> bool {
        !self.call_options.is_empty() && !self.auto_play
    }

    pub fn is_ai_thinking(&self) -> bool {
        self.auto_play && self.has_ai_prompt()
    }

    pub fn toggle_auto_play(&mut self) {
        self.auto_play = !self.auto_play;
        self.ai_decision_deadline = None;
        self.ai_action_in_flight = false;
        if self.auto_play {
            self.messages
                .push("已开启托管，AI 将接管你的行动".to_string());
            if self.has_ai_prompt() {
                self.schedule_ai_decision();
            }
        } else {
            self.messages.push("已关闭托管，恢复手动操作".to_string());
        }
    }

    /// 在主循环中调用；到达随机思考时间后发送一次 AI 决策。
    pub fn tick_ai(&mut self) {
        if !self.auto_play || !self.has_ai_prompt() {
            return;
        }
        let Some(deadline) = self.ai_decision_deadline else {
            self.schedule_ai_decision();
            return;
        };
        if Instant::now() < deadline {
            return;
        }
        self.ai_decision_deadline = None;
        self.ai_action_in_flight = true;

        if !self.call_options.is_empty() {
            let response = decide_call(&self.call_options);
            match response {
                Some(riichi_core::game::ResponseAction::Ron) => self.send_call_ron(),
                _ => self.send_call_pass(),
            }
            return;
        }

        let visible = self.visible_tiles();
        if self.can_tsumo {
            self.send_tsumo();
            return;
        }
        if self.can_riichi {
            if let Some(tile) = decide_riichi(
                &self.ai_calculator,
                &self.hand_tiles,
                &visible,
                &self.riichi_options,
            )
            .or_else(|| self.riichi_options.first().copied())
            {
                self.send_riichi_tile(Some(tile));
                return;
            }
        }

        let tile = if self.discard_options.len() == 1 {
            self.discard_options[0]
        } else {
            choose_discard(&self.ai_calculator, &self.hand_tiles, &visible)
                .and_then(|option| {
                    self.discard_options
                        .iter()
                        .copied()
                        .find(|candidate| candidate.tile_type() == option.tile.tile_type())
                })
                .or_else(|| self.discard_options.first().copied())
                .or_else(|| self.hand_tiles.last().copied())
                .unwrap_or_else(|| Tile::from_raw(0))
        };
        self.send_discard(tile);
    }

    fn has_ai_prompt(&self) -> bool {
        !self.ai_action_in_flight
            && (!self.call_options.is_empty()
                || (self.current_player == PlayerId(0)
                    && matches!(self.phase, GamePhase::ActionPhase { .. })
                    && (!self.discard_options.is_empty() || self.can_tsumo || self.can_riichi)))
    }

    fn schedule_ai_decision(&mut self) {
        let delay_ms = rand::thread_rng().gen_range(1_000..=2_000);
        self.ai_decision_deadline = Some(Instant::now() + Duration::from_millis(delay_ms));
    }

    fn visible_tiles(&self) -> VisibleTiles {
        let player_melds = vec![self.melds[0]
            .iter()
            .flat_map(|meld| meld.tiles.iter().copied())
            .collect::<Vec<_>>()];
        let other_melds = self.melds[1..]
            .iter()
            .map(|melds| {
                melds
                    .iter()
                    .flat_map(|meld| meld.tiles.iter().copied())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let all_discards = self.discards.iter().flatten().copied().collect::<Vec<_>>();
        VisibleTiles::from_data(&player_melds, &other_melds, &all_discards, &self.dora)
    }

    pub fn send_discard(&self, tile: Tile) {
        let _ = self.action_tx.try_send(PlayerCommand::new(
            PlayerId(0),
            PlayerAction::TurnAction(TurnAction::Discard(tile)),
        ));
    }

    pub fn send_tsumo(&self) {
        let _ = self.action_tx.try_send(PlayerCommand::new(
            PlayerId(0),
            PlayerAction::TurnAction(TurnAction::Tsumo),
        ));
    }

    pub fn send_riichi_tile(&self, tile: Option<Tile>) {
        let Some(tile) = tile else {
            return;
        };
        let _ = self.action_tx.try_send(PlayerCommand::new(
            PlayerId(0),
            PlayerAction::TurnAction(TurnAction::RiichiDiscard(tile)),
        ));
    }

    pub fn send_ankan(&self, tile: Tile) {
        let _ = self.action_tx.try_send(PlayerCommand::new(
            PlayerId(0),
            PlayerAction::TurnAction(TurnAction::Ankan(tile)),
        ));
    }

    pub fn send_kakan(&self, meld_index: usize, tile: Tile) {
        let _ = self.action_tx.try_send(PlayerCommand::new(
            PlayerId(0),
            PlayerAction::TurnAction(TurnAction::Kakan(meld_index, tile)),
        ));
    }

    pub fn send_kyuushu(&self) {
        let _ = self.action_tx.try_send(PlayerCommand::new(
            PlayerId(0),
            PlayerAction::TurnAction(TurnAction::KyuushuKyuuhai),
        ));
    }

    pub fn send_call_ron(&self) {
        let _ = self.action_tx.try_send(PlayerCommand::new(
            PlayerId(0),
            PlayerAction::CallResponse(CallResponse::Ron),
        ));
    }

    pub fn send_call_pon(&self, hand_tiles: [Tile; 2]) {
        let _ = self.action_tx.try_send(PlayerCommand::new(
            PlayerId(0),
            PlayerAction::CallResponse(CallResponse::Pon { hand_tiles }),
        ));
    }

    pub fn send_call_chi(&self, hand_tiles: [Tile; 2]) {
        let _ = self.action_tx.try_send(PlayerCommand::new(
            PlayerId(0),
            PlayerAction::CallResponse(CallResponse::Chi { hand_tiles }),
        ));
    }

    pub fn send_call_minkan(&self, hand_tiles: [Tile; 3]) {
        let _ = self.action_tx.try_send(PlayerCommand::new(
            PlayerId(0),
            PlayerAction::CallResponse(CallResponse::Minkan { hand_tiles }),
        ));
    }

    pub fn send_call_pass(&self) {
        let _ = self.action_tx.try_send(PlayerCommand::new(
            PlayerId(0),
            PlayerAction::CallResponse(CallResponse::Pass),
        ));
    }

    pub fn player_name(&self, idx: usize) -> &str {
        match idx {
            0 if self.auto_play => "你(托管)",
            0 => "你",
            1 => "AI-南",
            2 => "AI-西",
            3 => "AI-北",
            _ => "?",
        }
    }

    pub fn hand_count_for(&self, idx: usize) -> usize {
        if idx == 0 {
            self.hand_tiles.len()
        } else {
            self.hand_counts[idx]
        }
    }

    pub fn round_display(&self) -> String {
        let wind = if self.round <= 4 { "东" } else { "南" };
        let num = (self.round.saturating_sub(1) % 4) + 1;
        format!("{}{}局", wind, num)
    }
}
