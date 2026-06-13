use std::sync::Arc;

use axum::{
    Router,
    extract::ws::{Message, WebSocket},
    extract::{Path, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
};
use futures::{SinkExt, StreamExt};
use llm_harness_runtime::budget::BudgetControlAdapter;
use llm_harness_runtime::cost::{PricingProvider, TokenPrice};
use llm_harness_runtime_audit_jsonl::JsonlAuditSink;
use llm_harness_runtime_sandbox_os::OsEnv;
use serde::Deserialize;
use tutor_agent::governance::GovernanceConfig;
use tutor_agent::{Capability, CapabilityRouter, LlmConfig, LlmProviderKind};

use crate::session::{LlmSessionConfig, SessionEntry, SessionPool};

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
    let entry = match pool.get(&session_id) {
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
                        run_tutor_message(entry.clone(), content).await;
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

async fn run_tutor_message(entry: SessionEntry, content: String) {
    let result = async {
        let capability: Capability = entry.capability.parse()?;
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
        let router = CapabilityRouter::new(env, llm, governance);
        router.run(capability, &content).await
    }
    .await;

    match result {
        Ok(answer) => {
            let _ = entry.stream.content(&answer, false).await;
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
