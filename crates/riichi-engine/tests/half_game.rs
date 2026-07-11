use std::collections::HashSet;

use rand::SeedableRng;
use riichi_core::game::{GameEvent, TurnAction};
use riichi_core::player::PlayerId;
use riichi_engine::game::{GamePhase, GameState};

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
                    state.draw().unwrap();
                }
                GamePhase::RoundOver => break,
            }
        }

        assert!(state.round > round || state.is_game_over());
    }

    assert_eq!(state.round, 9);
    assert!(state
        .event_history()
        .iter()
        .any(|event| matches!(event, GameEvent::RoundEnded { .. })));
}
