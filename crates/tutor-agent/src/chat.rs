use std::collections::HashMap;
use std::sync::Arc;

use llm_harness_agent::{
    AgentHarness, AgentHarnessEvent, AgentHarnessOptions, HarnessHooks, Session,
};
use llm_harness_types::{
    AfterProviderResponseHook, AgentEvent, AgentMessage, AssistantMessage, ContentBlock,
    StopReason, UserMessage,
};
use tutor_tools::{CodeExecTool, RagSearchTool, WebSearchTool};

use crate::capability::CapabilityRouter;
use crate::error::{Result, TutorError};
use crate::event_sink::emit_trace;

/// Run a single Chat turn: question → [rag_search + web_search] → answer.
/// Creates a fresh in-memory harness per call (stateless in v0.1).
pub async fn run_chat(router: &CapabilityRouter, question: &str) -> Result<String> {
    run_chat_with_messages(router, vec![user_message(question)]).await
}

pub async fn run_chat_with_messages(
    router: &CapabilityRouter,
    messages: Vec<AgentMessage>,
) -> Result<String> {
    run_chat_inner(router, Some(messages), None).await
}

pub async fn run_chat_with_session(
    router: &CapabilityRouter,
    session: Session,
    question: &str,
) -> Result<String> {
    run_chat_inner(router, Some(vec![user_message(question)]), Some(session)).await
}

async fn run_chat_inner(
    router: &CapabilityRouter,
    messages: Option<Vec<AgentMessage>>,
    session: Option<Session>,
) -> Result<String> {
    emit_trace(
        &router.event_sink,
        "phase_start",
        serde_json::json!({ "capability": "chat", "phase": "respond" }),
    )
    .await;

    let rag_tool = router
        .retriever
        .clone()
        .map(RagSearchTool::with_retriever)
        .unwrap_or_else(RagSearchTool::new);
    let rag_tool = match &router.associated_kb {
        Some(kb) => rag_tool.with_associated_kb(kb.clone()),
        None => rag_tool,
    };

    let tools: Vec<Arc<dyn llm_harness_types::Tool>> = vec![
        Arc::new(rag_tool),
        Arc::new(WebSearchTool::new()),
        Arc::new(CodeExecTool::new()),
    ];

    let gov = &router.governance;

    let opts = AgentHarnessOptions {
        model: router.llm.model.clone(),
        tools,
        system_prompt: Some(
            "You are a knowledgeable tutor. Use rag_search to find relevant course material, \
             web_search for supplementary information, and code_exec when the user asks to run \
             or verify code. For non-trivial numeric calculations, approximations, transcendental \
             functions, statistics, simulations, or any answer where exact arithmetic matters, call \
             code_exec with Python to compute or verify the result before answering. Answer clearly \
             and concisely."
                .into(),
        ),
        auth: router.auth_hook(),
        hooks: HarnessHooks {
            after_provider_response: vec![gov.budget.clone() as Arc<dyn AfterProviderResponseHook>],
            ..HarnessHooks::none()
        },
        ..AgentHarnessOptions::new(router.llm.model.clone())
    };

    let client = router.make_client();

    let harness = if let Some(session) = session {
        AgentHarness::with_session(client, router.env.clone(), session, opts)
    } else {
        AgentHarness::new_in_memory(client, router.env.clone(), opts).await
    };
    let mut rx = harness.subscribe();

    harness
        .prompt_with_messages(messages.unwrap_or_default())
        .await?;

    // Collect the last complete assistant message.
    let mut last_text = String::new();
    let mut last_error: Option<String> = None;
    let mut tool_names: HashMap<String, String> = HashMap::new();
    loop {
        let event = match rx.recv().await {
            Ok(event) => event,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                emit_trace(
                    &router.event_sink,
                    "event_lagged",
                    serde_json::json!({ "capability": "chat", "skipped": skipped }),
                )
                .await;
                continue;
            }
            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
        };

        match event.as_ref() {
            AgentHarnessEvent::Agent(AgentEvent::MessageEnd { message, .. }) => {
                for block in &message.content {
                    if let ContentBlock::Text { text } = block {
                        last_text = text.clone();
                    }
                }
            }
            AgentHarnessEvent::Agent(AgentEvent::TextDelta { text, .. }) => {
                last_text.push_str(text);
            }
            AgentHarnessEvent::Agent(AgentEvent::ToolExecutionStart {
                tool_use_id,
                tool_name,
                args,
            }) => {
                tool_names.insert(tool_use_id.clone(), tool_name.clone());
                emit_trace(
                    &router.event_sink,
                    "tool_call",
                    serde_json::json!({
                        "capability": "chat",
                        "tool_use_id": tool_use_id,
                        "tool": tool_name,
                        "args": args,
                    }),
                )
                .await;
            }
            AgentHarnessEvent::Agent(AgentEvent::ToolExecutionEnd {
                tool_use_id,
                result,
            }) => {
                let tool_name = tool_names
                    .get(tool_use_id)
                    .cloned()
                    .unwrap_or_else(|| "tool".into());
                let details = result.as_ref().ok().map(|result| result.details.clone());
                emit_trace(
                    &router.event_sink,
                    "tool_result",
                    serde_json::json!({
                        "capability": "chat",
                        "tool_use_id": tool_use_id,
                        "tool": tool_name,
                        "ok": result.is_ok(),
                        "details": details,
                    }),
                )
                .await;
            }
            AgentHarnessEvent::Agent(AgentEvent::Error(err)) => {
                last_error = Some(err.to_string());
            }
            AgentHarnessEvent::Agent(AgentEvent::AgentEnd { new_messages }) => {
                if last_text.is_empty() {
                    last_text = last_assistant_text(new_messages).unwrap_or_default();
                }
            }
            AgentHarnessEvent::Settled | AgentHarnessEvent::Aborted => break,
            _ => {}
        }
    }

    emit_trace(
        &router.event_sink,
        "phase_end",
        serde_json::json!({ "capability": "chat", "phase": "respond" }),
    )
    .await;

    if let Some(error) = last_error {
        return Err(TutorError::Internal(error));
    }

    if last_text.is_empty() {
        return Err(TutorError::Internal(
            "agent settled without assistant text".into(),
        ));
    }

    Ok(last_text)
}

pub fn user_message(text: &str) -> AgentMessage {
    AgentMessage::User(UserMessage {
        content: vec![ContentBlock::Text {
            text: text.to_string(),
        }],
        timestamp: chrono::Utc::now(),
    })
}

pub fn assistant_message(text: &str) -> AgentMessage {
    AgentMessage::Assistant(AssistantMessage {
        content: vec![ContentBlock::Text {
            text: text.to_string(),
        }],
        stop_reason: Some(StopReason::EndTurn),
        timestamp: chrono::Utc::now(),
        provider: None,
        api: None,
        model: None,
        usage: None,
        error_message: None,
    })
}

fn last_assistant_text(messages: &[AgentMessage]) -> Option<String> {
    messages.iter().rev().find_map(|message| {
        let AgentMessage::Assistant(message) = message else {
            return None;
        };

        message.content.iter().rev().find_map(|block| {
            if let ContentBlock::Text { text } = block {
                Some(text.clone())
            } else {
                None
            }
        })
    })
}
