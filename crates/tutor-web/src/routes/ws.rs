use std::sync::Arc;

use axum::{
    Router,
    extract::ws::{Message, WebSocket},
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
};
use futures::{SinkExt, StreamExt, future::BoxFuture};
use llm_harness_runtime::budget::BudgetControlAdapter;
use llm_harness_runtime::cost::{PricingProvider, TokenPrice};
use llm_harness_runtime_audit_jsonl::JsonlAuditSink;
use llm_harness_runtime_sandbox_os::OsEnv;
use serde::Deserialize;
use tutor_agent::event_sink::{EventSink, SharedEventSink};
use tutor_agent::governance::GovernanceConfig;
use tutor_agent::{Capability, CapabilityRouter, LlmConfig, LlmProviderKind};
use tutor_rag::KnowledgeRetriever;

use crate::session::{LlmSessionConfig, SessionEntry, SessionPool};

#[derive(Clone)]
struct PersistedEventSink {
    pool: Arc<SessionPool>,
    session_id: String,
    stream: crate::stream::TutorStream,
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
}

struct NoOpPricing;

impl PricingProvider for NoOpPricing {
    fn price_for(&self, _model: &str, _provider: &str) -> Option<TokenPrice> {
        Some(TokenPrice {
            input_per_mtok: 0.0,
            output_per_mtok: 0.0,
            cache_read_per_mtok: 0.0,
            cache_write_per_mtok: 0.0,
        })
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum ClientMessage {
    #[serde(rename = "message")]
    Message { content: String },
    #[serde(rename = "approval_response")]
    ApprovalResponse { request_id: String, approved: bool },
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(pool): State<Arc<SessionPool>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, pool, session_id))
}

async fn handle_socket(socket: WebSocket, pool: Arc<SessionPool>, session_id: String) {
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

    while let Some(Ok(msg)) = ws_stream.next().await {
        match msg {
            Message::Text(text) => {
                let parsed = serde_json::from_str::<ClientMessage>(&text);
                match parsed {
                    Ok(ClientMessage::Message { content }) => {
                        let active_entry = pool
                            .ensure_entry(&session_id)
                            .await
                            .unwrap_or_else(|| entry.clone());
                        run_tutor_message(pool.clone(), active_entry, content).await;
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

    send_task.abort();
}

pub fn ws_router(pool: Arc<SessionPool>) -> Router {
    Router::new()
        .route("/ws/sessions/{session_id}", get(ws_handler))
        .with_state(pool)
}

async fn run_tutor_message(pool: Arc<SessionPool>, entry: SessionEntry, content: String) {
    let history_len = pool.history_len(&entry.id).await + 1;
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

    let result = async {
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
        let budget = Arc::new(BudgetControlAdapter::new(
            Arc::new(NoOpPricing),
            budget_limit,
            None,
        ));
        let audit_path = std::env::temp_dir().join(format!("tutor_web_{}.jsonl", entry.id));
        let audit = Arc::new(JsonlAuditSink::new(&audit_path));
        let require_approval = entry
            .llm
            .as_ref()
            .map(|config| config.require_approval)
            .unwrap_or(false);
        let governance = GovernanceConfig::new(budget, Some(audit), require_approval);
        let sink: SharedEventSink = Arc::new(PersistedEventSink {
            pool: pool.clone(),
            session_id: entry.id.clone(),
            stream: entry.stream.clone(),
        });
        let mut router = CapabilityRouter::new(env, llm, governance).with_event_sink(sink);
        if let Some(embedding) = entry.embedding.clone() {
            let retriever =
                tutor_rag::LanceDbRag::new(tutor_rag::LanceDbRag::default_root(), embedding);
            router = router.with_retriever(Arc::new(retriever));
        }
        if let Some(kb) = entry.kb.clone() {
            router = router.with_associated_kb(kb);
        }
        router
            .run_with_session(capability, runtime_session, &content)
            .await
    }
    .await;

    match result {
        Ok(answer) => {
            emit_rag_citations(&pool, &entry, &content).await;
            let _ = entry.stream.content(&answer, false).await;
            let _ = pool.refresh_compact_summary(&entry.id).await;
            let history_len = pool.history_len(&entry.id).await;
            let _ = entry
                .stream
                .status(
                    "done",
                    serde_json::json!({
                        "capability": entry.capability,
                        "history_len": history_len,
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

async fn emit_rag_citations(pool: &SessionPool, entry: &SessionEntry, query: &str) {
    let (Some(kb), Some(embedding)) = (entry.kb.as_ref(), entry.embedding.clone()) else {
        return;
    };
    let retriever = tutor_rag::LanceDbRag::new(tutor_rag::LanceDbRag::default_root(), embedding);
    let Ok(hits) = retriever.search(Some(kb), query, 5).await else {
        return;
    };
    if hits.is_empty() {
        return;
    }
    let sources = hits
        .iter()
        .enumerate()
        .map(|(index, hit)| {
            serde_json::json!({
                "index": index + 1,
                "id": hit.id,
                "kb": hit.kb,
                "source": hit.source,
                "text": hit.text,
                "score": hit.score,
            })
        })
        .collect::<Vec<_>>();
    let payload = serde_json::json!({
        "capability": entry.capability,
        "tool": "rag_search",
        "details": {
            "query": query,
            "kb": kb,
            "hits": sources.len(),
            "configured": true,
            "sources": sources,
        }
    });
    let _ = pool
        .append_trace(&entry.id, "rag_citations", payload.clone())
        .await;
    let _ = entry.stream.trace("rag_citations", payload).await;
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
    ))
}
