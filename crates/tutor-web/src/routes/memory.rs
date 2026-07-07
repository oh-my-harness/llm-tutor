use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use llm_harness_runtime_sandbox_os::OsEnv;
use llm_harness_types::ExecutionEnv;
use serde::Deserialize;
use tutor_agent::llm_provider::{LlmConfig, LlmProviderKind};

use crate::memory_store::{
    MemoryAssistAction, MemoryAssistTrace, MemoryAssistTraceChunk, MemoryFact, MemoryStore,
    MemoryTextEdit, MemoryTextEditOp,
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
struct SourceQuery {
    reference: String,
}

#[derive(Deserialize)]
struct ApplyConsolidationRequest {
    target_path: String,
    markdown: String,
}

#[derive(Deserialize)]
struct UndoMemoryRequest {
    target_path: String,
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

#[derive(Clone)]
pub(crate) struct MemoryState {
    store: Arc<MemoryStore>,
    workflow_root: PathBuf,
}

async fn list_files(State(state): State<MemoryState>) -> impl IntoResponse {
    match state.store.list() {
        Ok(files) => (StatusCode::OK, Json(serde_json::json!({ "files": files }))).into_response(),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn get_file(
    State(state): State<MemoryState>,
    Query(query): Query<MemoryFileQuery>,
) -> impl IntoResponse {
    match state.store.read(&query.path) {
        Ok(file) => (StatusCode::OK, Json(serde_json::json!({ "file": file }))).into_response(),
        Err(err) => error_response(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn update_file(
    State(state): State<MemoryState>,
    Query(query): Query<MemoryFileQuery>,
    Json(req): Json<UpdateMemoryFileRequest>,
) -> impl IntoResponse {
    match state.store.write(&query.path, req.markdown) {
        Ok(file) => (StatusCode::OK, Json(serde_json::json!({ "file": file }))).into_response(),
        Err(err) => error_response(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn list_events(
    State(state): State<MemoryState>,
    Query(query): Query<EventsQuery>,
) -> impl IntoResponse {
    match state
        .store
        .recent_events(query.limit.unwrap_or(50).clamp(1, 200))
    {
        Ok(events) => (
            StatusCode::OK,
            Json(serde_json::json!({ "events": events })),
        )
            .into_response(),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn get_source(
    State(state): State<MemoryState>,
    Query(query): Query<SourceQuery>,
) -> impl IntoResponse {
    match state.store.resolve_source_ref(&query.reference) {
        Ok(source) => (
            StatusCode::OK,
            Json(serde_json::json!({ "source": source })),
        )
            .into_response(),
        Err(err) => error_response(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn preview_consolidation(State(state): State<MemoryState>) -> impl IntoResponse {
    match state.store.consolidation_preview() {
        Ok(preview) => (
            StatusCode::OK,
            Json(serde_json::json!({ "preview": preview })),
        )
            .into_response(),
        Err(err) => error_response(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn apply_consolidation(
    State(state): State<MemoryState>,
    Json(req): Json<ApplyConsolidationRequest>,
) -> impl IntoResponse {
    match state.store.write(&req.target_path, req.markdown) {
        Ok(file) => (StatusCode::OK, Json(serde_json::json!({ "file": file }))).into_response(),
        Err(err) => error_response(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn undo_memory(
    State(state): State<MemoryState>,
    Json(req): Json<UndoMemoryRequest>,
) -> impl IntoResponse {
    match state.store.undo_latest_write(&req.target_path) {
        Ok(result) => (
            StatusCode::OK,
            Json(serde_json::json!({ "result": result })),
        )
            .into_response(),
        Err(err) => error_response(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn assist_memory(
    State(state): State<MemoryState>,
    Json(req): Json<AssistMemoryRequest>,
) -> impl IntoResponse {
    if let Some(llm) = req.llm {
        return match assist_memory_with_llm(
            &state.store,
            &state.workflow_root,
            req.target_path,
            req.action,
            req.markdown,
            llm,
        )
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
    match state
        .store
        .assist(&req.target_path, req.action, req.markdown)
    {
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
    workflow_root: &PathBuf,
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
    if action == MemoryAssistAction::Update {
        return assist_memory_update_with_llm(store, workflow_root, target_path, current, &llm)
            .await;
    }
    let input = store
        .consolidation_input(&target_path, action, Some(current.clone()))
        .map_err(|err| err.to_string())?;
    let consolidation_input_json =
        serde_json::to_string_pretty(&input).map_err(|err| err.to_string())?;
    let workflow_input = tutor_agent::memory::MemoryWorkflowInput {
        target_path: target_path.clone(),
        action: match action {
            MemoryAssistAction::Update => tutor_agent::memory::MemoryWorkflowAction::Update,
            MemoryAssistAction::Check => tutor_agent::memory::MemoryWorkflowAction::Check,
            MemoryAssistAction::Dedupe => tutor_agent::memory::MemoryWorkflowAction::Dedupe,
        },
        current_markdown: current,
        consolidation_input_json,
    };
    let run = run_memory_runtime_workflow(&llm, workflow_root, &workflow_input)
        .await
        .map_err(|err| err.to_string())?;
    let output_json = serde_json::to_string_pretty(&serde_json::json!({
        "output": &run.output,
        "runtime_cost": &run.cost,
    }))
    .map_err(|err| err.to_string())?;
    let output = run.output;
    let edits = workflow_edits_to_memory_edits(&output.edits);
    store
        .validate_text_edits_for_action(
            action,
            &input.target.existing_markdown,
            &edits,
            &input.chunk.citeable_refs,
        )
        .map_err(|err| err.to_string())?;
    let proposed_markdown = if action == MemoryAssistAction::Dedupe && output.changed {
        Some(
            store
                .apply_text_edits(&input.target.existing_markdown, &edits)
                .map_err(|err| err.to_string())?,
        )
    } else {
        output.proposed_markdown.clone()
    };
    Ok(crate::memory_store::MemoryAssistResult {
        target_path,
        action,
        report_markdown: output.report_markdown,
        proposed_markdown,
        facts: Vec::new(),
        edits,
        trace: Some(MemoryAssistTrace {
            input_json: serde_json::to_string_pretty(&input).map_err(|err| err.to_string())?,
            output_json,
            chunks: vec![MemoryAssistTraceChunk {
                index: input.chunk.index,
                total: input.chunk.total,
                citeable_refs: input.chunk.citeable_refs.clone(),
                status: "done".into(),
            }],
        }),
        changed: output.changed,
    })
}

async fn assist_memory_update_with_llm(
    store: &MemoryStore,
    workflow_root: &PathBuf,
    target_path: String,
    current: String,
    llm: &LlmConfig,
) -> Result<crate::memory_store::MemoryAssistResult, String> {
    let inputs = store
        .consolidation_inputs(
            &target_path,
            MemoryAssistAction::Update,
            Some(current.clone()),
        )
        .map_err(|err| err.to_string())?;
    let mut outputs = Vec::new();
    let mut runtime_costs = Vec::new();
    let mut facts = Vec::new();
    let mut citeable_refs = Vec::<String>::new();
    let mut trace_chunks = Vec::new();

    for input in &inputs {
        for reference in &input.chunk.citeable_refs {
            if !citeable_refs.iter().any(|item| item == reference) {
                citeable_refs.push(reference.clone());
            }
        }
        let consolidation_input_json =
            serde_json::to_string_pretty(input).map_err(|err| err.to_string())?;
        let workflow_input = tutor_agent::memory::MemoryWorkflowInput {
            target_path: target_path.clone(),
            action: tutor_agent::memory::MemoryWorkflowAction::Update,
            current_markdown: current.clone(),
            consolidation_input_json,
        };
        let run = run_memory_runtime_workflow(llm, workflow_root, &workflow_input)
            .await
            .map_err(|err| err.to_string())?;
        runtime_costs.push(run.cost);
        let output = run.output;
        for fact in &output.facts {
            facts.push(MemoryFact {
                text: fact.text.clone(),
                section: fact.section.clone(),
                refs: fact.refs.clone(),
            });
        }
        trace_chunks.push(MemoryAssistTraceChunk {
            index: input.chunk.index,
            total: input.chunk.total,
            citeable_refs: input.chunk.citeable_refs.clone(),
            status: "done".into(),
        });
        outputs.push(output);
    }

    let first_input = inputs
        .first()
        .ok_or_else(|| "memory workflow did not build any input chunks".to_string())?;
    let changed = outputs.iter().any(|output| output.changed) && !facts.is_empty();
    let proposed_markdown = if changed {
        Some(
            store
                .append_memory_facts(
                    &target_path,
                    &first_input.target.existing_markdown,
                    &facts,
                    &citeable_refs,
                    &first_input.target.allowed_sections,
                )
                .map_err(|err| err.to_string())?,
        )
    } else {
        None
    };
    let report_markdown = outputs
        .iter()
        .map(|output| output.report_markdown.trim())
        .filter(|report| !report.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");
    Ok(crate::memory_store::MemoryAssistResult {
        target_path,
        action: MemoryAssistAction::Update,
        report_markdown: if report_markdown.is_empty() {
            "No update facts were returned.".into()
        } else {
            report_markdown
        },
        proposed_markdown,
        facts,
        edits: Vec::new(),
        trace: Some(MemoryAssistTrace {
            input_json: serde_json::to_string_pretty(&inputs).map_err(|err| err.to_string())?,
            output_json: serde_json::to_string_pretty(&serde_json::json!({
                "outputs": outputs,
                "runtime_costs": runtime_costs,
            }))
            .map_err(|err| err.to_string())?,
            chunks: trace_chunks,
        }),
        changed,
    })
}

fn workflow_edits_to_memory_edits(
    edits: &[tutor_agent::memory::MemoryWorkflowEdit],
) -> Vec<MemoryTextEdit> {
    edits
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
            refs: edit.refs.clone(),
            reason: edit.reason.clone(),
        })
        .collect()
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

async fn run_memory_runtime_workflow(
    llm: &LlmConfig,
    workflow_root: &PathBuf,
    input: &tutor_agent::memory::MemoryWorkflowInput,
) -> tutor_agent::Result<tutor_agent::memory::MemoryWorkflowRun> {
    let cwd = std::env::current_dir()
        .map_err(|err| tutor_agent::TutorError::Internal(err.to_string()))?;
    let env = Arc::new(OsEnv::new(cwd)) as Arc<dyn ExecutionEnv>;
    let client = llm.build_client();
    let engine_config = tutor_agent::runtime_engine::build_workflow_engine_config(
        client,
        llm.model.clone(),
        env,
        workflow_root.join("memory"),
    );
    tutor_agent::memory::run_memory_workflow_with_runtime(input, engine_config).await
}

pub fn memory_router(store: Arc<MemoryStore>, workflow_root: impl Into<PathBuf>) -> Router {
    let state = MemoryState {
        store,
        workflow_root: workflow_root.into(),
    };
    Router::new()
        .route("/api/memory/files", get(list_files))
        .route("/api/memory/file", get(get_file).patch(update_file))
        .route("/api/memory/events", get(list_events))
        .route("/api/memory/source", get(get_source))
        .route(
            "/api/memory/consolidate/preview",
            axum::routing::post(preview_consolidation),
        )
        .route(
            "/api/memory/consolidate/apply",
            axum::routing::post(apply_consolidation),
        )
        .route("/api/memory/undo", axum::routing::post(undo_memory))
        .route("/api/memory/assist", axum::routing::post(assist_memory))
        .with_state(state)
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
        let app = memory_router(store, dir.path().join("workflow-sessions"));

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
        let app = memory_router(store, dir.path().join("workflow-sessions"));

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
        let app = memory_router(store, dir.path().join("workflow-sessions"));

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

    #[tokio::test]
    async fn undo_restores_latest_memory_write() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(MemoryStore::new_with_root(dir.path().join("memory")));
        store
            .write("L2/chat.md", "# Chat memory\n\n- Original.".into())
            .unwrap();
        let app = memory_router(store, dir.path().join("workflow-sessions"));

        let response = app
            .clone()
            .oneshot(json_request(
                Method::PATCH,
                "/api/memory/file?path=L2%2Fchat.md",
                serde_json::json!({ "markdown": "# Chat memory\n\n- Changed." }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .oneshot(json_request(
                Method::POST,
                "/api/memory/undo",
                serde_json::json!({ "target_path": "L2/chat.md" }),
            ))
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert!(
            body["result"]["file"]["markdown"]
                .as_str()
                .unwrap()
                .contains("Original")
        );
    }

    #[tokio::test]
    async fn resolves_memory_source_refs() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(MemoryStore::new_with_root(dir.path().join("memory")));
        store
            .record_event(
                crate::memory_store::MemoryEventCategory::Quiz,
                "answered",
                "Answered OPC question correctly",
                Some("quiz-1".into()),
                serde_json::json!({ "question_id": "q1" }),
            )
            .unwrap();
        let app = memory_router(store, dir.path().join("workflow-sessions"));

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/memory/source?reference=quiz%3Aquiz-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["source"]["reference"], "quiz:quiz-1");
        assert_eq!(
            body["source"]["event"]["summary"],
            "Answered OPC question correctly"
        );
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
