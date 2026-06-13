use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::session::{LlmSessionConfig, SessionPool};

#[derive(Deserialize)]
struct CreateSessionRequest {
    capability: String,
    kb: Option<String>,
    llm: Option<CreateLlmConfig>,
}

#[derive(Serialize)]
struct CreateSessionResponse {
    id: String,
}

#[derive(Deserialize)]
struct CreateLlmConfig {
    provider: String,
    model: String,
    api_key: Option<String>,
    base_url: Option<String>,
    chat_path: Option<String>,
    budget_limit_usd: Option<f64>,
    require_approval: Option<bool>,
}

async fn create_session(
    State(pool): State<Arc<SessionPool>>,
    Json(req): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    let llm = req.llm.map(|config| LlmSessionConfig {
        provider: config.provider,
        model: config.model,
        api_key: config.api_key.filter(|value| !value.trim().is_empty()),
        base_url: config.base_url.filter(|value| !value.trim().is_empty()),
        chat_path: config.chat_path.filter(|value| !value.trim().is_empty()),
        budget_limit_usd: config.budget_limit_usd,
        require_approval: config.require_approval.unwrap_or(false),
    });
    let id = pool.create(&req.capability, req.kb, llm);
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
                "llm": entry.llm.map(|config| serde_json::json!({
                    "provider": config.provider,
                    "model": config.model,
                    "api_key_configured": config.api_key.is_some(),
                    "base_url": config.base_url,
                    "chat_path": config.chat_path,
                    "budget_limit_usd": config.budget_limit_usd,
                    "require_approval": config.require_approval,
                })),
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
        .route("/api/sessions/{id}", get(get_session))
        .with_state(pool)
}
