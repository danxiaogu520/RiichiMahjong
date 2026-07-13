use rand::Rng;
use riichi_core::game::ResponseAction;
use riichi_core::meld::Meld;
use riichi_core::player::PlayerId;
use riichi_core::tile::Tile;
use riichi_engine::game::GamePhase;
use riichi_logic::acceptance::VisibleTiles;
use riichi_logic::shanten::ShantenCalculator;
use std::time::Duration;
use tokio::time::sleep;

use crate::channel::{
    ActionMsg, CallResponseMsg, ClientHandle, PlayerAction, ServerEvent, TurnActionMsg,
};
use riichi_ai::{choose_discard, decide_call, decide_riichi};

struct AiState {
    hand_tiles: Vec<Tile>,
    phase: GamePhase,
    current_player: PlayerId,
    can_tsumo: bool,
    can_riichi: bool,
    visible: VisibleTiles,
    calculator: ShantenCalculator,
}

pub async fn run_ai_client(mut handle: ClientHandle) {
    let mut state = AiState {
        hand_tiles: Vec::new(),
        phase: GamePhase::DrawPhase,
        current_player: PlayerId(0),
        can_tsumo: false,
        can_riichi: false,
        visible: VisibleTiles::new(),
        calculator: ShantenCalculator::new(),
    };

    while let Some(event) = handle.event_rx.recv().await {
        match event {
            ServerEvent::StateUpdate {
                phase,
                current_player,
                hand_tiles,
                discards,
                melds,
                dora,
                ..
            } => {
                state.phase = phase;
                state.current_player = current_player;
                state.hand_tiles = hand_tiles;
                state.visible = build_visible_tiles(&melds, &discards, &dora, handle.id);
            }
            ServerEvent::ActionRequired {
                can_tsumo,
                can_riichi,
                riichi_options,
                discard_options,
                ..
            } => {
                state.can_tsumo = can_tsumo;
                state.can_riichi = can_riichi;

                wait_before_decision().await;
                let action = decide_turn(&handle.id, &mut state, &riichi_options, &discard_options);
                let _ = handle.action_tx.send(action).await;
            }
            ServerEvent::CallRequired { options } => {
                wait_before_decision().await;
                let response = decide_call(handle.id, &options).unwrap_or(ResponseAction::Pass);
                let response = match response {
                    ResponseAction::Ron => CallResponseMsg::Ron,
                    _ => CallResponseMsg::Pass,
                };
                let msg: ActionMsg = (handle.id, PlayerAction::CallResponse(response));
                let _ = handle.action_tx.send(msg).await;
            }
            ServerEvent::RoundResult { .. } => {}
            ServerEvent::GameOver { .. } => break,
            ServerEvent::Error(_) => {}
        }
    }
}

async fn wait_before_decision() {
    let delay_ms = rand::thread_rng().gen_range(1_000..=2_000);
    sleep(Duration::from_millis(delay_ms)).await;
}

fn decide_turn(
    player: &PlayerId,
    state: &mut AiState,
    riichi_options: &[Tile],
    discard_options: &[Tile],
) -> ActionMsg {
    if state.can_tsumo {
        return (*player, PlayerAction::TurnAction(TurnActionMsg::Tsumo));
    }
    if state.can_riichi {
        if let Some(tile) = decide_riichi(
            *player,
            &mut state.calculator,
            &state.hand_tiles,
            &state.visible,
            riichi_options,
        ) {
            return (
                *player,
                PlayerAction::TurnAction(TurnActionMsg::RiichiDiscard(tile)),
            );
        }
        // 牌效分析异常时也不能退化为普通打牌；合法立直牌是安全兜底。
        if let Some(&tile) = riichi_options.first() {
            return (
                *player,
                PlayerAction::TurnAction(TurnActionMsg::RiichiDiscard(tile)),
            );
        }
    }
    let tile = if discard_options.len() == 1 {
        discard_options[0]
    } else {
        choose_discard(&mut state.calculator, &state.hand_tiles, &state.visible)
            .map(|option| option.tile)
            .and_then(|tile| {
                discard_options
                    .iter()
                    .copied()
                    .find(|candidate| candidate.tile_type() == tile.tile_type())
            })
            .or_else(|| discard_options.first().copied())
            .or_else(|| state.hand_tiles.last().copied())
            .unwrap_or_else(|| Tile::from_raw(0))
    };
    (
        *player,
        PlayerAction::TurnAction(TurnActionMsg::Discard(tile)),
    )
}

fn build_visible_tiles(
    melds: &[Vec<Meld>; 4],
    discards: &[Vec<Tile>; 4],
    dora: &[riichi_core::tile::TileType],
    player: PlayerId,
) -> VisibleTiles {
    let player_melds = vec![melds[player.0]
        .iter()
        .flat_map(|meld| meld.tiles.iter().copied())
        .collect::<Vec<_>>()];
    let other_melds = (0..4)
        .filter(|&index| index != player.0)
        .map(|index| {
            melds[index]
                .iter()
                .flat_map(|meld| meld.tiles.iter().copied())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let all_discards = discards.iter().flatten().copied().collect::<Vec<_>>();
    VisibleTiles::from_data(&player_melds, &other_melds, &all_discards, dora)
}
