use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use axum::{
    Router,
    extract::ws::{Message, WebSocket},
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
};
use futures::{SinkExt, StreamExt, future::BoxFuture};
use llm_harness_runtime_audit_jsonl::JsonlAuditSink;
use llm_harness_runtime_sandbox_os::OsEnv;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;
use tutor_agent::event_sink::{EventSink, SharedEventSink};
use tutor_agent::governance::GovernanceConfig;
use tutor_agent::{Capability, CapabilityRouter, LlmConfig, LlmProviderKind};

use crate::knowledge_store::KnowledgeStore;
use crate::memory_store::{MemoryEventCategory, MemoryStore};
use crate::notebook_store::NotebookStore;
use crate::quiz_store::QuizStore;
use crate::quiz_tool::{CreateQuizTool, ProposeQuizPlanTool};
use crate::routes::quiz::CreateLlmConfig;
use crate::routes::space::{SpaceMention, resolve_space_mention_markdown};
use crate::session::{LlmSessionConfig, SearchSessionConfig, SessionEntry, SessionPool};
use crate::space_tool::{
    ListNotebookTreeTool, ProposeNotebookEditTool, ReadSpaceItemTool, SearchNotebookTool,
};

#[derive(Clone)]
struct WsState {
    pool: Arc<SessionPool>,
    knowledge: Arc<KnowledgeStore>,
    memory: Arc<MemoryStore>,
    notebook: Arc<NotebookStore>,
    quizzes: Arc<QuizStore>,
    rag_root: PathBuf,
}

#[derive(Clone)]
struct PersistedEventSink {
    pool: Arc<SessionPool>,
    session_id: String,
    stream: crate::stream::TutorStream,
    streamed_content: Arc<AtomicBool>,
}

impl EventSink for PersistedEventSink {
    fn trace(&self, kind: String, data: serde_json::Value) -> BoxFuture<'static, ()> {
        let pool = self.pool.clone();
        let session_id = self.session_id.clone();
        let stream = self.stream.clone();
        Box::pin(async move {
            let _ = pool.append_trace(&session_id, &kind, data.clone()).await;
            stream.trace(&kind, data).await;
        })
    }

    fn content(&self, text: String, chunk: bool) -> BoxFuture<'static, ()> {
        let stream = self.stream.clone();
        let streamed_content = self.streamed_content.clone();
        Box::pin(async move {
            if chunk {
                streamed_content.store(true, Ordering::SeqCst);
            }
            stream.content(&text, chunk).await;
        })
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum ClientMessage {
    #[serde(rename = "message")]
    Message {
        content: String,
        mentions: Option<Vec<SpaceMention>>,
    },
    #[serde(rename = "stop")]
    Stop,
    #[serde(rename = "approval_response")]
    ApprovalResponse { request_id: String, approved: bool },
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<WsState>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state, session_id))
}

async fn handle_socket(socket: WebSocket, state: WsState, session_id: String) {
    let pool = state.pool.clone();
    let entry = match pool.ensure_entry(&session_id).await {
        Some(e) => e,
        None => return,
    };
    let mut event_rx = entry.stream.subscribe();

    let (mut ws_sink, mut ws_stream) = socket.split();

    // Forward events from the agent harness to the WebSocket client
    let send_task = tokio::spawn(async move {
        while let Ok(event) = event_rx.recv().await {
            let json = match serde_json::to_string(&event) {
                Ok(j) => j,
                Err(_) => continue,
            };
            if ws_sink.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    let mut active_cancel: Option<CancellationToken> = None;
    let mut active_task: Option<tokio::task::JoinHandle<()>> = None;

    while let Some(Ok(msg)) = ws_stream.next().await {
        if active_task.as_ref().is_some_and(|task| task.is_finished()) {
            if let Some(task) = active_task.take() {
                let _ = task.await;
            }
            active_cancel = None;
        }
        match msg {
            Message::Text(text) => {
                let parsed = serde_json::from_str::<ClientMessage>(&text);
                match parsed {
                    Ok(ClientMessage::Message { content, mentions }) => {
                        if active_task.is_some() {
                            let _ = entry
                                .stream
                                .status(
                                    "error",
                                    serde_json::json!({
                                        "message": "agent is already running"
                                    }),
                                )
                                .await;
                            continue;
                        }
                        let active_entry = pool
                            .ensure_entry(&session_id)
                            .await
                            .unwrap_or_else(|| entry.clone());
                        let cancel = CancellationToken::new();
                        active_cancel = Some(cancel.clone());
                        active_task = Some(tokio::spawn(run_tutor_message(
                            pool.clone(),
                            state.knowledge.clone(),
                            state.memory.clone(),
                            state.notebook.clone(),
                            state.quizzes.clone(),
                            state.rag_root.clone(),
                            active_entry,
                            content,
                            mentions.unwrap_or_default(),
                            cancel,
                        )));
                    }
                    Ok(ClientMessage::Stop) => {
                        if let Some(cancel) = active_cancel.take() {
                            cancel.cancel();
                        }
                    }
                    Ok(ClientMessage::ApprovalResponse {
                        request_id,
                        approved,
                    }) => {
                        let _ = entry
                            .stream
                            .status(
                                "approval_response_received",
                                serde_json::json!({
                                    "request_id": request_id,
                                    "approved": approved,
                                }),
                            )
                            .await;
                    }
                    Err(err) => {
                        let _ = entry
                            .stream
                            .status(
                                "error",
                                serde_json::json!({
                                    "message": format!("invalid websocket message: {err}"),
                                }),
                            )
                            .await;
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    if let Some(cancel) = active_cancel {
        cancel.cancel();
    }
    if let Some(task) = active_task {
        task.abort();
    }
    send_task.abort();
}

pub fn ws_router(
    pool: Arc<SessionPool>,
    knowledge: Arc<KnowledgeStore>,
    memory: Arc<MemoryStore>,
    notebook: Arc<NotebookStore>,
    quizzes: Arc<QuizStore>,
    rag_root: impl Into<PathBuf>,
) -> Router {
    let state = WsState {
        pool,
        knowledge,
        memory,
        notebook,
        quizzes,
        rag_root: rag_root.into(),
    };
    Router::new()
        .route("/ws/sessions/{session_id}", get(ws_handler))
        .with_state(state)
}

async fn run_tutor_message(
    pool: Arc<SessionPool>,
    knowledge: Arc<KnowledgeStore>,
    memory: Arc<MemoryStore>,
    notebook: Arc<NotebookStore>,
    quizzes: Arc<QuizStore>,
    rag_root: PathBuf,
    entry: SessionEntry,
    content: String,
    mentions: Vec<SpaceMention>,
    cancel: CancellationToken,
) {
    let history_len = pool.history_len(&entry.id).await + 1;
    let user_message_index = next_user_message_index(&pool, &entry.id).await;
    if !mentions.is_empty() {
        let _ = pool
            .append_message_mentions(
                &entry.id,
                user_message_index,
                mentions
                    .iter()
                    .map(|mention| serde_json::to_value(mention).unwrap_or_default())
                    .collect(),
            )
            .await;
    }
    let _ = entry
        .stream
        .status(
            "running",
            serde_json::json!({
                "capability": entry.capability,
                "history_len": history_len,
            }),
        )
        .await;

    let work = async {
        let capability: Capability = entry.capability.parse()?;
        let runtime_session = pool
            .open_runtime_session(&entry.id)
            .await
            .map_err(|err| tutor_agent::TutorError::Internal(err.to_string()))?;
        let llm = llm_config_for_session(entry.llm.clone())?;
        let cwd = std::env::current_dir()
            .map_err(|err| tutor_agent::TutorError::Internal(err.to_string()))?;
        let env = Arc::new(OsEnv::new(cwd));
        let budget_limit = entry
            .llm
            .as_ref()
            .and_then(|config| config.budget_limit_usd)
            .unwrap_or(2.0);
        let audit_path = std::env::temp_dir().join(format!("tutor_web_{}.jsonl", entry.id));
        let audit = Arc::new(JsonlAuditSink::new(&audit_path));
        let require_approval = entry
            .llm
            .as_ref()
            .map(|config| config.require_approval)
            .unwrap_or(false);
        let governance = GovernanceConfig::new(budget_limit, Some(audit), require_approval);
        let streamed_content = Arc::new(AtomicBool::new(false));
        let sink: SharedEventSink = Arc::new(PersistedEventSink {
            pool: pool.clone(),
            session_id: entry.id.clone(),
            stream: entry.stream.clone(),
            streamed_content: streamed_content.clone(),
        });
        let mut router = CapabilityRouter::new(env, llm, governance)
            .with_event_sink(sink)
            .with_workflow_root(rag_root.join("workflow-sessions"))
            .with_product_tool(Arc::new(ReadSpaceItemTool::new(
                notebook.clone(),
                quizzes.clone(),
            )));
        if entry.capability == "quiz" {
            router = router
                .with_product_tool(Arc::new(ProposeQuizPlanTool))
                .with_product_tool(Arc::new(CreateQuizTool::new(
                    quizzes.clone(),
                    knowledge.clone(),
                    notebook.clone(),
                    memory.clone(),
                    rag_root.clone(),
                    rag_root.join("workflow-sessions").join("quiz"),
                    entry.kb.clone(),
                    create_quiz_llm_config_for_session(entry.llm.clone()),
                )));
        }
        if entry.notebook_enabled {
            router = router
                .with_product_tool(Arc::new(ListNotebookTreeTool::new(notebook.clone())))
                .with_product_tool(Arc::new(SearchNotebookTool::new(notebook.clone())));
        }
        if entry.capability == "organize" {
            router =
                router.with_product_tool(Arc::new(ProposeNotebookEditTool::new(notebook.clone())));
        }
        if let Some(search) = web_search_config_for_session(entry.search.clone()) {
            router = router.with_web_search(search);
        }
        if let Some(embedding) = entry.embedding.clone() {
            let retriever = tutor_rag::LanceDbRag::new(rag_root, embedding);
            router = router.with_retriever(Arc::new(retriever));
        }
        if let Some(kb) = entry.kb.clone() {
            router = router.with_associated_kb(kb);
        }
        let resolved_content =
            resolve_message_content_with_space_mentions(&notebook, &quizzes, &content, &mentions);
        if !mentions.is_empty() {
            let _ = entry
                .stream
                .status(
                    "space_context",
                    serde_json::json!({
                        "count": mentions.len(),
                        "resolved": resolved_content.resolved_count,
                    }),
                )
                .await;
        }
        let answer = router
            .run_with_session(capability, runtime_session, &resolved_content.content)
            .await?;
        Ok((answer, streamed_content.load(Ordering::SeqCst)))
    };

    let result: tutor_agent::Result<(String, bool)> = tokio::select! {
        _ = cancel.cancelled() => {
            let _ = entry
                .stream
                .status(
                    "stopped",
                    serde_json::json!({
                        "capability": entry.capability,
                    }),
                )
                .await;
            let _ = entry.stream.content("", false).await;
            return;
        }
        result = work => result,
    };

    match result {
        Ok((answer, streamed)) => {
            if matches!(entry.capability.as_str(), "chat" | "research" | "organize") {
                let category = if entry.capability == "research" {
                    MemoryEventCategory::Research
                } else {
                    MemoryEventCategory::Chat
                };
                let _ = memory.record_event(
                    category,
                    "answered",
                    summarize_exchange(&content, &answer),
                    Some(entry.id.clone()),
                    serde_json::json!({
                        "session_id": entry.id,
                        "capability": entry.capability,
                    "user": content.chars().take(500).collect::<String>(),
                    "space_mentions": mentions.iter().map(|mention| serde_json::json!({
                        "id": mention.id,
                        "type": mention.mention_type,
                        "target_id": mention.target_id,
                        "question_id": mention.question_id,
                        "title": mention.title,
                    })).collect::<Vec<_>>(),
                        "assistant": answer.chars().take(1000).collect::<String>(),
                    }),
                );
            }
            let final_text = if streamed { "" } else { &answer };
            let _ = entry.stream.content(final_text, false).await;
            let history_len = pool.history_len(&entry.id).await;
            let latest_usage = pool.latest_usage(&entry.id).await.ok().flatten();
            let context_window_tokens = entry
                .llm
                .as_ref()
                .and_then(|config| config.context_window_tokens)
                .unwrap_or(200_000);
            let _ = entry
                .stream
                .status(
                    "done",
                    serde_json::json!({
                        "capability": entry.capability,
                        "history_len": history_len,
                        "context_window_tokens": context_window_tokens,
                        "usage": latest_usage.map(|usage| serde_json::json!({
                            "input_tokens": usage.input_tokens,
                            "output_tokens": usage.output_tokens,
                            "cache_read_tokens": usage.cache_read_tokens,
                            "cache_creation_tokens": usage.cache_creation_tokens,
                            "total_tokens": usage.total_tokens(),
                            "source": "provider",
                        })),
                    }),
                )
                .await;
        }
        Err(err) => {
            let _ = entry
                .stream
                .status(
                    "error",
                    serde_json::json!({
                        "message": err.to_string(),
                    }),
                )
                .await;
            let _ = entry.stream.content(&format!("Error: {err}"), false).await;
        }
    }
}

async fn next_user_message_index(pool: &SessionPool, session_id: &str) -> usize {
    pool.messages(session_id)
        .await
        .map(|messages| {
            messages
                .iter()
                .filter(|message| matches!(crate::session::message_role(message), Some("user")))
                .count()
                + 1
        })
        .unwrap_or(1)
}

struct ResolvedMessageContent {
    content: String,
    resolved_count: usize,
}

fn resolve_message_content_with_space_mentions(
    notebook: &NotebookStore,
    quizzes: &QuizStore,
    content: &str,
    mentions: &[SpaceMention],
) -> ResolvedMessageContent {
    if mentions.is_empty() {
        return ResolvedMessageContent {
            content: content.to_string(),
            resolved_count: 0,
        };
    }

    let mut resolved_count = 0usize;
    let mut blocks = Vec::new();
    for mention in mentions.iter().take(8) {
        let Some((resolved_id, _markdown)) =
            resolve_space_mention_markdown(notebook, quizzes, mention)
        else {
            continue;
        };
        resolved_count += 1;
        let path = mention
            .metadata
            .get("path")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        blocks.push(format!(
            "- id: {}; item_type: {}; target_id: {}; question_id: {}; title: {}; path: {}",
            resolved_id,
            mention_type_name(&mention.mention_type),
            mention.target_id.as_deref().unwrap_or(""),
            mention.question_id.as_deref().unwrap_or(""),
            mention.title,
            path
        ));
    }

    if blocks.is_empty() {
        return ResolvedMessageContent {
            content: content.to_string(),
            resolved_count,
        };
    }

    ResolvedMessageContent {
        content: format!(
            "The user explicitly referenced these Space artifacts. Use the read_space_item tool to inspect exact content before relying on a referenced item, and identify the artifact when you use it.\n\n{}\n\nUser message:\n{}",
            blocks.join("\n"),
            content
        ),
        resolved_count,
    }
}

fn mention_type_name(value: &crate::routes::space::SpaceMentionType) -> &'static str {
    match value {
        crate::routes::space::SpaceMentionType::NotebookEntry => "notebook_entry",
        crate::routes::space::SpaceMentionType::QuizSession => "quiz_session",
        crate::routes::space::SpaceMentionType::QuizQuestion => "quiz_question",
    }
}

fn summarize_exchange(user: &str, assistant: &str) -> String {
    let user = user.split_whitespace().collect::<Vec<_>>().join(" ");
    let assistant = assistant.split_whitespace().collect::<Vec<_>>().join(" ");
    format!(
        "User asked: {}; assistant answered: {}",
        user.chars().take(160).collect::<String>(),
        assistant.chars().take(220).collect::<String>()
    )
}

fn web_search_config_for_session(
    config: Option<SearchSessionConfig>,
) -> Option<tutor_tools::WebSearchConfig> {
    let config = config?;
    Some(tutor_tools::WebSearchConfig {
        provider: config.provider,
        base_url: config.base_url,
        api_key: config.api_key,
        max_results: config.max_results.unwrap_or(5).clamp(1, 10),
        fetch_timeout_secs: config.fetch_timeout_secs.unwrap_or(12).clamp(3, 60),
        max_fetch_chars: config
            .max_fetch_chars
            .unwrap_or(12_000)
            .clamp(1_000, 60_000),
    })
}

fn llm_config_for_session(config: Option<LlmSessionConfig>) -> tutor_agent::Result<LlmConfig> {
    let Some(config) = config else {
        return LlmConfig::from_env();
    };

    let provider = match config.provider.as_str() {
        "anthropic" | "claude" => LlmProviderKind::Anthropic,
        "deepseek" => LlmProviderKind::DeepSeek,
        "openai" | "openai-compatible" => LlmProviderKind::OpenAI,
        other => {
            return Err(tutor_agent::TutorError::Internal(format!(
                "unsupported LLM provider `{other}`"
            )));
        }
    };

    let api_key = config
        .api_key
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| tutor_agent::TutorError::Internal("LLM API key is not configured".into()))?;

    if config.model.trim().is_empty() {
        return Err(tutor_agent::TutorError::Internal(
            "LLM model is not configured".into(),
        ));
    }

    Ok(LlmConfig::from_parts(
        provider,
        config.model,
        api_key,
        config.base_url,
        config.chat_path,
        config.context_window_tokens,
    ))
}

fn create_quiz_llm_config_for_session(config: Option<LlmSessionConfig>) -> Option<CreateLlmConfig> {
    let config = config?;
    Some(CreateLlmConfig {
        provider: config.provider,
        model: config.model,
        api_key: config.api_key,
        base_url: config.base_url,
        chat_path: config.chat_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notebook_store::{NotebookEntryInput, NotebookEntryType};

    #[test]
    fn resolves_space_mentions_into_turn_context() {
        let dir = tempfile::tempdir().unwrap();
        let notebook = NotebookStore::new_with_path(dir.path().join("notebook"));
        let quizzes = QuizStore::new_with_path(dir.path().join("quizzes.json"));
        let entry = notebook
            .create(NotebookEntryInput {
                space_id: None,
                entry_type: NotebookEntryType::Note,
                path: None,
                title: "Mask notes".into(),
                markdown: "Alignment marks are used during lithography.".into(),
                metadata: None,
                source_session_id: None,
                source_message_id: None,
            })
            .unwrap();

        let resolved = resolve_message_content_with_space_mentions(
            &notebook,
            &quizzes,
            "summarize this",
            &[SpaceMention {
                id: format!("notebook_entry:{}", entry.id),
                mention_type: crate::routes::space::SpaceMentionType::NotebookEntry,
                target_id: Some(entry.id),
                question_id: None,
                title: "Mask notes".into(),
                preview: None,
                metadata: serde_json::json!({}),
            }],
        );

        assert_eq!(resolved.resolved_count, 1);
        assert!(resolved.content.contains("read_space_item"));
        assert!(resolved.content.contains("notebook_entry:"));
        assert!(!resolved.content.contains("Alignment marks"));
        assert!(resolved.content.contains("User message:\nsummarize this"));
    }
}
