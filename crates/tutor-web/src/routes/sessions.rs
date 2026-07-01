use std::{collections::HashMap, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::knowledge_store::KnowledgeStore;
use crate::session::{
    LlmSessionConfig, SearchSessionConfig, SessionPool, message_role, message_text,
};

#[derive(Clone)]
pub struct SessionsState {
    pool: Arc<SessionPool>,
    knowledge: Arc<KnowledgeStore>,
}

#[derive(Deserialize)]
struct CreateSessionRequest {
    capability: String,
    kb: Option<String>,
    notebook_enabled: Option<bool>,
    llm: Option<CreateLlmConfig>,
    search: Option<CreateSearchConfig>,
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
    context_window_tokens: Option<u32>,
    budget_limit_usd: Option<f64>,
    require_approval: Option<bool>,
}

#[derive(Deserialize)]
struct CreateSearchConfig {
    provider: String,
    base_url: String,
    api_key: Option<String>,
    max_results: Option<usize>,
    fetch_timeout_secs: Option<u64>,
    max_fetch_chars: Option<usize>,
}

#[derive(Deserialize)]
struct UpdateSessionRequest {
    capability: Option<String>,
    name: Option<String>,
    kb: Option<String>,
    notebook_enabled: Option<bool>,
    llm: Option<CreateLlmConfig>,
    search: Option<CreateSearchConfig>,
}

#[derive(Deserialize)]
struct AppendMessageRequest {
    user: Option<String>,
    assistant: Option<String>,
    quiz_id: Option<String>,
    assistant_citations: Option<Vec<serde_json::Value>>,
}

#[derive(Deserialize)]
struct AppendMessageCitationsRequest {
    citations: Vec<serde_json::Value>,
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
        context_window_tokens: config.context_window_tokens,
        budget_limit_usd: config.budget_limit_usd,
        require_approval: config.require_approval.unwrap_or(false),
    });
    let search = req.search.and_then(search_config_from_request);
    let notebook_enabled = req.notebook_enabled.unwrap_or(false);
    let (kb, embedding) = match knowledge_binding(&state.knowledge, req.kb, notebook_enabled) {
        Ok(binding) => binding,
        Err(err) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response();
        }
    };
    match pool
        .create(
            &req.capability,
            kb,
            notebook_enabled,
            llm,
            search,
            embedding,
        )
        .await
    {
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
    let traces = match pool.traces(&id).await {
        Ok(traces) => traces,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response();
        }
    };
    let compact_summary = match pool.compact_summary(&id).await {
        Ok(summary) => summary,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response();
        }
    };
    let latest_usage = match pool.latest_usage(&id).await {
        Ok(usage) => usage,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response();
        }
    };
    let message_mentions = match pool.message_mentions(&id).await {
        Ok(items) => items,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response();
        }
    };
    let mentions_by_user_index = message_mentions
        .into_iter()
        .map(|item| (item.user_message_index, item.mentions))
        .collect::<HashMap<_, _>>();
    let message_citations = match pool.message_citations(&id).await {
        Ok(items) => items,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response();
        }
    };
    let citations_by_assistant_index = message_citations
        .into_iter()
        .map(|item| (item.assistant_message_index, item.citations))
        .collect::<HashMap<_, _>>();
    let mut user_message_index = 0usize;
    let mut assistant_message_index = 0usize;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "id": entry.id,
            "capability": entry.capability,
            "kb": entry.kb,
            "notebook_enabled": entry.notebook_enabled,
            "history_len": history_len,
            "metadata": {
                "name": meta.name,
                "created_at": meta.created_at,
                "updated_at": meta.updated_at,
                "model": meta.model,
            },
            "messages": messages.into_iter().filter_map(|message| {
                let role = message_role(&message)?;
                let mut value = serde_json::json!({
                    "role": role,
                    "text": message_text(&message),
                });
                if role == "user" {
                    user_message_index += 1;
                    if let Some(mentions) = mentions_by_user_index.get(&user_message_index) {
                        if let Some(map) = value.as_object_mut() {
                            map.insert("mentions".into(), serde_json::Value::Array(mentions.clone()));
                        }
                    }
                } else if role == "assistant" {
                    assistant_message_index += 1;
                    if let Some(citations) = citations_by_assistant_index.get(&assistant_message_index) {
                        if let Some(map) = value.as_object_mut() {
                            map.insert("citations".into(), serde_json::Value::Array(citations.clone()));
                        }
                    }
                }
                Some(value)
            }).collect::<Vec<_>>(),
            "trace": traces.into_iter().map(|trace| {
                let mut payload = trace.payload;
                if let Some(map) = payload.as_object_mut() {
                    map.insert("kind".into(), serde_json::Value::String(trace.kind.clone()));
                }
                serde_json::json!({
                    "kind": trace.kind,
                    "timestamp": trace.timestamp,
                    "payload": payload,
                })
            }).collect::<Vec<_>>(),
            "compact_summary": compact_summary.map(|summary| serde_json::json!({
                "summary": summary.summary,
                "timestamp": summary.timestamp,
                "message_count": summary.message_count,
            })),
            "latest_usage": latest_usage.map(|usage| serde_json::json!({
                "input_tokens": usage.input_tokens,
                "output_tokens": usage.output_tokens,
                "cache_read_tokens": usage.cache_read_tokens,
                "cache_creation_tokens": usage.cache_creation_tokens,
                "total_tokens": usage.total_tokens(),
                "source": "provider",
            })),
            "llm": entry.llm.map(|config| serde_json::json!({
                "provider": config.provider,
                "model": config.model,
                "api_key_configured": config.api_key.is_some(),
                "base_url": config.base_url,
                "chat_path": config.chat_path,
                "context_window_tokens": config.context_window_tokens,
                "budget_limit_usd": config.budget_limit_usd,
                "require_approval": config.require_approval,
            })),
            "search": entry.search.map(|config| serde_json::json!({
                "provider": config.provider,
                "base_url": config.base_url,
                "api_key_configured": config.api_key.is_some(),
                "max_results": config.max_results,
                "fetch_timeout_secs": config.fetch_timeout_secs,
                "max_fetch_chars": config.max_fetch_chars,
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

    if let Some(notebook_enabled) = req.notebook_enabled {
        let _ = pool.ensure_entry(&id).await;
        if notebook_enabled {
            if !pool.set_knowledge(&id, None, None) || !pool.set_notebook_enabled(&id, true) {
                return (
                    StatusCode::NOT_FOUND,
                    Json(serde_json::json!({ "error": "session not found" })),
                )
                    .into_response();
            }
        } else if !pool.set_notebook_enabled(&id, false) {
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
        if !normalized_kb.is_empty() {
            let _ = pool.set_notebook_enabled(&id, false);
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

    if let Some(search) = req.search {
        let _ = pool.ensure_entry(&id).await;
        let search = search_config_from_request(search);
        if !pool.set_search(&id, search) {
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

async fn append_session_messages(
    State(state): State<Arc<SessionsState>>,
    Path(id): Path<String>,
    Json(req): Json<AppendMessageRequest>,
) -> impl IntoResponse {
    if state.pool.ensure_entry(&id).await.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "session not found" })),
        )
            .into_response();
    }

    let session = match state.pool.open_runtime_session(&id).await {
        Ok(session) => session,
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response();
        }
    };

    if let Some(user) = req.user.filter(|value| !value.trim().is_empty()) {
        if let Err(err) = session
            .append_message(tutor_agent::chat::user_message(&user))
            .await
        {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response();
        }
    }

    if let Some(assistant) = req.assistant.filter(|value| !value.trim().is_empty()) {
        if let Err(err) = session
            .append_message(tutor_agent::chat::assistant_message(&assistant))
            .await
        {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response();
        }
    }

    if let Some(quiz_id) = req.quiz_id.filter(|value| !value.trim().is_empty()) {
        if let Err(err) = state
            .pool
            .append_trace(
                &id,
                "quiz_created",
                serde_json::json!({
                    "capability": "quiz",
                    "quiz_id": quiz_id,
                }),
            )
            .await
        {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response();
        }
    }

    if let Some(citations) = req.assistant_citations.filter(|items| !items.is_empty()) {
        let assistant_message_index = match state.pool.assistant_message_count(&id).await {
            Ok(count) if count > 0 => count,
            Ok(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": "no assistant message to annotate" })),
                )
                    .into_response();
            }
            Err(err) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": err.to_string() })),
                )
                    .into_response();
            }
        };
        if let Err(err) = state
            .pool
            .append_message_citations(&id, assistant_message_index, citations)
            .await
        {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response();
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "id": id, "appended": true })),
    )
        .into_response()
}

async fn append_message_citations(
    State(state): State<Arc<SessionsState>>,
    Path(id): Path<String>,
    Json(req): Json<AppendMessageCitationsRequest>,
) -> impl IntoResponse {
    if state.pool.ensure_entry(&id).await.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "session not found" })),
        )
            .into_response();
    }
    if req.citations.is_empty() {
        return (
            StatusCode::OK,
            Json(serde_json::json!({ "id": id, "appended": false })),
        )
            .into_response();
    }

    let assistant_message_index = match state.pool.assistant_message_count(&id).await {
        Ok(count) if count > 0 => count,
        Ok(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "no assistant message to annotate" })),
            )
                .into_response();
        }
        Err(err) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": err.to_string() })),
            )
                .into_response();
        }
    };

    match state
        .pool
        .append_message_citations(&id, assistant_message_index, req.citations)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "id": id,
                "appended": true,
                "assistant_message_index": assistant_message_index,
            })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

fn llm_config_from_request(config: CreateLlmConfig) -> LlmSessionConfig {
    LlmSessionConfig {
        provider: config.provider,
        model: config.model,
        api_key: config.api_key.filter(|value| !value.trim().is_empty()),
        base_url: config.base_url.filter(|value| !value.trim().is_empty()),
        chat_path: config.chat_path.filter(|value| !value.trim().is_empty()),
        context_window_tokens: config.context_window_tokens,
        budget_limit_usd: config.budget_limit_usd,
        require_approval: config.require_approval.unwrap_or(false),
    }
}

fn search_config_from_request(config: CreateSearchConfig) -> Option<SearchSessionConfig> {
    let provider = config.provider.trim().to_string();
    let base_url = config.base_url.trim().to_string();
    if provider.is_empty() || base_url.is_empty() {
        return None;
    }
    Some(SearchSessionConfig {
        provider,
        base_url,
        api_key: config.api_key.filter(|value| !value.trim().is_empty()),
        max_results: config.max_results,
        fetch_timeout_secs: config.fetch_timeout_secs,
        max_fetch_chars: config.max_fetch_chars,
    })
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
        .route("/api/sessions/{id}/messages", post(append_session_messages))
        .route(
            "/api/sessions/{id}/message-citations",
            post(append_message_citations),
        )
        .with_state(state)
}

fn knowledge_binding(
    knowledge: &KnowledgeStore,
    kb: Option<String>,
    notebook_enabled: bool,
) -> Result<(Option<String>, Option<tutor_rag::EmbeddingConfig>), anyhow::Error> {
    if notebook_enabled {
        return Ok((None, None));
    }

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
