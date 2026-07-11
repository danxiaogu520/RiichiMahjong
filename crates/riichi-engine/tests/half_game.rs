use std::collections::HashSet;

use rand::SeedableRng;
use riichi_core::game::{GameEvent, TurnAction};
use riichi_core::player::PlayerId;
use riichi_core::tile::Tile;
use riichi_engine::game::{GamePhase, GameState};
use riichi_logic::analysis::analyze_wait_tiles;
use riichi_logic::shanten::ShantenCalculator;

fn choose_full_efficiency_discard(state: &GameState, player: PlayerId) -> Tile {
    let mut candidates = state.players[player.0].hand.tiles().to_vec();
    if let Some(drawn) = state.drawn_tile {
        candidates.push(drawn);
    }

    let calculator = ShantenCalculator::new();
    candidates
        .into_iter()
        .min_by_key(|&discard| {
            let mut after = state.players[player.0].hand.tiles().to_vec();
            if let Some(drawn) = state.drawn_tile {
                after.push(drawn);
            }
            let index = after
                .iter()
                .position(|&tile| tile == discard)
                .expect("候选弃牌必须存在于摸牌后手牌");
            after.remove(index);

            let shanten = calculator.calculate(&after);
            let waits = analyze_wait_tiles(&after).len();
            // 先最小化向听，再最大化听牌种类；最后用牌型编号保持确定性。
            (shanten, std::cmp::Reverse(waits), discard.tile_type().0)
        })
        .expect("行动阶段必须存在可弃牌")
}

fn execute_all_closed_ai_responses(state: &mut GameState) {
    let options = state.get_call_options();
    let mut responders = HashSet::new();
    let mut ron_winners = Vec::new();
    for option in options {
        if !responders.insert(option.player) {
            continue;
        }
        if matches!(option.call_type, riichi_core::game::CallType::Ron) {
            ron_winners.push(option.player);
        }
    }

    match state.phase {
        GamePhase::ResponsePhase { .. } if !ron_winners.is_empty() => {
            state.execute_multiple_ron(&ron_winners).unwrap();
        }
        GamePhase::ChankanResponse { .. } if !ron_winners.is_empty() => {
            state
                .execute_call(ron_winners[0], riichi_core::game::ResponseAction::Ron)
                .unwrap();
        }
        _ => {
            for player in responders {
                state.record_response_pass(player).unwrap();
            }
            state.complete_response_pass().unwrap();
            if matches!(state.phase, GamePhase::DrawPhase) {
                let _ = state.draw();
            }
        }
    }
}

/// 用最小确定性客户端跑完一个不鸣牌半庄，验证牌局状态机不会卡在
/// 响应阶段、牌墙耗尽或局数推进边界。
#[test]
fn local_auto_pass_game_finishes_half_game() {
    let mut state = GameState::new();
    let mut rng = rand::rngs::StdRng::seed_from_u64(20260711);
    let mut action_count = 0usize;

    while !state.is_game_over() {
        state.start_round(&mut rng);
        let round = state.round;
        let honba = state.honba;

        while !matches!(state.phase, GamePhase::RoundOver) {
            action_count += 1;
            assert!(action_count < 200_000, "半庄状态机疑似没有推进");

            match state.phase.clone() {
                GamePhase::ActionPhase => {
                    let player = state.current_player;
                    if state.check_tsumo(player).is_some() {
                        state.execute_action(TurnAction::Tsumo).unwrap();
                    } else {
                        let drawn = state.drawn_tile.expect("行动阶段必须存在摸牌");
                        state.execute_action(TurnAction::Discard(drawn)).unwrap();
                    }
                }
                GamePhase::ResponsePhase { .. } | GamePhase::ChankanResponse { .. } => {
                    let responders: HashSet<PlayerId> = state
                        .get_call_options()
                        .into_iter()
                        .map(|option| option.player)
                        .collect();
                    for player in responders {
                        state.record_response_pass(player).unwrap();
                    }
                    state.complete_response_pass().unwrap();
                    if matches!(state.phase, GamePhase::DrawPhase) {
                        let _ = state.draw();
                    }
                }
                GamePhase::DrawPhase => {
                    let _ = state.draw();
                }
                GamePhase::RoundOver => break,
            }
        }

        assert!(state.round > round || state.honba > honba || state.is_game_over());
    }

    assert_eq!(state.round, 9);
    assert!(state
        .event_history()
        .iter()
        .any(|event| matches!(event, GameEvent::RoundEnded { .. })));
}

/// 四个只会门清推进的牌效 AI：
/// - 优先自摸；
/// - 能立直就立直；
/// - 只按向听数和听牌种类选择弃牌；
/// - 永远不吃、碰、杠，但会荣和。
///
/// 该测试不是追求 AI 强度，而是用真实的和牌、立直、荣和和流局路径
/// 压测整个半庄状态机，并逐局检查结束事件与点数变化。
#[test]
fn four_closed_efficiency_ai_finishes_and_monitors_half_game() {
    let mut state = GameState::new();
    let mut rng = rand::rngs::StdRng::seed_from_u64(20260712);
    let mut action_count = 0usize;
    let mut round_count = 0usize;
    let mut tsumo_count = 0usize;
    let mut ron_count = 0usize;
    let mut exhaustive_count = 0usize;

    while !state.is_game_over() {
        state.start_round(&mut rng);
        let round = state.round;
        let honba = state.honba;
        let riichi_sticks_start = state.riichi_sticks;
        let round_start_points: [i32; 4] = std::array::from_fn(|index| state.players[index].points);
        let round_history_start = state.event_history().len();

        while !matches!(state.phase, GamePhase::RoundOver) {
            action_count += 1;
            assert!(action_count < 300_000, "四 AI 对局疑似没有推进");

            match state.phase.clone() {
                GamePhase::ActionPhase => {
                    let player = state.current_player;
                    if state.check_tsumo(player).is_some() {
                        state.execute_action(TurnAction::Tsumo).unwrap();
                    } else if state.players[player.0].is_riichi {
                        let drawn = state.drawn_tile.expect("立直后行动阶段必须摸牌");
                        state.execute_action(TurnAction::Discard(drawn)).unwrap();
                    } else if let Some(&discard) = state.get_riichi_discard_options(player).first()
                    {
                        state
                            .execute_action(TurnAction::RiichiDiscard(discard))
                            .unwrap();
                    } else {
                        let discard = choose_full_efficiency_discard(&state, player);
                        state.execute_action(TurnAction::Discard(discard)).unwrap();
                    }
                }
                GamePhase::ResponsePhase { .. } | GamePhase::ChankanResponse { .. } => {
                    execute_all_closed_ai_responses(&mut state);
                }
                GamePhase::DrawPhase => {
                    state.draw().unwrap();
                }
                GamePhase::RoundOver => break,
            }
        }

        let round_events = state.event_history()[round_history_start..].to_vec();
        let ended = round_events
            .iter()
            .find_map(|event| match event {
                GameEvent::RoundEnded { reason } => Some(reason),
                _ => None,
            })
            .expect("每小局必须产生 RoundEnded");
        match ended {
            riichi_core::game::RoundEndReason::Win { is_tsumo, .. } => {
                if *is_tsumo {
                    tsumo_count += 1;
                } else {
                    ron_count += 1;
                }
            }
            riichi_core::game::RoundEndReason::MultiWin { .. } => ron_count += 1,
            riichi_core::game::RoundEndReason::ExhaustiveDraw => exhaustive_count += 1,
            _ => {}
        }

        let end_points: [i32; 4] = std::array::from_fn(|index| state.players[index].points);
        let point_delta: i32 = end_points
            .iter()
            .zip(round_start_points.iter())
            .map(|(end, start)| end - start)
            .sum();
        // 立直棒在流局时留场，在和牌时由赢家取得；点数变化应等于
        // 本局开始与结束时场上立直棒池的反向变化。
        let expected_external_loss =
            (riichi_sticks_start as i32 - state.riichi_sticks as i32) * 1000;
        assert_eq!(
            point_delta, expected_external_loss,
            "第 {round} 局点数不守恒"
        );
        assert!(state.players.iter().all(|player| player.melds.is_empty()));
        round_count += 1;
        println!(
            "第 {round} 局结束: {ended:?}, 点数变化: {:?}",
            end_points
                .iter()
                .zip(round_start_points.iter())
                .map(|(end, start)| end - start)
                .collect::<Vec<_>>()
        );
        assert!(state.round > round || state.honba > honba || state.is_game_over());
    }

    assert_eq!(state.round, 9);
    assert!(round_count >= 8);
    assert!(tsumo_count + ron_count + exhaustive_count > 0);
    println!(
        "半庄完成: 小局={}, 自摸={}, 荣和={}, 流局={}, 动作数={}",
        round_count, tsumo_count, ron_count, exhaustive_count, action_count
    );
}
