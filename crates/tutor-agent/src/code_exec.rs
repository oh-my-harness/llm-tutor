use std::sync::Arc;

use llm_harness_agent::{
    AgentHarness, AgentHarnessEvent, AgentHarnessOptions, HarnessHooks, Session,
};
use llm_harness_runtime::composite::CompositeBeforeToolCallHook;
use llm_harness_types::{
    AfterProviderResponseHook, AgentEvent, AgentMessage, BeforeToolCallHook, ContentBlock,
};
use tutor_tools::CodeExecTool;

use crate::capability::CapabilityRouter;
use crate::error::{Result, TutorError};
use crate::event_sink::emit_trace;

/// Run code execution as an agent turn: the model calls `code_exec`, then explains the result.
pub async fn run_code_exec(router: &CapabilityRouter, request: &str) -> Result<String> {
    run_code_exec_with_messages(router, vec![crate::chat::user_message(request)]).await
}

pub async fn run_code_exec_with_messages(
    router: &CapabilityRouter,
    messages: Vec<AgentMessage>,
) -> Result<String> {
    run_code_exec_inner(router, Some(messages), None).await
}

pub async fn run_code_exec_with_session(
    router: &CapabilityRouter,
    session: Session,
    request: &str,
) -> Result<String> {
    run_code_exec_inner(
        router,
        Some(vec![crate::chat::user_message(request)]),
        Some(session),
    )
    .await
}

async fn run_code_exec_inner(
    router: &CapabilityRouter,
    messages: Option<Vec<AgentMessage>>,
    session: Option<Session>,
) -> Result<String> {
    emit_trace(
        &router.event_sink,
        "phase_start",
        serde_json::json!({ "capability": "code_exec", "phase": "execute" }),
    )
    .await;

    let tools: Vec<Arc<dyn llm_harness_types::Tool>> = vec![Arc::new(CodeExecTool::new())];
    let before_tool_call: Vec<Arc<dyn BeforeToolCallHook>> =
        if let Some(approval) = &router.governance.approval {
            vec![Arc::new(CompositeBeforeToolCallHook::new(vec![
                approval.clone() as Arc<dyn BeforeToolCallHook>,
            ]))]
        } else {
            vec![]
        };

    let opts = AgentHarnessOptions {
        model: router.llm.model.clone(),
        tools,
        system_prompt: Some(
            "You are a code execution tutor. When the user asks to run code, \
             call code_exec with the correct language and code, then explain stdout, stderr, \
             and exit code clearly. For non-trivial numeric calculations or approximations, \
             call code_exec with Python to compute or verify the result before answering. If no \
             runnable code or computable task is provided, ask for the missing details."
                .into(),
        ),
        auth: router.auth_hook(),
        hooks: HarnessHooks {
            after_provider_response: vec![
                router.governance.budget.clone() as Arc<dyn AfterProviderResponseHook>
            ],
            before_tool_call,
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

    let mut last_text = String::new();
    let mut last_error: Option<String> = None;
    loop {
        let event = match rx.recv().await {
            Ok(event) => event,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                emit_trace(
                    &router.event_sink,
                    "event_lagged",
                    serde_json::json!({ "capability": "code_exec", "skipped": skipped }),
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
                emit_trace(
                    &router.event_sink,
                    "tool_call",
                    serde_json::json!({
                        "capability": "code_exec",
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
                emit_trace(
                    &router.event_sink,
                    "tool_result",
                    serde_json::json!({
                        "capability": "code_exec",
                        "tool_use_id": tool_use_id,
                        "ok": result.is_ok(),
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
        serde_json::json!({ "capability": "code_exec", "phase": "execute" }),
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
