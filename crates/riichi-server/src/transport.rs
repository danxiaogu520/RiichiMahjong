use crate::application::{JoinInfo, RoomInfo, ServerApplication};
use crate::room::RoomError;
use axum::extract::{
    ws::{Message, WebSocket, WebSocketUpgrade},
    Path, Query, State,
};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use tokio::time::{timeout, Duration};

#[derive(Debug, Deserialize)]
pub struct JoinRequest {
    pub nickname: String,
}

#[derive(Debug, Deserialize)]
pub struct WebSocketQuery {
    pub room_id: String,
    pub token: String,
}

pub fn router(application: ServerApplication) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/rooms", post(create_room))
        .route("/rooms/:room_id/join", post(join_room))
        .route("/ws", get(websocket))
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
    Ok(upgrade.on_upgrade(move |socket| websocket_session(socket, query.room_id, player)))
}

async fn websocket_session(
    mut socket: WebSocket,
    room_id: String,
    player: riichi_core::player::PlayerId,
) {
    let mut sequencer = crate::protocol::ServerSequencer::new();
    let welcome = riichi_proto::messages::ServerMessage::RoomJoined {
        room_id,
        player_id: player,
    };
    if let Ok(text) = serde_json::to_string(&sequencer.envelope(welcome)) {
        if socket.send(Message::Text(text)).await.is_err() {
            return;
        }
    }

    while let Ok(Some(result)) = timeout(Duration::from_secs(60), socket.recv()).await {
        match result {
            Ok(Message::Ping(payload)) => {
                if socket.send(Message::Pong(payload)).await.is_err() {
                    break;
                }
            }
            Ok(Message::Text(text)) => {
                if serde_json::from_str::<riichi_proto::messages::ClientEnvelope>(&text).is_err() {
                    let error = riichi_proto::messages::ServerMessage::Error(
                        "无法解析客户端协议消息".to_string(),
                    );
                    if let Ok(text) = serde_json::to_string(&sequencer.envelope(error)) {
                        if socket.send(Message::Text(text)).await.is_err() {
                            break;
                        }
                    }
                }
            }
            Ok(Message::Close(_)) | Err(_) => break,
            Ok(Message::Binary(_)) | Ok(Message::Pong(_)) => {}
        }
    }
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
