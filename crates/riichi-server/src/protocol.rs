//! Internal channel messages and wire protocol messages are deliberately kept
//! separate. This module is the single conversion boundary between them.

use riichi_core::game::{CallOption, CallType};
use riichi_core::meld::MeldKind;
use riichi_core::player::PlayerId;
use riichi_engine::game::GamePhase;
use riichi_proto::messages::{
    ActionRequest, CallResponsePayload, CallTypeView, ClientMessage, GamePhaseView, GameStateView,
    MeldKindView, MeldView, PlayerView, ServerMessage, TenpaiInfoView, TurnActionPayload,
    WaitInfoView,
};

use crate::channel::{CallResponseMsg, PlayerAction, ServerEvent, TurnActionMsg};

/// Converts an authenticated wire message into the internal action format.
/// The player identity is supplied by the session, never by the wire message.
pub fn client_message_to_action(message: ClientMessage) -> Option<PlayerAction> {
    match message {
        ClientMessage::TurnAction { action } => Some(PlayerAction::TurnAction(match action {
            TurnActionPayload::Discard(tile) => TurnActionMsg::Discard(tile),
            TurnActionPayload::RiichiDiscard(tile) => TurnActionMsg::RiichiDiscard(tile),
            TurnActionPayload::Tsumo => TurnActionMsg::Tsumo,
            TurnActionPayload::Ankan(tile) => TurnActionMsg::Ankan(tile),
            TurnActionPayload::Kakan(index, tile) => TurnActionMsg::Kakan(index, tile),
            TurnActionPayload::KyuushuKyuuhai => TurnActionMsg::KyuushuKyuuhai,
        })),
        ClientMessage::CallResponse { action } => Some(PlayerAction::CallResponse(match action {
            CallResponsePayload::Pass => CallResponseMsg::Pass,
            CallResponsePayload::Ron => CallResponseMsg::Ron,
            CallResponsePayload::Pon { hand_tiles } => CallResponseMsg::Pon { hand_tiles },
            CallResponsePayload::Chi { hand_tiles } => CallResponseMsg::Chi { hand_tiles },
            CallResponsePayload::Minkan { hand_tiles } => CallResponseMsg::Minkan { hand_tiles },
        })),
        ClientMessage::JoinRoom { .. } | ClientMessage::Ready | ClientMessage::LeaveRoom => None,
    }
}

/// Converts one player's internal state event into a player-scoped wire view.
/// Opponent hands are always omitted, even though the internal event contains
/// the hand snapshot for the recipient only.
pub fn state_update_to_wire(event: &ServerEvent, recipient: PlayerId) -> Option<ServerMessage> {
    let ServerEvent::StateUpdate {
        phase,
        current_player,
        drawn_tile,
        hand_tiles,
        hand_count,
        hand_counts,
        points,
        winds,
        is_riichi,
        discards,
        melds,
        dora,
        remaining_tiles,
        round,
        honba,
        riichi_sticks,
        tenpai_info,
        ..
    } = event
    else {
        return None;
    };

    let players = std::array::from_fn(|index| PlayerView {
        id: PlayerId(index),
        hand: (index == recipient.0).then(|| hand_tiles.clone()),
        hand_count: if index == recipient.0 {
            *hand_count
        } else {
            hand_counts[index]
        },
        points: points[index],
        wind: winds[index],
        discards: discards[index].clone(),
        melds: melds[index]
            .iter()
            .map(|meld| MeldView {
                kind: meld_kind(meld.kind),
                tiles: meld.tiles.clone(),
                from_player: meld.from_player,
            })
            .collect(),
        is_riichi: is_riichi[index],
        riichi_declaration_tile: None,
    });

    Some(ServerMessage::StateUpdate(Box::new(GameStateView {
        players,
        wind: winds[recipient.0],
        round: *round,
        honba: *honba,
        riichi_sticks: *riichi_sticks,
        current_player: *current_player,
        drawn_tile: (*current_player == recipient)
            .then_some(*drawn_tile)
            .flatten(),
        dora: dora.clone(),
        remaining_tiles: *remaining_tiles,
        phase: phase_view(phase),
        recent_events: Vec::new(),
        analysis: None,
        tenpai_info: tenpai_info.as_ref().map(|info| TenpaiInfoView {
            is_furiten: info.is_furiten,
            waits: info
                .waits
                .iter()
                .map(|wait| WaitInfoView {
                    tile_type: wait.tile_type,
                    remaining: wait.remaining,
                    is_no_yaku: wait.is_no_yaku,
                })
                .collect(),
        }),
    })))
}

pub fn action_required_to_wire(request: ActionRequest) -> ServerMessage {
    ServerMessage::ActionRequired(request)
}

pub fn call_options_to_wire(player: PlayerId, options: &[CallOption]) -> ServerMessage {
    ServerMessage::CallRequired(riichi_proto::messages::CallRequest {
        player,
        options: options
            .iter()
            .filter(|option| option.player == player)
            .map(|option| riichi_proto::messages::CallOptionView {
                player: option.player,
                call_type: match &option.call_type {
                    CallType::Ron => CallTypeView::Ron,
                    CallType::Minkan { hand_tiles } => CallTypeView::Minkan {
                        hand_tiles: *hand_tiles,
                    },
                    CallType::Pon { hand_tiles } => CallTypeView::Pon {
                        hand_tiles: *hand_tiles,
                    },
                    CallType::Chi { hand_tiles } => CallTypeView::Chi {
                        hand_tiles: *hand_tiles,
                    },
                },
            })
            .collect(),
    })
}

fn phase_view(phase: &GamePhase) -> GamePhaseView {
    match phase {
        GamePhase::DrawPhase => GamePhaseView::DrawPhase,
        GamePhase::ActionPhase => GamePhaseView::ActionPhase,
        GamePhase::ResponsePhase { .. } => GamePhaseView::ResponsePhase,
        GamePhase::ChankanResponse { .. } => GamePhaseView::ChankanResponse,
        GamePhase::RoundOver => GamePhaseView::RoundOver,
    }
}

fn meld_kind(kind: MeldKind) -> MeldKindView {
    match kind {
        MeldKind::Chi => MeldKindView::Chi,
        MeldKind::Pon => MeldKindView::Pon,
        MeldKind::Ankan => MeldKindView::Ankan,
        MeldKind::Minkan => MeldKindView::Minkan,
        MeldKind::Kakan => MeldKindView::Kakan,
    }
}

#[cfg(test)]
mod tests {
    use super::{call_options_to_wire, client_message_to_action, state_update_to_wire};
    use crate::channel::{PlayerAction, ServerEvent, TurnActionMsg};
    use riichi_core::game::{CallOption, CallType};
    use riichi_core::player::PlayerId;
    use riichi_core::tile::Tile;
    use riichi_engine::game::GamePhase;
    use riichi_proto::messages::{ClientMessage, ServerMessage, TurnActionPayload};

    #[test]
    fn wire_action_does_not_supply_player_identity() {
        let action = client_message_to_action(ClientMessage::TurnAction {
            action: TurnActionPayload::Discard(Tile::from_raw(7)),
        });
        assert!(matches!(
            action,
            Some(PlayerAction::TurnAction(TurnActionMsg::Discard(tile))) if tile == Tile::from_raw(7)
        ));
    }

    #[test]
    fn state_view_only_exposes_recipient_hand_and_drawn_tile() {
        let event = ServerEvent::StateUpdate {
            phase: GamePhase::ActionPhase,
            current_player: PlayerId(0),
            pending_discard: None,
            drawn_tile: Some(Tile::from_raw(12)),
            hand_tiles: vec![Tile::from_raw(1)],
            hand_count: 1,
            hand_counts: [1; 4],
            points: [25000; 4],
            winds: [riichi_core::tile::TileType::EAST; 4],
            is_riichi: [false; 4],
            discards: std::array::from_fn(|_| Vec::new()),
            melds_count: [0; 4],
            melds: std::array::from_fn(|_| Vec::new()),
            dora: Vec::new(),
            remaining_tiles: 100,
            round: 1,
            honba: 0,
            riichi_sticks: 0,
            tenpai_info: None,
            analysis_options: Vec::new(),
        };

        let ServerMessage::StateUpdate(view) = state_update_to_wire(&event, PlayerId(0)).unwrap()
        else {
            panic!("expected state update")
        };
        assert_eq!(
            view.players[0].hand.as_deref(),
            Some(&[Tile::from_raw(1)][..])
        );
        assert!(view.players[1].hand.is_none());
        assert_eq!(view.drawn_tile, Some(Tile::from_raw(12)));
    }

    #[test]
    fn call_view_only_exposes_recipient_options() {
        let options = vec![
            CallOption {
                player: PlayerId(0),
                call_type: CallType::Ron,
            },
            CallOption {
                player: PlayerId(1),
                call_type: CallType::Ron,
            },
        ];

        let ServerMessage::CallRequired(request) = call_options_to_wire(PlayerId(1), &options)
        else {
            panic!("expected call request")
        };
        assert_eq!(request.player, PlayerId(1));
        assert_eq!(request.options.len(), 1);
        assert_eq!(request.options[0].player, PlayerId(1));
    }
}
