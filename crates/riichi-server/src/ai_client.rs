use riichi_core::player::PlayerId;
use riichi_core::tile::Tile;
use riichi_engine::game::GamePhase;

use crate::channel::{ActionMsg, CallResponseMsg, ClientHandle, PlayerAction, ServerEvent, TurnActionMsg};

struct AiState {
    hand_tiles: Vec<Tile>,
    phase: GamePhase,
    current_player: PlayerId,
    can_tsumo: bool,
    can_riichi: bool,
}

pub async fn run_ai_client(mut handle: ClientHandle) {
    let mut state = AiState {
        hand_tiles: Vec::new(),
        phase: GamePhase::DrawPhase,
        current_player: PlayerId(0),
        can_tsumo: false,
        can_riichi: false,
    };

    while let Some(event) = handle.event_rx.recv().await {
        match event {
            ServerEvent::StateUpdate {
                phase, current_player, hand_tiles, ..
            } => {
                state.phase = phase;
                state.current_player = current_player;
                state.hand_tiles = hand_tiles;
            }
            ServerEvent::ActionRequired { can_tsumo, can_riichi } => {
                state.can_tsumo = can_tsumo;
                state.can_riichi = can_riichi;

                let action = decide_turn(&handle.id, &state);
                let _ = handle.action_tx.send(action).await;
            }
            ServerEvent::CallRequired { .. } => {
                let msg: ActionMsg = (handle.id, PlayerAction::CallResponse(CallResponseMsg::Pass));
                let _ = handle.action_tx.send(msg).await;
            }
            ServerEvent::GameOver { .. } => break,
        }
    }
}

fn decide_turn(player: &PlayerId, state: &AiState) -> ActionMsg {
    if state.can_tsumo {
        return (*player, PlayerAction::TurnAction(TurnActionMsg::Tsumo));
    }
    if state.can_riichi {
        return (*player, PlayerAction::TurnAction(TurnActionMsg::Riichi));
    }
    let tile = state.hand_tiles.last().copied()
        .unwrap_or_else(|| Tile::from_raw(0));
    (*player, PlayerAction::TurnAction(TurnActionMsg::Discard(tile)))
}
