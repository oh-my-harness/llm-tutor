use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::Deserialize;
use tutor_agent::llm_provider::{LlmConfig, LlmProviderKind};

use crate::memory_store::{
    MemoryAssistAction, MemoryFact, MemoryStore, MemoryTextEdit, MemoryTextEditOp,
};

#[derive(Deserialize)]
struct MemoryFileQuery {
    path: String,
}

#[derive(Deserialize)]
struct UpdateMemoryFileRequest {
    markdown: String,
}

#[derive(Deserialize)]
struct EventsQuery {
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct ApplyConsolidationRequest {
    target_path: String,
    markdown: String,
}

#[derive(Deserialize)]
struct AssistMemoryRequest {
    target_path: String,
    action: MemoryAssistAction,
    markdown: Option<String>,
    llm: Option<MemoryLlmConfig>,
}

#[derive(Deserialize)]
struct MemoryLlmConfig {
    provider: String,
    model: String,
    api_key: Option<String>,
    base_url: Option<String>,
    chat_path: Option<String>,
    context_window_tokens: Option<u32>,
}

async fn list_files(State(store): State<Arc<MemoryStore>>) -> impl IntoResponse {
    match store.list() {
        Ok(files) => (StatusCode::OK, Json(serde_json::json!({ "files": files }))).into_response(),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn get_file(
    State(store): State<Arc<MemoryStore>>,
    Query(query): Query<MemoryFileQuery>,
) -> impl IntoResponse {
    match store.read(&query.path) {
        Ok(file) => (StatusCode::OK, Json(serde_json::json!({ "file": file }))).into_response(),
        Err(err) => error_response(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn update_file(
    State(store): State<Arc<MemoryStore>>,
    Query(query): Query<MemoryFileQuery>,
    Json(req): Json<UpdateMemoryFileRequest>,
) -> impl IntoResponse {
    match store.write(&query.path, req.markdown) {
        Ok(file) => (StatusCode::OK, Json(serde_json::json!({ "file": file }))).into_response(),
        Err(err) => error_response(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn list_events(
    State(store): State<Arc<MemoryStore>>,
    Query(query): Query<EventsQuery>,
) -> impl IntoResponse {
    match store.recent_events(query.limit.unwrap_or(50).clamp(1, 200)) {
        Ok(events) => (
            StatusCode::OK,
            Json(serde_json::json!({ "events": events })),
        )
            .into_response(),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn preview_consolidation(State(store): State<Arc<MemoryStore>>) -> impl IntoResponse {
    match store.consolidation_preview() {
        Ok(preview) => (
            StatusCode::OK,
            Json(serde_json::json!({ "preview": preview })),
        )
            .into_response(),
        Err(err) => error_response(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn apply_consolidation(
    State(store): State<Arc<MemoryStore>>,
    Json(req): Json<ApplyConsolidationRequest>,
) -> impl IntoResponse {
    match store.write(&req.target_path, req.markdown) {
        Ok(file) => (StatusCode::OK, Json(serde_json::json!({ "file": file }))).into_response(),
        Err(err) => error_response(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn assist_memory(
    State(store): State<Arc<MemoryStore>>,
    Json(req): Json<AssistMemoryRequest>,
) -> impl IntoResponse {
    if let Some(llm) = req.llm {
        return match assist_memory_with_llm(&store, req.target_path, req.action, req.markdown, llm)
            .await
        {
            Ok(result) => (
                StatusCode::OK,
                Json(serde_json::json!({ "result": result })),
            )
                .into_response(),
            Err(err) => error_response(StatusCode::BAD_REQUEST, err),
        };
    }
    match store.assist(&req.target_path, req.action, req.markdown) {
        Ok(result) => (
            StatusCode::OK,
            Json(serde_json::json!({ "result": result })),
        )
            .into_response(),
        Err(err) => error_response(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn assist_memory_with_llm(
    store: &MemoryStore,
    target_path: String,
    action: MemoryAssistAction,
    markdown: Option<String>,
    llm: MemoryLlmConfig,
) -> Result<crate::memory_store::MemoryAssistResult, String> {
    let llm = build_llm_config(llm)?;
    let current = match markdown {
        Some(value) => value,
        None => {
            store
                .read(&target_path)
                .map_err(|err| err.to_string())?
                .markdown
        }
    };
    let input = store
        .consolidation_input(&target_path, action, Some(current.clone()))
        .map_err(|err| err.to_string())?;
    let consolidation_input_json =
        serde_json::to_string_pretty(&input).map_err(|err| err.to_string())?;
    let output = tutor_agent::memory::run_memory_workflow(
        &llm,
        &tutor_agent::memory::MemoryWorkflowInput {
            target_path: target_path.clone(),
            action: match action {
                MemoryAssistAction::Update => tutor_agent::memory::MemoryWorkflowAction::Update,
                MemoryAssistAction::Check => tutor_agent::memory::MemoryWorkflowAction::Check,
                MemoryAssistAction::Dedupe => tutor_agent::memory::MemoryWorkflowAction::Dedupe,
            },
            current_markdown: current,
            consolidation_input_json,
        },
    )
    .await
    .map_err(|err| err.to_string())?;
    let edits = output
        .edits
        .iter()
        .map(|edit| MemoryTextEdit {
            op: match edit.op {
                tutor_agent::memory::MemoryWorkflowEditOp::Replace => MemoryTextEditOp::Replace,
                tutor_agent::memory::MemoryWorkflowEditOp::Delete => MemoryTextEditOp::Delete,
                tutor_agent::memory::MemoryWorkflowEditOp::Insert => MemoryTextEditOp::Insert,
            },
            start_line: edit.start_line,
            end_line: edit.end_line,
            text: edit.text.clone(),
        })
        .collect::<Vec<_>>();
    let proposed_markdown = match action {
        MemoryAssistAction::Update if output.changed => {
            let facts = output
                .facts
                .into_iter()
                .map(|fact| MemoryFact {
                    text: fact.text,
                    section: fact.section,
                    refs: fact.refs,
                })
                .collect::<Vec<_>>();
            Some(
                store
                    .append_memory_facts(
                        &target_path,
                        &input.target.existing_markdown,
                        &facts,
                        &input.chunk.citeable_refs,
                        &input.target.allowed_sections,
                    )
                    .map_err(|err| err.to_string())?,
            )
        }
        MemoryAssistAction::Dedupe if output.changed => Some(
            store
                .apply_text_edits(&input.target.existing_markdown, &edits)
                .map_err(|err| err.to_string())?,
        ),
        _ => output.proposed_markdown,
    };
    Ok(crate::memory_store::MemoryAssistResult {
        target_path,
        action,
        report_markdown: output.report_markdown,
        proposed_markdown,
        edits,
        changed: output.changed,
    })
}

fn build_llm_config(config: MemoryLlmConfig) -> Result<LlmConfig, String> {
    let provider = match config.provider.trim().to_ascii_lowercase().as_str() {
        "anthropic" => LlmProviderKind::Anthropic,
        "deepseek" => LlmProviderKind::DeepSeek,
        "openai" | "openai-compatible" => LlmProviderKind::OpenAI,
        _ => return Err("unsupported LLM provider".into()),
    };
    let model = config.model.trim().to_string();
    if model.is_empty() {
        return Err("memory workflow requires a model".into());
    }
    let api_key = config
        .api_key
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "memory workflow requires an API key".to_string())?;
    Ok(LlmConfig::from_parts(
        provider,
        model,
        api_key,
        config
            .base_url
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        config
            .chat_path
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        config.context_window_tokens.filter(|value| *value > 0),
    ))
}

pub fn memory_router(store: Arc<MemoryStore>) -> Router {
    Router::new()
        .route("/api/memory/files", get(list_files))
        .route("/api/memory/file", get(get_file).patch(update_file))
        .route("/api/memory/events", get(list_events))
        .route(
            "/api/memory/consolidate/preview",
            axum::routing::post(preview_consolidation),
        )
        .route(
            "/api/memory/consolidate/apply",
            axum::routing::post(apply_consolidation),
        )
        .route("/api/memory/assist", axum::routing::post(assist_memory))
        .with_state(store)
}

fn error_response(status: StatusCode, message: String) -> axum::response::Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn lists_and_updates_memory_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(MemoryStore::new_with_root(dir.path().join("memory")));
        let app = memory_router(store);

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/memory/files")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert!(body["files"].as_array().unwrap().len() >= 3);

        let response = app
            .oneshot(json_request(
                Method::PATCH,
                "/api/memory/file?path=L3%2Fprofile.md",
                serde_json::json!({ "markdown": "# Student profile\n\n- Needs review. <!--m_01-->" }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert!(
            body["file"]["markdown"]
                .as_str()
                .unwrap()
                .contains("Needs review")
        );
    }

    #[tokio::test]
    async fn previews_and_applies_consolidation() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(MemoryStore::new_with_root(dir.path().join("memory")));
        store
            .record_event(
                crate::memory_store::MemoryEventCategory::Chat,
                "answered",
                "Explained lithography",
                Some("session-1".into()),
                serde_json::json!({}),
            )
            .unwrap();
        let app = memory_router(store);

        let response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/memory/consolidate/preview",
                serde_json::json!({}),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["preview"]["target_path"], "L3/recent.md");
        assert!(
            body["preview"]["proposed_markdown"]
                .as_str()
                .unwrap()
                .contains("Explained lithography")
        );

        let response = app
            .oneshot(json_request(
                Method::POST,
                "/api/memory/consolidate/apply",
                serde_json::json!({
                    "target_path": "L3/recent.md",
                    "markdown": "# Recent learning context\n\n- Reviewed lithography."
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert!(
            body["file"]["markdown"]
                .as_str()
                .unwrap()
                .contains("Reviewed")
        );
    }

    #[tokio::test]
    async fn assists_memory_check_and_dedupe() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(MemoryStore::new_with_root(dir.path().join("memory")));
        let app = memory_router(store);

        let markdown = "# Quiz memory\n\n- Same fact. <!--m_01-->\n- Same fact. <!--m_02-->\n\n[^1]: quiz:q1\n[^1]: quiz:q1";
        let response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/memory/assist",
                serde_json::json!({
                    "target_path": "L2/quiz.md",
                    "action": "check",
                    "markdown": markdown
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert!(
            body["result"]["report_markdown"]
                .as_str()
                .unwrap()
                .contains("Duplicate bullets")
        );

        let response = app
            .oneshot(json_request(
                Method::POST,
                "/api/memory/assist",
                serde_json::json!({
                    "target_path": "L2/quiz.md",
                    "action": "dedupe",
                    "markdown": markdown
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["result"]["changed"], true);
    }

    fn json_request(method: Method, uri: &str, value: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(value.to_string()))
            .unwrap()
    }

    async fn response_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }
}
