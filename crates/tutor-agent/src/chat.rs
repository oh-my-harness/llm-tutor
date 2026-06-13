use std::sync::Arc;

use llm_adapter::anthropic::AnthropicProvider;
use llm_harness::{AgentHarness, AgentHarnessEvent, AgentHarnessOptions};
use llm_harness_runtime_auth::EnvAuthHook;
use llm_harness_types::{AgentEvent, ContentBlock};
use tutor_tools::{RagSearchTool, WebSearchTool};

use crate::capability::CapabilityRouter;
use crate::error::Result;

/// Run a single Chat turn: question → [rag_search + web_search] → answer.
/// Creates a fresh in-memory harness per call (stateless in v0.1).
pub async fn run_chat(router: &CapabilityRouter, question: &str) -> Result<String> {
    let tools: Vec<Arc<dyn llm_harness_types::Tool>> = vec![
        Arc::new(RagSearchTool::new()),
        Arc::new(WebSearchTool::new()),
    ];

    let opts = AgentHarnessOptions {
        model: router.model.clone(),
        tools,
        system_prompt: Some(
            "You are a knowledgeable tutor. Use rag_search to find relevant course material, \
             web_search for supplementary information, then answer clearly and concisely."
                .into(),
        ),
        auth: Some(Arc::new(EnvAuthHook::for_provider("anthropic"))),
        ..AgentHarnessOptions::new(router.model.clone())
    };

    // AnthropicProvider is the concrete LlmClient implementation.
    let client = Arc::new(AnthropicProvider::builder(&router.anthropic_api_key).build());

    // Subscribe before prompt() so we don't miss any events.
    let harness = AgentHarness::new_in_memory(client, router.env.clone(), opts).await;
    let mut rx = harness.subscribe();

    harness.prompt(question).await?;

    // Collect the last complete assistant message.
    let mut last_text = String::new();
    while let Ok(event) = rx.recv().await {
        match event.as_ref() {
            AgentHarnessEvent::Agent(AgentEvent::MessageEnd { message, .. }) => {
                for block in &message.content {
                    if let ContentBlock::Text { text } = block {
                        last_text = text.clone();
                    }
                }
            }
            AgentHarnessEvent::Settled | AgentHarnessEvent::Aborted => break,
            _ => {}
        }
    }

    Ok(if last_text.is_empty() {
        "(no response)".into()
    } else {
        last_text
    })
}
