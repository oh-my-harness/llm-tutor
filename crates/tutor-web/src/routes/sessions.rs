use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::{Deserialize, Serialize};

use crate::knowledge_store::KnowledgeStore;
use crate::session::{LlmSessionConfig, SessionPool, message_role, message_text};

#[derive(Clone)]
pub struct SessionsState {
    pool: Arc<SessionPool>,
    knowledge: Arc<KnowledgeStore>,
}

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
    name: Option<String>,
    kb: Option<String>,
    llm: Option<CreateLlmConfig>,
}

async fn create_session(
    State(state): State<Arc<SessionsState>>,
    Json(req): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    let pool = &state.pool;
    let llm = req.llm.map(|config| LlmSessionConfig {
        provider: config.provider,
        model: config.model,
        api_key: config.api_key.filter(|value| !value.trim().is_empty()),
        base_url: config.base_url.filter(|value| !value.trim().is_empty()),
        chat_path: config.chat_path.filter(|value| !value.trim().is_empty()),
        budget_limit_usd: config.budget_limit_usd,
        require_approval: config.require_approval.unwrap_or(false),
    });
    let (kb, embedding) = match knowledge_binding(&state.knowledge, req.kb) {
        Ok(binding) => binding,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response();
        }
    };
    match pool.create(&req.capability, kb, llm, embedding).await {
        Ok(id) => (StatusCode::CREATED, Json(CreateSessionResponse { id })).into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

async fn list_sessions(State(state): State<Arc<SessionsState>>) -> impl IntoResponse {
    let pool = &state.pool;
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
    State(state): State<Arc<SessionsState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let pool = &state.pool;
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
            "embedding": entry.embedding.map(|config| serde_json::json!({
                "provider": config.provider,
                "model": config.model,
                "api_key_configured": !config.api_key.trim().is_empty(),
                "base_url": config.base_url,
                "embeddings_path": config.embeddings_path,
                "dimensions": config.dimensions,
                "send_dimensions": config.send_dimensions,
            })),
        })),
    )
        .into_response()
}

async fn update_session(
    State(state): State<Arc<SessionsState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateSessionRequest>,
) -> impl IntoResponse {
    let pool = &state.pool;
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

    if let Some(kb) = req.kb {
        let normalized_kb = kb.trim().to_string();
        let (kb, embedding) = if normalized_kb.is_empty() {
            (None, None)
        } else {
            let Some(item) = state.knowledge.get(&normalized_kb) else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": "knowledge base not found" })),
                )
                    .into_response();
            };
            (Some(item.id), Some(item.embedding))
        };

        let _ = pool.ensure_entry(&id).await;
        if !pool.set_knowledge(&id, kb, embedding) {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "session not found" })),
            )
                .into_response();
        }
    }

    if let Some(llm) = req.llm {
        let _ = pool.ensure_entry(&id).await;
        let llm = Some(llm_config_from_request(llm));
        if !pool.set_llm(&id, llm) {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "session not found" })),
            )
                .into_response();
        }
    }

    if let Some(name) = req.name {
        let normalized = name.trim().to_string();
        let next_name = if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        };
        if let Err(err) = pool.rename(&id, next_name).await {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": err.to_string() })),
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

fn llm_config_from_request(config: CreateLlmConfig) -> LlmSessionConfig {
    LlmSessionConfig {
        provider: config.provider,
        model: config.model,
        api_key: config.api_key.filter(|value| !value.trim().is_empty()),
        base_url: config.base_url.filter(|value| !value.trim().is_empty()),
        chat_path: config.chat_path.filter(|value| !value.trim().is_empty()),
        budget_limit_usd: config.budget_limit_usd,
        require_approval: config.require_approval.unwrap_or(false),
    }
}

async fn delete_session(
    State(state): State<Arc<SessionsState>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let pool = &state.pool;
    match pool.delete(&id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

pub fn sessions_router(pool: Arc<SessionPool>, knowledge: Arc<KnowledgeStore>) -> Router {
    let state = Arc::new(SessionsState { pool, knowledge });
    Router::new()
        .route("/api/sessions", get(list_sessions).post(create_session))
        .route(
            "/api/sessions/{id}",
            get(get_session)
                .patch(update_session)
                .delete(delete_session),
        )
        .with_state(state)
}

fn knowledge_binding(
    knowledge: &KnowledgeStore,
    kb: Option<String>,
) -> Result<(Option<String>, Option<tutor_rag::EmbeddingConfig>), anyhow::Error> {
    let Some(kb) = kb
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    else {
        return Ok((None, None));
    };

    let Some(item) = knowledge.get(&kb) else {
        return Err(anyhow::anyhow!("knowledge base not found"));
    };

    Ok((Some(item.id), Some(item.embedding)))
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
