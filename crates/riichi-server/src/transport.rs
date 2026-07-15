use crate::application::{JoinInfo, RoomInfo, ServerApplication};
use crate::protocol::{
    client_envelope_to_command, session_event_to_wire, state_snapshot_to_wire, CommandTracker,
};
use crate::room::RoomError;
use axum::extract::{
    ws::{Message, WebSocket, WebSocketUpgrade},
    Path, Query, State,
};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use riichi_proto::messages::{ClientEnvelope, ClientMessage, ServerMessage};
use riichi_session::SessionEvent;
use serde::Deserialize;
use tokio::time::{timeout, Duration};
use tower_http::cors::CorsLayer;

#[derive(Debug, Deserialize)]
pub struct JoinRequest {
    pub nickname: String,
}

#[derive(Debug, Deserialize)]
pub struct WebSocketQuery {
    pub room_id: String,
    pub token: String,
    #[serde(default)]
    pub last_event_id: u64,
}

#[derive(Debug, Deserialize)]
pub struct ReadyRequest {
    pub token: String,
    pub ready: bool,
}

#[derive(Debug, Deserialize)]
pub struct StartRequest {
    pub token: String,
}

pub fn router(application: ServerApplication) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/rooms", post(create_room))
        .route("/rooms/:room_id/join", post(join_room))
        .route("/rooms/:room_id/ready", post(set_ready))
        .route("/rooms/:room_id/start", post(start_room))
        .route("/ws", get(websocket))
        .layer(CorsLayer::permissive())
        .with_state(application)
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn create_room(State(application): State<ServerApplication>) -> Json<RoomInfo> {
    Json(application.create_room())
}

async fn join_room(
    State(application): State<ServerApplication>,
    Path(room_id): Path<String>,
    Json(request): Json<JoinRequest>,
) -> Result<Json<JoinInfo>, (StatusCode, String)> {
    application
        .join_room(&room_id, request.nickname)
        .map(Json)
        .map_err(room_error_response)
}

async fn set_ready(
    State(application): State<ServerApplication>,
    Path(room_id): Path<String>,
    Json(request): Json<ReadyRequest>,
) -> Result<Json<RoomInfo>, (StatusCode, String)> {
    application
        .set_ready(&room_id, &request.token, request.ready)
        .map(Json)
        .map_err(room_error_response)
}

async fn start_room(
    State(application): State<ServerApplication>,
    Path(room_id): Path<String>,
    Json(request): Json<StartRequest>,
) -> Result<Json<RoomInfo>, (StatusCode, String)> {
    application
        .authenticate(&room_id, &request.token)
        .map_err(room_error_response)?;
    application
        .launch_game(&room_id)
        .await
        .map(Json)
        .map_err(room_error_response)
}

fn room_error_response(error: RoomError) -> (StatusCode, String) {
    let status = match error {
        RoomError::NotFound => StatusCode::NOT_FOUND,
        _ => StatusCode::BAD_REQUEST,
    };
    (status, error.to_string())
}

async fn websocket(
    State(application): State<ServerApplication>,
    Query(query): Query<WebSocketQuery>,
    upgrade: WebSocketUpgrade,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let player = application
        .authenticate(&query.room_id, &query.token)
        .map_err(room_error_response)?;
    let (action_tx, event_rx) = application
        .session_channels(&query.room_id, player, query.last_event_id)
        .await
        .map_err(room_error_response)?;
    Ok(upgrade.on_upgrade(move |socket| {
        websocket_session(
            socket,
            query.room_id,
            query.token,
            player,
            application,
            action_tx,
            event_rx,
        )
    }))
}

enum Outbound {
    Text(String),
    Pong(Vec<u8>),
}

async fn websocket_session(
    socket: WebSocket,
    room_id: String,
    token: String,
    player: riichi_core::player::PlayerId,
    application: ServerApplication,
    action_tx: tokio::sync::mpsc::Sender<riichi_session::PlayerCommand>,
    event_rx: std::sync::Arc<tokio::sync::Mutex<tokio::sync::mpsc::Receiver<SessionEvent>>>,
) {
    if application.connect_player(&room_id, &token).is_err() {
        return;
    }
    let (mut socket_sender, mut socket_receiver) = socket.split();
    let (outbound_tx, mut outbound_rx) = tokio::sync::mpsc::channel(64);
    let writer = tokio::spawn(async move {
        while let Some(message) = outbound_rx.recv().await {
            let result = match message {
                Outbound::Text(text) => socket_sender.send(Message::Text(text)).await,
                Outbound::Pong(payload) => socket_sender.send(Message::Pong(payload)).await,
            };
            if result.is_err() {
                break;
            }
        }
    });
    let (inbound_tx, mut inbound_rx) = tokio::sync::mpsc::channel(64);
    let reader = tokio::spawn(async move {
        while let Some(message) = socket_receiver.next().await {
            if inbound_tx.send(message.map_err(|_| ())).await.is_err() {
                break;
            }
        }
    });

    let mut sequencer = crate::protocol::ServerSequencer::new();
    let mut command_tracker = CommandTracker::new();
    let mut sent_snapshot = false;
    let welcome = riichi_proto::messages::ServerMessage::RoomJoined {
        room_id: room_id.clone(),
        player_id: player,
    };
    if send_server_message(&outbound_tx, &mut sequencer, welcome)
        .await
        .is_err()
    {
        return;
    }

    loop {
        let received = timeout(Duration::from_secs(60), async {
            tokio::select! {
                message = inbound_rx.recv() => message.map(Ok),
                event = async {
                    let mut receiver = event_rx.lock().await;
                    receiver.recv().await
                } => event.map(Err),
            }
        })
        .await;
        let Ok(Some(result)) = received else { break };
        match result {
            Err(event) => {
                let message = if !sent_snapshot {
                    match event {
                        SessionEvent::GameEvent { .. } => session_event_to_wire(&event, player),
                        _ => state_snapshot_to_wire(&event, player),
                    }
                } else {
                    session_event_to_wire(&event, player)
                };
                let Some(message) = message else {
                    continue;
                };
                if matches!(message, ServerMessage::StateSnapshot(_)) {
                    sent_snapshot = true;
                }
                let game_over = matches!(&message, ServerMessage::GameOver { .. });
                if send_server_message(&outbound_tx, &mut sequencer, message)
                    .await
                    .is_err()
                {
                    break;
                }
                if game_over {
                    let _ = application.finish_game(&room_id).await;
                    break;
                }
            }
            Ok(Ok(Message::Ping(payload))) => {
                if outbound_tx.send(Outbound::Pong(payload)).await.is_err() {
                    break;
                }
            }
            Ok(Ok(Message::Text(text))) => {
                let envelope = serde_json::from_str::<ClientEnvelope>(&text);
                let Ok(envelope) = envelope else {
                    if send_server_message(
                        &outbound_tx,
                        &mut sequencer,
                        ServerMessage::Error("无法解析客户端协议消息".to_string()),
                    )
                    .await
                    .is_err()
                    {
                        break;
                    }
                    continue;
                };
                let command_id = envelope.command_id;
                let expected_seq = envelope.expected_seq;
                let requests_snapshot = matches!(&envelope.body, ClientMessage::RequestSnapshot);
                match client_envelope_to_command(
                    envelope,
                    player,
                    &mut command_tracker,
                    sequencer.current_seq(),
                ) {
                    Ok(Some(command)) => {
                        let ack_seq = sequencer.current_seq().saturating_add(1);
                        if action_tx.send(command).await.is_err()
                            || send_server_message(
                                &outbound_tx,
                                &mut sequencer,
                                ServerMessage::CommandAccepted {
                                    command_id,
                                    seq: ack_seq,
                                },
                            )
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(None) if requests_snapshot => {
                        // 快照请求不改变牌局，只从该玩家专属事件队列中取下一份
                        // 状态更新，避免把其他玩家的隐藏信息暴露给重连客户端。
                        let snapshot = loop {
                            let event = {
                                let mut receiver = event_rx.lock().await;
                                receiver.recv().await
                            };
                            let Some(event) = event else { break None };
                            if let Some(message) = state_snapshot_to_wire(&event, player) {
                                break Some(message);
                            }
                        };
                        if let Some(message) = snapshot {
                            if send_server_message(&outbound_tx, &mut sequencer, message)
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(error) => {
                        let actual_seq = sequencer.current_seq();
                        if send_server_message(
                            &outbound_tx,
                            &mut sequencer,
                            ServerMessage::CommandRejected {
                                command_id,
                                expected_seq,
                                actual_seq,
                                reason: format!("{error:?}"),
                            },
                        )
                        .await
                        .is_err()
                        {
                            break;
                        }
                    }
                }
            }
            Ok(Ok(Message::Close(_))) | Ok(Err(_)) => break,
            Ok(Ok(Message::Binary(_))) | Ok(Ok(Message::Pong(_))) => {}
        }
    }
    reader.abort();
    writer.abort();
    let _ = application.disconnect_player(&room_id, &token);
}

async fn send_server_message(
    outbound_tx: &tokio::sync::mpsc::Sender<Outbound>,
    sequencer: &mut crate::protocol::ServerSequencer,
    message: ServerMessage,
) -> Result<(), ()> {
    let text = serde_json::to_string(&sequencer.envelope(message)).map_err(|_| ())?;
    outbound_tx.send(Outbound::Text(text)).await.map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::router;
    use crate::application::ServerApplication;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_and_room_creation_routes_are_available() {
        let app = router(ServerApplication::new());

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rooms")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/rooms/000001/join")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"nickname":"玩家"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
