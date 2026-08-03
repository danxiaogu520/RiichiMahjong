use std::time::Duration;

use riichi_core::player::PlayerId;
use riichi_server::application::{ServerApplication, SessionEventReceiver};
use riichi_session::{CallResponse, PlayerAction, PlayerCommand, SessionEvent, TurnAction};
use tokio::sync::mpsc;

#[tokio::test]
async fn one_human_three_ai_room_reaches_game_over() {
    let app = ServerApplication::new_with_delays(Duration::from_millis(10), Duration::ZERO);
    let room = app.create_room();
    let owner = app.join_room(&room.id, "房主").unwrap();
    app.set_ai_count(&room.id, &owner.token, 3).unwrap();
    app.set_ready(&room.id, &owner.token, true).unwrap();
    app.launch_game(&room.id, &owner.token).await.unwrap();

    let (action_tx, event_rx) = app
        .session_channels(&room.id, owner.player, 0)
        .await
        .unwrap();
    let human = tokio::spawn(drive_human_player(owner.player, action_tx, event_rx));
    tokio::time::timeout(Duration::from_secs(45), human)
        .await
        .expect("AI room should finish within forty-five seconds")
        .unwrap()
        .unwrap();
    app.finish_game(&room.id).await.unwrap();
}

#[tokio::test]
async fn reconnect_before_delay_prevents_ai_takeover() {
    let app = ServerApplication::new_with_ai_takeover_delay(Duration::from_millis(20));
    let room = app.create_room();
    let owner = app.join_room(&room.id, "房主").unwrap();
    app.set_ai_count(&room.id, &owner.token, 3).unwrap();
    app.set_ready(&room.id, &owner.token, true).unwrap();
    app.launch_game(&room.id, &owner.token).await.unwrap();

    app.disconnect_player(&room.id, &owner.token).await.unwrap();
    tokio::time::sleep(Duration::from_millis(5)).await;
    app.connect_player(&room.id, &owner.token).unwrap();
    tokio::time::sleep(Duration::from_millis(30)).await;

    let room_after = app.room_info(&room.id).unwrap();
    let owner_view = room_after
        .players
        .iter()
        .find(|player| player.id == owner.player)
        .unwrap();
    assert!(!owner_view.ai_takeover);
    assert!(!owner_view.is_ai);
    app.finish_game(&room.id).await.unwrap();
}

#[tokio::test]
async fn reconnect_after_takeover_restores_human_controller() {
    let app = ServerApplication::new_with_ai_takeover_delay(Duration::from_millis(10));
    let room = app.create_room();
    let owner = app.join_room(&room.id, "房主").unwrap();
    app.set_ai_count(&room.id, &owner.token, 3).unwrap();
    app.set_ready(&room.id, &owner.token, true).unwrap();
    app.launch_game(&room.id, &owner.token).await.unwrap();

    app.disconnect_player(&room.id, &owner.token).await.unwrap();
    tokio::time::sleep(Duration::from_millis(25)).await;
    let room_after_takeover = app.room_info(&room.id).unwrap();
    assert!(room_after_takeover
        .players
        .iter()
        .any(|player| player.id == owner.player && player.ai_takeover));

    app.connect_player(&room.id, &owner.token).unwrap();
    let (_, event_rx) = app
        .session_channels(&room.id, owner.player, 0)
        .await
        .unwrap();

    let restored = tokio::time::timeout(Duration::from_secs(1), async move {
        loop {
            let event = {
                let mut receiver = event_rx.lock().await;
                receiver.recv().await
            };
            match event {
                Some(SessionEvent::PlayerControllerChanged {
                    player,
                    is_ai,
                    ai_takeover,
                }) if player == owner.player && !is_ai && !ai_takeover => break true,
                Some(SessionEvent::GameOver { .. }) | None => break false,
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    assert!(restored);
    app.finish_game(&room.id).await.unwrap();
}

async fn drive_human_player(
    player: PlayerId,
    action_tx: mpsc::Sender<PlayerCommand>,
    event_rx: SessionEventReceiver,
) -> Result<(), String> {
    while let Some(event) = {
        let mut receiver = event_rx.lock().await;
        receiver.recv().await
    } {
        match event {
            SessionEvent::ActionRequired {
                discard_options,
                can_tsumo,
                ..
            } => {
                let action = if can_tsumo {
                    TurnAction::Tsumo
                } else {
                    TurnAction::Discard(
                        discard_options
                            .first()
                            .copied()
                            .ok_or_else(|| "no discard option".to_string())?,
                    )
                };
                action_tx
                    .send(PlayerCommand::new(player, PlayerAction::TurnAction(action)))
                    .await
                    .map_err(|error| error.to_string())?;
            }
            SessionEvent::CallRequired { .. } => {
                action_tx
                    .send(PlayerCommand::new(
                        player,
                        PlayerAction::CallResponse(CallResponse::Pass),
                    ))
                    .await
                    .map_err(|error| error.to_string())?;
            }
            SessionEvent::GameOver { .. } => return Ok(()),
            _ => {}
        }
    }
    Err("session ended before GameOver".to_string())
}
