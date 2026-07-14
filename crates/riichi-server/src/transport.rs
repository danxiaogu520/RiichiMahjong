use crate::application::{RoomInfo, ServerApplication};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};

pub fn router(application: ServerApplication) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/rooms", post(create_room))
        .with_state(application)
}

async fn health() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn create_room(State(application): State<ServerApplication>) -> Json<RoomInfo> {
    Json(application.create_room())
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
    }
}
