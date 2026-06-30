use std::collections::HashMap;
use std::sync::Arc;

use llm_harness_agent::{
    AgentHarness, AgentHarnessEvent, AgentHarnessOptions, HarnessHooks, Session,
};
use llm_harness_types::{
    AfterProviderResponseHook, AgentEvent, AgentMessage, AssistantMessage, ContentBlock,
    StopReason, UserMessage,
};
use tutor_tools::{
    CodeExecTool, RagSearchTool, ReadMemoryTool, WebFetchTool, WebSearchTool, WriteMemoryTool,
};

use crate::capability::CapabilityRouter;
use crate::error::{Result, TutorError};
use crate::event_sink::{emit_content, emit_trace};

/// Run a single Chat turn: question → [rag_search + web_search] → answer.
/// Creates a fresh in-memory harness per call (stateless in v0.1).
pub async fn run_chat(router: &CapabilityRouter, question: &str) -> Result<String> {
    run_chat_with_messages(router, vec![user_message(question)]).await
}

pub async fn run_chat_with_messages(
    router: &CapabilityRouter,
    messages: Vec<AgentMessage>,
) -> Result<String> {
    run_chat_inner(router, "chat", chat_system_prompt(), Some(messages), None).await
}

pub async fn run_chat_with_session(
    router: &CapabilityRouter,
    session: Session,
    question: &str,
) -> Result<String> {
    run_chat_inner(
        router,
        "chat",
        chat_system_prompt(),
        Some(vec![user_message(question)]),
        Some(session),
    )
    .await
}

pub async fn run_research_with_messages(
    router: &CapabilityRouter,
    messages: Vec<AgentMessage>,
) -> Result<String> {
    run_chat_inner(
        router,
        "research",
        research_system_prompt(),
        Some(messages),
        None,
    )
    .await
}

pub async fn run_research_with_session(
    router: &CapabilityRouter,
    session: Session,
    question: &str,
) -> Result<String> {
    run_chat_inner(
        router,
        "research",
        research_system_prompt(),
        Some(vec![user_message(question)]),
        Some(session),
    )
    .await
}

async fn run_chat_inner(
    router: &CapabilityRouter,
    capability: &'static str,
    system_prompt: String,
    messages: Option<Vec<AgentMessage>>,
    session: Option<Session>,
) -> Result<String> {
    emit_trace(
        &router.event_sink,
        "phase_start",
        serde_json::json!({ "capability": capability, "phase": "respond" }),
    )
    .await;
    if capability == "research" {
        emit_trace(
            &router.event_sink,
            "research_stage_start",
            serde_json::json!({
                "capability": "research",
                "stage": "plan",
                "title": "Plan research"
            }),
        )
        .await;
    }

    let rag_tool = router
        .retriever
        .clone()
        .map(RagSearchTool::with_retriever)
        .unwrap_or_else(RagSearchTool::new);
    let rag_tool = match &router.associated_kb {
        Some(kb) => rag_tool.with_associated_kb(kb.clone()),
        None => rag_tool,
    };

    let mut tools: Vec<Arc<dyn llm_harness_types::Tool>> = vec![
        Arc::new(ReadMemoryTool::new()),
        Arc::new(WriteMemoryTool::new()),
        Arc::new(rag_tool),
        Arc::new(match router.web_search.clone() {
            Some(config) => WebSearchTool::with_config(config),
            None => WebSearchTool::new(),
        }),
        Arc::new(match router.web_search.clone() {
            Some(config) => WebFetchTool::with_config(config),
            None => WebFetchTool::new(),
        }),
        Arc::new(CodeExecTool::new()),
    ];
    tools.extend(router.product_tools.iter().cloned());

    let gov = &router.governance;

    let opts = AgentHarnessOptions {
        model: router.llm.model.clone(),
        model_info: Some(router.llm.model_info(8192)),
        tools,
        system_prompt: Some(system_prompt),
        auth: router.auth_hook(),
        hooks: HarnessHooks {
            after_provider_response: vec![gov.budget.clone() as Arc<dyn AfterProviderResponseHook>],
            ..HarnessHooks::none()
        },
        ..AgentHarnessOptions::new(router.llm.model.clone())
    };

    let client = router.make_client();

    let has_session = session.is_some();
    let harness = if let Some(session) = session {
        AgentHarness::with_session(client, router.env.clone(), session, opts)
    } else {
        AgentHarness::new_in_memory(client, router.env.clone(), opts).await
    };
    if has_session {
        try_auto_compact(&harness, router, capability).await;
    }
    let mut rx = harness.subscribe();
    let prompt_task = tokio::spawn(async move {
        harness
            .prompt_with_messages(messages.unwrap_or_default())
            .await
    });

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
                    serde_json::json!({ "capability": capability, "skipped": skipped }),
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
                emit_content(&router.event_sink, text.clone(), true).await;
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
                        "capability": capability,
                        "tool_use_id": tool_use_id,
                        "tool": tool_name,
                        "args": args,
                    }),
                )
                .await;
                if capability == "research" && tool_name == "web_search" {
                    emit_trace(
                        &router.event_sink,
                        "research_search",
                        serde_json::json!({
                            "capability": "research",
                            "stage": "search",
                            "title": "Search web",
                            "payload": { "args": args },
                        }),
                    )
                    .await;
                } else if capability == "research" && tool_name == "web_fetch" {
                    emit_trace(
                        &router.event_sink,
                        "research_read",
                        serde_json::json!({
                            "capability": "research",
                            "stage": "read",
                            "title": "Read source",
                            "payload": { "args": args },
                        }),
                    )
                    .await;
                }
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
                        "capability": capability,
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
    prompt_task
        .await
        .map_err(|err| TutorError::Internal(format!("agent prompt task failed: {err}")))??;

    emit_trace(
        &router.event_sink,
        "phase_end",
        serde_json::json!({ "capability": capability, "phase": "respond" }),
    )
    .await;
    if capability == "research" {
        emit_trace(
            &router.event_sink,
            "research_report_done",
            serde_json::json!({
                "capability": "research",
                "stage": "synthesize",
                "title": "Research report ready",
                "summary": last_text.chars().take(240).collect::<String>(),
            }),
        )
        .await;
    }

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

pub(crate) async fn try_auto_compact(
    harness: &AgentHarness,
    router: &CapabilityRouter,
    capability: &str,
) {
    match harness.compact().await {
        Ok(stats) => {
            emit_trace(
                &router.event_sink,
                "context_compacted",
                serde_json::json!({
                    "capability": capability,
                    "tokens_before": stats.tokens_before,
                    "tokens_after": stats.tokens_after,
                    "compressed_entries": stats.compressed_entries,
                }),
            )
            .await;
        }
        Err(err) if err.to_string().contains("not enough tokens to compact") => {}
        Err(err) => {
            emit_trace(
                &router.event_sink,
                "context_compaction_skipped",
                serde_json::json!({
                    "capability": capability,
                    "reason": err.to_string(),
                }),
            )
            .await;
        }
    }
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

fn chat_system_prompt() -> String {
    "You are a knowledgeable tutor. Use read_memory when personalization is relevant, \
     such as prior weaknesses, learning preferences, recent study state, follow-up teaching, \
     review, practice, or adapting explanation style. Memory is only learner profile/context; \
     do not treat it as an external factual source. Use write_memory only when the user explicitly \
     asks you to remember something or clearly approves recording a durable preference; never infer \
     private profile facts or silently write ordinary chat content. Use rag_search to find relevant course material. \
     When the user references Space artifacts such as Notebook entries, Quiz sessions, or Quiz questions, \
     call read_space_item before relying on their content. Do not guess the contents of a referenced Space item. \
     Web verification rules are strict: when the user asks you to collect facts, trivia, \
     current information, latest information, sources, external references, or information \
     about real-world/public entities, products, games, communities, papers, libraries, \
     events, or online content, you must call web_search before answering. After web_search, \
     use web_fetch to read important source pages before making citation-backed or factual \
     claims. If web_search or web_fetch fails, say what could not be verified instead of \
     inventing facts from memory. Use code_exec when the user asks to run or verify code. \
     For non-trivial numeric calculations, approximations, transcendental functions, \
     statistics, simulations, or any answer where exact arithmetic matters, call code_exec \
     with Python to compute or verify the result before answering. Answer clearly and \
     concisely."
        .into()
}

fn research_system_prompt() -> String {
    "You are a research tutor. Your job is to turn the user's topic into a sourced, reusable research report. \
     Use read_memory only to adapt the report to the learner's preferences, scope, or prior weaknesses; \
     never use memory as a factual source. Use write_memory only when the user explicitly asks you to remember \
     a durable preference or approves recording it; research findings belong in reports, not memory. \
     Follow this workflow: (1) briefly identify the research question and scope, \
     (2) optionally call read_memory when personalization is relevant, (3) call web_search for external facts, \
     (4) call web_fetch on the most relevant sources before relying on them, (5) call read_space_item when the user references Notebook or Quiz artifacts, (6) optionally call rag_search when a knowledge base is associated, \
     (7) synthesize a Markdown report. Do not answer research requests from memory when external verification is needed. \
     If search or fetch fails, clearly state what failed and what remains unverified. \
     The final answer must be a Markdown report with these sections: Title, Summary, Key Findings, Analysis, Limitations, Follow-up Questions, Sources. \
     Cite factual claims using numbered source references that match the Sources section. \
     Keep intermediate planning brief; the final report is the main deliverable."
        .into()
}

#[cfg(test)]
mod tests {
    use super::{chat_system_prompt, research_system_prompt};

    #[test]
    fn chat_prompt_requires_web_search_for_fact_collection() {
        let prompt = chat_system_prompt();
        assert!(prompt.contains("Use read_memory when personalization is relevant"));
        assert!(prompt.contains("do not treat it as an external factual source"));
        assert!(prompt.contains("Use write_memory only when the user explicitly"));
        assert!(prompt.contains("never infer"));
        assert!(prompt.contains("call read_space_item"));
        assert!(prompt.contains("collect facts"));
        assert!(prompt.contains("trivia"));
        assert!(prompt.contains("must call web_search before answering"));
        assert!(prompt.contains("If web_search or web_fetch fails"));
    }

    #[test]
    fn research_prompt_requires_search_fetch_and_report() {
        let prompt = research_system_prompt();
        assert!(prompt.contains("Use read_memory only to adapt"));
        assert!(prompt.contains("research findings belong in reports"));
        assert!(prompt.contains("call web_search"));
        assert!(prompt.contains("call web_fetch"));
        assert!(prompt.contains("read_space_item"));
        assert!(prompt.contains("Markdown report"));
        assert!(prompt.contains("Sources"));
    }
}
