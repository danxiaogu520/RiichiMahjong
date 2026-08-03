use crate::{choose_discard, decide_call, decide_riichi};
use riichi_core::game::ResponseAction;
use riichi_core::meld::Meld;
use riichi_core::player::PlayerId;
use riichi_core::tile::Tile;
use riichi_engine::game::GamePhase;
use riichi_logic::shanten::ShantenCalculator;
use riichi_logic::visibility::VisibleTiles;
use riichi_session::{AgentFuture, PlayerAction, PlayerAgent, SessionEvent, TurnAction};
use std::time::Duration;
use tokio::time::sleep;

struct AiState {
    hand_tiles: Vec<Tile>,
    phase: GamePhase,
    current_player: PlayerId,
    can_tsumo: bool,
    can_riichi: bool,
    visible: VisibleTiles,
    calculator: ShantenCalculator,
}

pub struct BasicAiAgent {
    player: PlayerId,
    state: AiState,
}

impl BasicAiAgent {
    pub fn new(player: PlayerId) -> Self {
        Self {
            player,
            state: AiState {
                hand_tiles: Vec::new(),
                phase: GamePhase::DrawPhase {
                    player,
                    position: riichi_core::game::DrawPosition::LiveWall,
                },
                current_player: player,
                can_tsumo: false,
                can_riichi: false,
                visible: VisibleTiles::new(),
                calculator: ShantenCalculator::new(),
            },
        }
    }

    fn update_state(
        &mut self,
        phase: GamePhase,
        hand_tiles: Vec<Tile>,
        discards: [Vec<Tile>; 4],
        melds: [Vec<Meld>; 4],
        dora: Vec<riichi_core::tile::TileType>,
    ) {
        self.state.phase = phase;
        self.state.current_player = match self.state.phase {
            GamePhase::DrawPhase { player, .. }
            | GamePhase::ActionPhase { player, .. }
            | GamePhase::ResponsePhase { player, .. }
            | GamePhase::ChankanResponse { player, .. } => player,
            GamePhase::RoundOver => self.player,
        };
        self.state.hand_tiles = hand_tiles;
        self.state.visible = build_visible_tiles(&melds, &discards, &dora, self.player);
    }
}

impl PlayerAgent for BasicAiAgent {
    fn player_id(&self) -> PlayerId {
        self.player
    }

    fn decide<'a>(&'a mut self, observation: SessionEvent) -> AgentFuture<'a> {
        Box::pin(async move {
            match observation {
                SessionEvent::StateUpdate {
                    phase,
                    hand_tiles,
                    discards,
                    melds,
                    dora,
                    ..
                } => {
                    self.update_state(phase, hand_tiles, discards, melds, dora);
                    None
                }
                SessionEvent::ActionRequired {
                    can_tsumo,
                    can_riichi,
                    riichi_options,
                    discard_options,
                    ..
                } => {
                    self.state.can_tsumo = can_tsumo;
                    self.state.can_riichi = can_riichi;
                    wait_before_decision().await;
                    Some(decide_turn(&self.state, &riichi_options, &discard_options))
                }
                SessionEvent::CallRequired { options } => {
                    wait_before_decision().await;
                    let response = decide_call(&options).unwrap_or(ResponseAction::Pass);
                    let response = match response {
                        ResponseAction::Ron => riichi_session::CallResponse::Ron,
                        _ => riichi_session::CallResponse::Pass,
                    };
                    Some(PlayerAction::CallResponse(response))
                }
                SessionEvent::GameOver { .. } => None,
                SessionEvent::RoundResult { .. }
                | SessionEvent::Error(_)
                | SessionEvent::GameEvent { .. }
                | SessionEvent::PlayerControllerChanged { .. } => None,
            }
        })
    }
}

async fn wait_before_decision() {
    sleep(Duration::from_millis(5)).await;
}

fn decide_turn(state: &AiState, riichi_options: &[Tile], discard_options: &[Tile]) -> PlayerAction {
    if state.can_tsumo {
        return PlayerAction::TurnAction(TurnAction::Tsumo);
    }
    if state.can_riichi {
        if let Some(tile) = decide_riichi(
            &state.calculator,
            &state.hand_tiles,
            &state.visible,
            riichi_options,
        ) {
            return PlayerAction::TurnAction(TurnAction::RiichiDiscard(tile));
        }
        if let Some(&tile) = riichi_options.first() {
            return PlayerAction::TurnAction(TurnAction::RiichiDiscard(tile));
        }
    }
    let tile = if discard_options.len() == 1 {
        discard_options[0]
    } else {
        choose_discard(&state.calculator, &state.hand_tiles, &state.visible)
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
    PlayerAction::TurnAction(TurnAction::Discard(tile))
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

#[cfg(test)]
mod tests {
    use super::BasicAiAgent;
    use riichi_core::game::{CallOption, CallType};
    use riichi_core::player::PlayerId;
    use riichi_core::tile::Tile;
    use riichi_session::{CallResponse, PlayerAction, PlayerAgent, SessionEvent, TurnAction};

    #[tokio::test]
    async fn basic_ai_returns_a_legal_discard_from_action_options() {
        let mut ai = BasicAiAgent::new(PlayerId(0));
        let action = ai
            .decide(SessionEvent::ActionRequired {
                can_tsumo: false,
                can_riichi: false,
                riichi_options: Vec::new(),
                discard_options: vec![Tile::from_raw(0)],
                ankan_options: Vec::new(),
                kakan_options: Vec::new(),
                can_kyuushu: false,
            })
            .await;
        assert!(matches!(
            action,
            Some(PlayerAction::TurnAction(TurnAction::Discard(tile)))
                if tile == Tile::from_raw(0)
        ));
    }

    #[tokio::test]
    async fn basic_ai_ron_or_passes_call_options() {
        let mut ai = BasicAiAgent::new(PlayerId(0));
        let action = ai
            .decide(SessionEvent::CallRequired {
                options: vec![CallOption {
                    player: PlayerId(0),
                    call_type: CallType::Ron,
                }],
            })
            .await;
        assert!(matches!(
            action,
            Some(PlayerAction::CallResponse(CallResponse::Ron))
        ));

        let action = ai
            .decide(SessionEvent::CallRequired {
                options: vec![CallOption {
                    player: PlayerId(0),
                    call_type: CallType::Pon {
                        hand_tiles: [Tile::from_raw(0), Tile::from_raw(1)],
                    },
                }],
            })
            .await;
        assert!(matches!(
            action,
            Some(PlayerAction::CallResponse(CallResponse::Pass))
        ));
    }
}
