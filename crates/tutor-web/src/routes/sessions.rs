use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::session::SessionPool;

#[derive(Deserialize)]
struct CreateSessionRequest {
    capability: String,
    kb: Option<String>,
}

#[derive(Serialize)]
struct CreateSessionResponse {
    id: String,
}

async fn create_session(
    State(pool): State<Arc<SessionPool>>,
    Json(req): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    let id = pool.create(&req.capability, req.kb);
    (StatusCode::CREATED, Json(CreateSessionResponse { id }))
}

async fn get_session(
    State(pool): State<Arc<SessionPool>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match pool.get(&id) {
        Some(entry) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "id": entry.id,
                "capability": entry.capability,
                "kb": entry.kb,
            })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "session not found" })),
        ),
    }
}

pub fn sessions_router(pool: Arc<SessionPool>) -> Router {
    Router::new()
        .route("/api/sessions", post(create_session))
        .route("/api/sessions/:id", get(get_session))
        .with_state(pool)
}
