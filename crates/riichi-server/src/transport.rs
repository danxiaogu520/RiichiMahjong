use crate::application::{JoinInfo, RoomInfo, ServerApplication};
use crate::room::RoomError;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct JoinRequest {
    pub nickname: String,
}

pub fn router(application: ServerApplication) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/rooms", post(create_room))
        .route("/rooms/:room_id/join", post(join_room))
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
