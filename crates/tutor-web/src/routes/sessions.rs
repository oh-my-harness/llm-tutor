use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::{Deserialize, Serialize};

use crate::session::{LlmSessionConfig, SessionPool, message_role, message_text};

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

#[derive(Deserialize)]
struct UpdateSessionRequest {
    capability: Option<String>,
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
    match pool.create(&req.capability, req.kb, llm).await {
        Ok(id) => (StatusCode::CREATED, Json(CreateSessionResponse { id })).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

async fn list_sessions(State(pool): State<Arc<SessionPool>>) -> impl IntoResponse {
    match pool.list(Some(50)).await {
        Ok(sessions) => {
            let mut items = Vec::with_capacity(sessions.len());
            for session in sessions {
                let title = match session.name.clone() {
                    Some(name) if !name.trim().is_empty() => name,
                    _ => pool
                        .messages(&session.id)
                        .await
                        .ok()
                        .and_then(|messages| title_from_messages(&messages))
                        .unwrap_or_else(|| "New session".into()),
                };
                items.push(serde_json::json!({
                    "id": session.id,
                    "title": title,
                    "name": session.name,
                    "created_at": session.created_at,
                    "updated_at": session.updated_at,
                    "model": session.model,
                }));
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "sessions": items,
                })),
            )
                .into_response()
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

async fn get_session(
    State(pool): State<Arc<SessionPool>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let Some(entry) = pool.ensure_entry(&id).await else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "session not found" })),
        )
            .into_response();
    };

    let meta = match pool.metadata(&id).await {
        Ok(meta) => meta,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response();
        }
    };
    let messages = match pool.messages(&id).await {
        Ok(messages) => messages,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response();
        }
    };
    let history_len = messages.len();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "id": entry.id,
            "capability": entry.capability,
            "kb": entry.kb,
            "history_len": history_len,
            "metadata": {
                "name": meta.name,
                "created_at": meta.created_at,
                "updated_at": meta.updated_at,
                "model": meta.model,
            },
            "messages": messages.into_iter().filter_map(|message| {
                let role = message_role(&message)?;
                Some(serde_json::json!({
                    "role": role,
                    "text": message_text(&message),
                }))
            }).collect::<Vec<_>>(),
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
    )
        .into_response()
}

async fn update_session(
    State(pool): State<Arc<SessionPool>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateSessionRequest>,
) -> impl IntoResponse {
    if let Some(capability) = req.capability {
        if capability
            .parse::<tutor_agent::capability::Capability>()
            .is_err()
        {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "unsupported capability" })),
            )
                .into_response();
        }

        if !pool.set_capability(&id, &capability) {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "session not found" })),
            )
                .into_response();
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "id": id, "updated": true })),
    )
        .into_response()
}

pub fn sessions_router(pool: Arc<SessionPool>) -> Router {
    Router::new()
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route("/api/sessions/{id}", get(get_session).patch(update_session))
        .with_state(pool)
}

fn title_from_messages(messages: &[llm_harness_types::AgentMessage]) -> Option<String> {
    messages.iter().find_map(|message| {
        if message_role(message) != Some("user") {
            return None;
        }
        let title = session_title_from_message(&message_text(message));
        (!title.is_empty()).then_some(title)
    })
}

fn session_title_from_message(text: &str) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= 18 {
        normalized
    } else {
        format!("{}...", normalized.chars().take(18).collect::<String>())
    }
}
