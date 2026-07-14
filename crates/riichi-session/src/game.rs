use rand::rngs::StdRng;
use rand::SeedableRng;
use riichi_core::game::{
    CallType, GameEvent, ResponseAction, RoundEndReason, TurnAction as EngineTurnAction,
};
use riichi_core::player::PlayerId;
use riichi_core::tile::Tile;
use riichi_engine::game::{GamePhase, GameState};
use riichi_engine::legal::LegalAction;
use riichi_engine::rules::{
    ALLOW_DOUBLE_RON, ALLOW_TRIPLE_RON, RESPONSE_TIMEOUT_MS, TURN_TIMEOUT_MS,
};
use std::collections::HashMap;
use std::collections::HashSet;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration, Instant};

use crate::channel::{
    CallResponse, PlayerAction, PlayerCommand, SessionControl, SessionEvent, TurnAction,
};

pub struct GameSession {
    pub game: GameState,
    pub rng: StdRng,
    pub event_txs: [mpsc::Sender<SessionEvent>; 4],
    pub action_tx: mpsc::Sender<PlayerCommand>,
    pub action_rx: mpsc::Receiver<PlayerCommand>,
    control_rx: mpsc::Receiver<SessionControl>,
    control_enabled: bool,
    action_forwarders: [Option<tokio::task::JoinHandle<()>>; 4],
}

impl GameSession {
    pub fn new(
        event_txs: [mpsc::Sender<SessionEvent>; 4],
        action_tx: mpsc::Sender<PlayerCommand>,
        action_rx: mpsc::Receiver<PlayerCommand>,
    ) -> Self {
        let (_, control_rx) = mpsc::channel(1);
        Self {
            game: GameState::new(),
            rng: StdRng::from_entropy(),
            event_txs,
            action_tx,
            action_rx,
            control_rx,
            control_enabled: false,
            action_forwarders: std::array::from_fn(|_| None),
        }
    }

    pub fn new_with_control(
        event_txs: [mpsc::Sender<SessionEvent>; 4],
        action_tx: mpsc::Sender<PlayerCommand>,
        action_rx: mpsc::Receiver<PlayerCommand>,
        control_rx: mpsc::Receiver<SessionControl>,
    ) -> Self {
        Self {
            game: GameState::new(),
            rng: StdRng::from_entropy(),
            event_txs,
            action_tx,
            action_rx,
            control_rx,
            control_enabled: true,
            action_forwarders: std::array::from_fn(|_| None),
        }
    }

    pub async fn run(&mut self) {
        self.game.start_round(&mut self.rng);
        self.broadcast_state().await;

        loop {
            if self.game.is_game_over() {
                self.broadcast(SessionEvent::GameOver {
                    scores: self.scores(),
                    ranking: self
                        .game
                        .ranking_at_game_end
                        .unwrap_or_else(|| self.game.final_ranking()),
                })
                .await;
                return;
            }

            match self.game.phase {
                GamePhase::DrawPhase => {
                    if self.game.draw().is_err() {
                        self.handle_round_end().await;
                        continue;
                    }
                    self.broadcast_state().await;
                }
                GamePhase::ActionPhase => {
                    let player = self.game.current_player;
                    let can_tsumo = self.game.check_tsumo(player).is_some();
                    let can_riichi = self.game.can_declare_riichi(player);
                    let riichi_options = self.riichi_options(player);
                    let discard_options = if self.game.players[player.0].is_riichi {
                        self.game.drawn_tile.into_iter().collect()
                    } else {
                        let mut options = self.game.players[player.0].hand.tiles().to_vec();
                        if let Some(drawn) = self.game.drawn_tile {
                            options.push(drawn);
                        }
                        options
                    };
                    let ankan_options = self.game.get_ankan_options(player);
                    let kakan_options = self.game.get_kakan_options(player);
                    let can_kyuushu = self.game.can_declare_kyuushu(player);

                    self.send_to(
                        player,
                        SessionEvent::ActionRequired {
                            can_tsumo,
                            can_riichi,
                            riichi_options,
                            discard_options,
                            ankan_options,
                            kakan_options,
                            can_kyuushu,
                        },
                    )
                    .await;

                    match self.wait_for_turn_action(player).await {
                        Some(action) => {
                            self.apply_turn_action(player, action).await;
                        }
                        None => return,
                    }
                }
                GamePhase::ResponsePhase { .. } | GamePhase::ChankanResponse { .. } => {
                    self.handle_response_phase().await;
                }
                GamePhase::RoundOver => {
                    self.handle_round_end().await;
                }
            }
        }
    }

    /// 替换指定座位的事件连接，并立即发送该玩家视角的完整状态快照。
    ///
    /// 玩家身份由座位保持，重连客户端不会获得其他玩家的手牌；状态广播
    /// 仍然按照现有的逐玩家视角逻辑生成。
    pub async fn reconnect_player(
        &mut self,
        player: PlayerId,
        event_tx: mpsc::Sender<SessionEvent>,
        mut action_rx: mpsc::Receiver<PlayerCommand>,
    ) {
        self.event_txs[player.0] = event_tx;
        if let Some(forwarder) = self.action_forwarders[player.0].take() {
            forwarder.abort();
        }
        let action_tx = self.action_tx.clone();
        self.action_forwarders[player.0] = Some(tokio::spawn(async move {
            while let Some(message) = action_rx.recv().await {
                if message.player == player && action_tx.send(message).await.is_err() {
                    break;
                }
            }
        }));
        self.broadcast_state().await;
        self.send_current_action_prompt(player).await;
    }

    /// 向重连玩家重新发送当前阶段的操作提示。
    async fn send_current_action_prompt(&self, player: PlayerId) {
        match self.game.phase {
            GamePhase::ActionPhase if self.game.current_player == player => {
                let _ = self.event_txs[player.0]
                    .send(SessionEvent::ActionRequired {
                        can_tsumo: self.game.check_tsumo(player).is_some(),
                        can_riichi: self.game.can_declare_riichi(player),
                        riichi_options: self.riichi_options(player),
                        discard_options: if self.game.players[player.0].is_riichi {
                            self.game.drawn_tile.into_iter().collect()
                        } else {
                            let mut options = self.game.players[player.0].hand.tiles().to_vec();
                            if let Some(drawn) = self.game.drawn_tile {
                                options.push(drawn);
                            }
                            options
                        },
                        ankan_options: self.game.get_ankan_options(player),
                        kakan_options: self.game.get_kakan_options(player),
                        can_kyuushu: self.game.can_declare_kyuushu(player),
                    })
                    .await;
            }
            GamePhase::ResponsePhase { .. } | GamePhase::ChankanResponse { .. } => {
                let options: Vec<_> = self
                    .game
                    .get_call_options()
                    .into_iter()
                    .filter(|option| option.player == player)
                    .collect();
                if !options.is_empty() {
                    let _ = self.event_txs[player.0]
                        .send(SessionEvent::CallRequired { options })
                        .await;
                }
            }
            _ => {}
        }
    }

    async fn apply_turn_action(&mut self, player: PlayerId, action: TurnAction) {
        let action = match action {
            TurnAction::Riichi => match self.riichi_options(player).first().copied() {
                Some(tile) => TurnAction::RiichiDiscard(tile),
                None => {
                    self.recover_invalid_turn(player, "当前没有合法的立直弃牌".to_string())
                        .await;
                    return;
                }
            },
            action => action,
        };

        let validation_action = match &action {
            TurnAction::Discard(tile) => EngineTurnAction::Discard(*tile),
            TurnAction::Tsumo => EngineTurnAction::Tsumo,
            TurnAction::RiichiDiscard(tile) => EngineTurnAction::RiichiDiscard(*tile),
            TurnAction::Ankan(tile) => EngineTurnAction::Ankan(*tile),
            TurnAction::Kakan(index, tile) => EngineTurnAction::Kakan(*index, *tile),
            TurnAction::KyuushuKyuuhai => EngineTurnAction::KyuushuKyuuhai,
            TurnAction::Riichi => unreachable!("Riichi 已在上方规范化"),
        };
        if let Err(error) = self
            .game
            .validate_action(player, &LegalAction::Turn(validation_action))
        {
            self.recover_invalid_turn(player, error.to_string()).await;
            return;
        }

        match action {
            TurnAction::Discard(tile) => {
                if let Err(error) = self.game.execute_action(EngineTurnAction::Discard(tile)) {
                    self.send_to(player, SessionEvent::Error(error.to_string()))
                        .await;
                    return;
                }
                self.broadcast_state().await;
                self.handle_after_turn().await;
            }
            TurnAction::Tsumo => {
                if let Err(error) = self.game.execute_action(EngineTurnAction::Tsumo) {
                    self.send_to(player, SessionEvent::Error(error.to_string()))
                        .await;
                    return;
                }
                self.broadcast_state().await;
                self.handle_round_end().await;
            }
            TurnAction::RiichiDiscard(tile) => {
                if let Err(error) = self
                    .game
                    .execute_action(EngineTurnAction::RiichiDiscard(tile))
                {
                    self.send_to(player, SessionEvent::Error(error.to_string()))
                        .await;
                    return;
                }
                self.broadcast_state().await;
                self.handle_after_turn().await;
            }
            TurnAction::Riichi => {
                if let Some(tile) = self.riichi_options(player).first().copied() {
                    if let Err(error) = self
                        .game
                        .execute_action(EngineTurnAction::RiichiDiscard(tile))
                    {
                        self.send_to(player, SessionEvent::Error(error.to_string()))
                            .await;
                        return;
                    }
                    self.broadcast_state().await;
                    self.handle_after_turn().await;
                } else {
                    self.send_to(player, SessionEvent::Error("当前没有合法的立直弃牌".into()))
                        .await;
                }
            }
            TurnAction::Ankan(tile) => {
                if let Err(error) = self.game.execute_action(EngineTurnAction::Ankan(tile)) {
                    self.send_to(player, SessionEvent::Error(error.to_string()))
                        .await;
                    return;
                }
                self.broadcast_state().await;
            }
            TurnAction::Kakan(index, tile) => {
                if let Err(error) = self
                    .game
                    .execute_action(EngineTurnAction::Kakan(index, tile))
                {
                    self.send_to(player, SessionEvent::Error(error.to_string()))
                        .await;
                    return;
                }
                self.broadcast_state().await;
                self.handle_after_turn().await;
            }
            TurnAction::KyuushuKyuuhai => {
                if let Err(error) = self.game.execute_action(EngineTurnAction::KyuushuKyuuhai) {
                    self.send_to(player, SessionEvent::Error(error.to_string()))
                        .await;
                    return;
                }
                self.broadcast_state().await;
                self.handle_round_end().await;
            }
        }
    }

    /// 非法行动的兜底：报告错误后，从引擎生成的合法行动中选择摸切。
    /// 这样 AI 或客户端即使提交了过期/非法动作，也不会让行动阶段反复重试。
    async fn recover_invalid_turn(&mut self, player: PlayerId, error: String) {
        self.send_to(player, SessionEvent::Error(error)).await;

        let fallback = self
            .game
            .legal_actions(player)
            .into_iter()
            .find_map(|action| {
                let LegalAction::Turn(EngineTurnAction::Discard(tile)) = action else {
                    return None;
                };
                let action = LegalAction::Turn(EngineTurnAction::Discard(tile));
                self.game
                    .validate_action(player, &action)
                    .ok()
                    .map(|_| tile)
            });

        let Some(tile) = fallback else {
            self.broadcast(SessionEvent::Error(
                "没有可执行的安全弃牌，安全结束本局".into(),
            ))
            .await;
            self.game.resolve_round_end(RoundEndReason::ExhaustiveDraw);
            self.handle_round_end().await;
            return;
        };

        if let Err(fallback_error) = self.game.execute_action(EngineTurnAction::Discard(tile)) {
            self.broadcast(SessionEvent::Error(format!(
                "安全弃牌执行失败: {}",
                fallback_error
            )))
            .await;
            return;
        }
        self.broadcast_state().await;
        self.handle_after_turn().await;
    }

    async fn handle_after_turn(&mut self) {
        loop {
            let phase = self.game.phase.clone();
            match phase {
                GamePhase::ResponsePhase { discarder, .. }
                | GamePhase::ChankanResponse {
                    kakan_player: discarder,
                    ..
                } => {
                    self.handle_response_phase_with_discarder(discarder).await;
                    if matches!(self.game.phase, GamePhase::DrawPhase) {
                        if self.game.draw().is_err() {
                            self.handle_round_end().await;
                        }
                        self.broadcast_state().await;
                        return;
                    }
                    if matches!(self.game.phase, GamePhase::RoundOver) {
                        self.handle_round_end().await;
                        return;
                    }
                }
                _ => return,
            }
        }
    }

    async fn handle_response_phase(&mut self) {
        let phase = self.game.phase.clone();
        let discarder = match phase {
            GamePhase::ResponsePhase { discarder, .. } => discarder,
            GamePhase::ChankanResponse { kakan_player, .. } => kakan_player,
            _ => return,
        };
        self.handle_response_phase_with_discarder(discarder).await;
    }

    async fn handle_response_phase_with_discarder(&mut self, discarder: PlayerId) {
        let call_options = self.game.get_call_options();

        let mut accepted_call: Option<(PlayerId, ResponseAction)> = None;
        let mut ron_players = Vec::new();

        let mut eligible = Vec::new();
        for idx in 0..4u8 {
            let pid = PlayerId(idx as usize);
            if pid == discarder {
                continue;
            }

            let player_options: Vec<_> = call_options
                .iter()
                .filter(|o| o.player == pid)
                .cloned()
                .collect();

            if player_options.is_empty() {
                continue;
            }

            eligible.push(pid);
            self.send_to(
                pid,
                SessionEvent::CallRequired {
                    options: player_options,
                },
            )
            .await;
        }

        let responses = self
            .wait_for_prioritized_call_responses(&call_options, &eligible)
            .await;
        for (pid, response) in responses {
            let player_options: Vec<_> = call_options
                .iter()
                .filter(|o| o.player == pid)
                .cloned()
                .collect();

            let has_ron = player_options
                .iter()
                .any(|o| matches!(o.call_type, CallType::Ron));

            if has_ron && !matches!(response, CallResponse::Ron) {
                let _ = self.game.record_response_pass(pid);
            }

            match response {
                CallResponse::Ron if has_ron => {
                    ron_players.push(pid);
                }
                CallResponse::Pon { hand_tiles } => {
                    let candidate = ResponseAction::Pon { hand_tiles };
                    if should_replace_call(accepted_call.as_ref(), pid, &candidate, discarder) {
                        accepted_call = Some((pid, candidate));
                    }
                }
                CallResponse::Chi { hand_tiles } => {
                    let candidate = ResponseAction::Chi { hand_tiles };
                    if should_replace_call(accepted_call.as_ref(), pid, &candidate, discarder) {
                        accepted_call = Some((pid, candidate));
                    }
                }
                CallResponse::Minkan { hand_tiles } => {
                    let candidate = ResponseAction::Minkan { hand_tiles };
                    if should_replace_call(accepted_call.as_ref(), pid, &candidate, discarder) {
                        accepted_call = Some((pid, candidate));
                    }
                }
                _ => {
                    let _ = self.game.record_response_pass(pid);
                }
            }
        }

        if !ron_players.is_empty() {
            let mut ordered_winners = Vec::new();
            for offset in 1..4 {
                let candidate = PlayerId((discarder.0 + offset) % 4);
                if ron_players.contains(&candidate) {
                    ordered_winners.push(candidate);
                }
            }
            let max_winners = if ALLOW_TRIPLE_RON {
                3
            } else if ALLOW_DOUBLE_RON {
                2
            } else {
                1
            };
            ordered_winners.truncate(max_winners);
            if ordered_winners.len() > 1 {
                if let Err(error) = self.game.execute_multiple_ron(&ordered_winners) {
                    for winner in &ordered_winners {
                        self.send_to(*winner, SessionEvent::Error(error.to_string()))
                            .await;
                    }
                    return;
                }
            } else if let Some(&pid) = ordered_winners.first() {
                if let Err(error) = self.game.execute_call(pid, ResponseAction::Ron) {
                    self.send_to(pid, SessionEvent::Error(error.to_string()))
                        .await;
                    return;
                }
            }
            self.broadcast_state().await;
            if matches!(self.game.phase, GamePhase::RoundOver) {
                self.handle_round_end().await;
            }
            return;
        }

        if let Some((pid, action)) = accepted_call {
            if let Err(error) = self.game.execute_call(pid, action) {
                self.send_to(pid, SessionEvent::Error(error.to_string()))
                    .await;
                return;
            }
            self.broadcast_state().await;
            if matches!(self.game.phase, GamePhase::RoundOver) {
                self.handle_round_end().await;
            }
            return;
        }

        if let Err(error) = self.game.complete_response_pass() {
            self.broadcast(SessionEvent::Error(error.to_string())).await;
            return;
        }
        self.broadcast_state().await;

        if matches!(self.game.phase, GamePhase::DrawPhase) {
            if self.game.draw().is_err() {
                self.handle_round_end().await;
            }
            self.broadcast_state().await;
        } else if matches!(self.game.phase, GamePhase::RoundOver) {
            self.handle_round_end().await;
        }
    }

    async fn handle_round_end(&mut self) {
        self.broadcast_state().await;

        self.broadcast(SessionEvent::RoundResult {
            reason: self.round_end_reason(),
            win_details: self.round_win_details(),
            point_changes: self.game.round_point_changes(),
            scores: self.scores(),
        })
        .await;
        tokio::time::sleep(Duration::from_secs(5)).await;

        if self.game.is_game_over() {
            self.broadcast(SessionEvent::GameOver {
                scores: self.scores(),
                ranking: self
                    .game
                    .ranking_at_game_end
                    .unwrap_or_else(|| self.game.final_ranking()),
            })
            .await;
        } else {
            self.game.start_round(&mut self.rng);
            self.broadcast_state().await;
        }
    }

    fn round_end_reason(&self) -> String {
        self.game
            .events
            .iter()
            .rev()
            .find_map(|event| match event {
                GameEvent::RoundEnded { reason } => Some(match reason {
                    RoundEndReason::Win { is_tsumo: true, .. } => "自摸".to_string(),
                    RoundEndReason::Win {
                        is_tsumo: false, ..
                    }
                    | RoundEndReason::MultiWin { .. } => "荣和".to_string(),
                    RoundEndReason::ExhaustiveDraw => "流局".to_string(),
                    RoundEndReason::KyuushuKyuuhai
                    | RoundEndReason::SuufonRenda
                    | RoundEndReason::SuuchaRiichi
                    | RoundEndReason::SuuKantsu => "途中流局".to_string(),
                }),
                _ => None,
            })
            .unwrap_or_else(|| "局结束".to_string())
    }

    fn round_win_details(&self) -> Vec<String> {
        self.game
            .events
            .iter()
            .filter_map(|event| match event {
                GameEvent::PlayerWon { yaku_names, .. } => Some(yaku_names.clone()),
                _ => None,
            })
            .flatten()
            .collect()
    }

    async fn wait_for_turn_action(&mut self, expected: PlayerId) -> Option<TurnAction> {
        let deadline = Instant::now() + Duration::from_millis(TURN_TIMEOUT_MS);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let received = timeout(remaining, async {
                if self.control_enabled {
                    tokio::select! {
                        command = self.action_rx.recv() => command,
                        control = self.control_rx.recv() => {
                            if let Some(control) = control {
                                self.reconnect_player(control.player, control.event_tx, control.action_rx).await;
                            } else {
                                self.control_enabled = false;
                            }
                            None
                        }
                    }
                } else {
                    self.action_rx.recv().await
                }
            })
            .await;
            match received {
                Ok(Some(PlayerCommand {
                    player: pid,
                    action: PlayerAction::TurnAction(action),
                })) if pid == expected => return Some(action),
                Ok(Some(_)) => continue,
                Ok(None) | Err(_) => break,
            }
        }

        // 超时默认摸切；若状态异常没有摸牌，则选择当前手牌最后一张，
        // 后续仍由规则引擎进行最终合法性校验。
        self.game.drawn_tile.map(TurnAction::Discard).or_else(|| {
            self.game.players[expected.0]
                .hand
                .tiles()
                .last()
                .copied()
                .map(TurnAction::Discard)
        })
    }

    /// 收集同步响应：低优先级响应可以先到达，但在更高优先级尚未
    /// 完成前只缓存、不处理。更高优先级动作一旦提交，低优先级响应
    /// 全部失效；只有更高优先级全部 Pass/超时后才进入下一层。
    async fn wait_for_prioritized_call_responses(
        &mut self,
        call_options: &[riichi_core::game::CallOption],
        eligible: &[PlayerId],
    ) -> Vec<(PlayerId, CallResponse)> {
        if eligible.is_empty() {
            return Vec::new();
        }

        let deadline = Instant::now() + Duration::from_millis(RESPONSE_TIMEOUT_MS);
        let eligible_set: HashSet<PlayerId> = eligible.iter().copied().collect();
        let mut responses: HashMap<PlayerId, CallResponse> = HashMap::new();

        // 同一玩家只有一个合法的非 Pass 动作；按 CallOption 判断其
        // 当前是否属于某个优先级层。荣和层可能有多个玩家，其他层
        // 在合法牌局中最多只有一个玩家。
        let priority_players = |priority: u8| -> Vec<PlayerId> {
            eligible
                .iter()
                .copied()
                .filter(|pid| {
                    call_options.iter().any(|option| {
                        option.player == *pid
                            && matches!(
                                (&option.call_type, priority),
                                (CallType::Ron, 0)
                                    | (CallType::Minkan { .. }, 1)
                                    | (CallType::Pon { .. }, 2)
                                    | (CallType::Chi { .. }, 3)
                            )
                    })
                })
                .collect()
        };

        for priority in 0..=3u8 {
            let players = priority_players(priority);
            if players.is_empty() {
                continue;
            }

            let pending: HashSet<PlayerId> = players
                .iter()
                .copied()
                .filter(|pid| !responses.contains_key(pid))
                .collect();
            while !pending.iter().all(|pid| responses.contains_key(pid)) {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                let received = timeout(remaining, async {
                    if self.control_enabled {
                        tokio::select! {
                            command = self.action_rx.recv() => command,
                            control = self.control_rx.recv() => {
                                if let Some(control) = control {
                                    self.reconnect_player(control.player, control.event_tx, control.action_rx).await;
                                } else {
                                    self.control_enabled = false;
                                }
                                None
                            }
                        }
                    } else {
                        self.action_rx.recv().await
                    }
                }).await;
                match received {
                    Ok(Some(PlayerCommand {
                        player: pid,
                        action: PlayerAction::CallResponse(response),
                    })) if eligible_set.contains(&pid) && !responses.contains_key(&pid) => {
                        responses.insert(pid, response);
                    }
                    Ok(Some(_)) => continue,
                    Ok(None) | Err(_) => break,
                }
            }

            // 超时只补齐当前优先级尚未响应的玩家；缓存的低优先级
            // 响应会在没有更高优先级动作时进入后续层。
            for &player in &players {
                responses.entry(player).or_insert(CallResponse::Pass);
            }

            let has_action = responses.iter().any(|(pid, response)| {
                players.contains(pid)
                    && matches!(
                        (priority, response),
                        (0, CallResponse::Ron)
                            | (1, CallResponse::Minkan { .. })
                            | (2, CallResponse::Pon { .. })
                            | (3, CallResponse::Chi { .. })
                    )
            });
            if has_action {
                break;
            }
        }

        eligible
            .iter()
            .map(|&player| {
                (
                    player,
                    responses.remove(&player).unwrap_or(CallResponse::Pass),
                )
            })
            .collect()
    }

    fn riichi_options(&self, player: PlayerId) -> Vec<Tile> {
        self.game.get_riichi_discard_options(player)
    }

    async fn broadcast_state(&self) {
        let players = &self.game.players;
        let points = [
            players[0].points,
            players[1].points,
            players[2].points,
            players[3].points,
        ];
        let discards = [
            players[0].discards.clone(),
            players[1].discards.clone(),
            players[2].discards.clone(),
            players[3].discards.clone(),
        ];
        let pending_discard = match self.game.phase {
            GamePhase::ResponsePhase {
                discarded_tile,
                discarder,
            } => Some((discarder, discarded_tile)),
            _ => None,
        };
        let melds_count = [
            players[0].melds.len(),
            players[1].melds.len(),
            players[2].melds.len(),
            players[3].melds.len(),
        ];
        let melds = [
            players[0].melds.clone(),
            players[1].melds.clone(),
            players[2].melds.clone(),
            players[3].melds.clone(),
        ];
        let hand_counts = [
            players[0].hand.len(),
            players[1].hand.len(),
            players[2].hand.len(),
            players[3].hand.len(),
        ];
        let winds = [
            players[0].wind,
            players[1].wind,
            players[2].wind,
            players[3].wind,
        ];
        let is_riichi = [
            players[0].is_riichi,
            players[1].is_riichi,
            players[2].is_riichi,
            players[3].is_riichi,
        ];

        #[allow(clippy::needless_range_loop)]
        for idx in 0..4 {
            let pid = PlayerId(idx);
            let my_hand = {
                let mut tiles = players[idx].hand.tiles().to_vec();
                if self.game.current_player == pid {
                    if let Some(drawn) = self.game.drawn_tile {
                        tiles.push(drawn);
                    }
                }
                tiles
            };
            let event = SessionEvent::StateUpdate {
                phase: self.game.phase.clone(),
                current_player: self.game.current_player,
                pending_discard,
                drawn_tile: if self.game.current_player == pid {
                    self.game.drawn_tile
                } else {
                    None
                },
                hand_tiles: my_hand,
                hand_count: players[idx].hand.len(),
                hand_counts,
                points,
                winds,
                is_riichi,
                discards: discards.clone(),
                melds_count,
                melds: melds.clone(),
                dora: self.game.dora.clone(),
                remaining_tiles: self.game.remaining_tiles(),
                round: self.game.round,
                honba: self.game.honba,
                riichi_sticks: self.game.riichi_sticks,
                tenpai_info: self.game.tenpai_info(pid),
            };
            let _ = self.event_txs[idx].send(event).await;
        }
    }

    async fn broadcast(&self, event: SessionEvent) {
        for tx in &self.event_txs {
            let _ = tx.send(event.clone()).await;
        }
    }

    async fn send_to(&self, player: PlayerId, event: SessionEvent) {
        let _ = self.event_txs[player.0].send(event).await;
    }

    fn scores(&self) -> [i32; 4] {
        [
            self.game.players[0].points,
            self.game.players[1].points,
            self.game.players[2].points,
            self.game.players[3].points,
        ]
    }
}

fn should_replace_call(
    current: Option<&(PlayerId, ResponseAction)>,
    candidate_player: PlayerId,
    candidate: &ResponseAction,
    discarder: PlayerId,
) -> bool {
    let candidate_key = call_priority_key(candidate_player, candidate, discarder);
    current
        .map(|(player, action)| candidate_key > call_priority_key(*player, action, discarder))
        .unwrap_or(true)
}

fn call_priority_key(player: PlayerId, action: &ResponseAction, discarder: PlayerId) -> (u8, u8) {
    let priority = match action {
        ResponseAction::Minkan { .. } => 3,
        ResponseAction::Pon { .. } => 2,
        ResponseAction::Chi { .. } => 1,
        ResponseAction::Pass | ResponseAction::Ron => 0,
    };
    let distance = ((player.0 + 4 - discarder.0) % 4) as u8;
    (priority, 4 - distance)
}

#[cfg(test)]
mod tests {
    use super::{call_priority_key, should_replace_call, GameSession};
    use crate::{create_player_pair, SessionEvent};
    use riichi_core::game::ResponseAction;
    use riichi_core::player::PlayerId;
    use riichi_core::tile::Tile;
    use tokio::sync::mpsc;

    #[test]
    fn pon_beats_chi_and_nearer_call_wins_same_priority() {
        let chi = ResponseAction::Chi {
            hand_tiles: [Tile::from_raw(0), Tile::from_raw(4)],
        };
        let pon = ResponseAction::Pon {
            hand_tiles: [Tile::from_raw(8), Tile::from_raw(12)],
        };
        let discarder = PlayerId(0);
        assert!(should_replace_call(
            Some(&(PlayerId(1), chi.clone())),
            PlayerId(2),
            &pon,
            discarder,
        ));

        let farther_pon = ResponseAction::Pon {
            hand_tiles: [Tile::from_raw(16), Tile::from_raw(20)],
        };
        assert!(!should_replace_call(
            Some(&(PlayerId(1), pon)),
            PlayerId(2),
            &farther_pon,
            discarder,
        ));
        assert_eq!(call_priority_key(PlayerId(1), &chi, discarder).0, 1);
    }

    #[tokio::test]
    async fn reconnect_sends_state_and_current_action_prompt() {
        let mut pairs = Vec::new();
        for index in 0..4 {
            pairs.push(create_player_pair(PlayerId(index)));
        }
        let event_txs = std::array::from_fn(|index| pairs[index].0.event_tx.clone());
        let (action_tx, action_rx) = mpsc::channel(8);
        let mut session = GameSession::new(event_txs, action_tx, action_rx);
        let (replacement_tx, mut replacement_rx) = mpsc::channel(8);
        let (_, replacement_action_rx) = mpsc::channel(8);

        session
            .reconnect_player(PlayerId(0), replacement_tx, replacement_action_rx)
            .await;

        assert!(matches!(
            replacement_rx.recv().await,
            Some(SessionEvent::StateUpdate { .. })
        ));
        assert!(matches!(
            replacement_rx.recv().await,
            Some(SessionEvent::ActionRequired { .. })
        ));
    }
}
