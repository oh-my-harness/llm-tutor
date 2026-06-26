use std::sync::Arc;

use llm_adapter::Provider;
use llm_adapter::types::{ChatRequest, Message, RequestContent, ResponseContent, ResponseFormat};
use serde::{Deserialize, Serialize};

use crate::error::{Result, TutorError};
use crate::llm_provider::LlmConfig;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryWorkflowAction {
    Update,
    Check,
    Dedupe,
}

#[derive(Debug, Clone)]
pub struct MemoryWorkflowInput {
    pub target_path: String,
    pub action: MemoryWorkflowAction,
    pub current_markdown: String,
    pub recent_events_markdown: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryWorkflowOutput {
    pub report_markdown: String,
    pub proposed_markdown: Option<String>,
    pub changed: bool,
}

#[derive(Debug, Deserialize)]
struct MemoryWorkflowJson {
    report_markdown: String,
    proposed_markdown: Option<String>,
    changed: bool,
}

pub async fn run_memory_workflow(
    llm: &LlmConfig,
    input: &MemoryWorkflowInput,
) -> Result<MemoryWorkflowOutput> {
    run_memory_workflow_with_client(llm.build_client(), &llm.model, input).await
}

pub async fn run_memory_workflow_with_client(
    client: Arc<dyn Provider>,
    model: &str,
    input: &MemoryWorkflowInput,
) -> Result<MemoryWorkflowOutput> {
    let prompt = memory_prompt(input);
    let mut builder = ChatRequest::builder(model, 2048)
        .message(Message::System(system_prompt()))
        .message(Message::User(vec![RequestContent::Text(prompt)]))
        .temperature(0.1);

    if client.capabilities().supports_json_schema() {
        builder = builder.response_format(ResponseFormat::JsonSchema {
            name: "memory_workflow".into(),
            schema: memory_schema(),
            strict: Some(true),
        });
    } else {
        builder = builder.response_format(ResponseFormat::JsonObject);
    }

    let response = client
        .chat(&builder.build())
        .await
        .map_err(|err| TutorError::Internal(format!("memory LLM workflow failed: {err}")))?;
    parse_memory_workflow_output(&response_text(&response.content), input.action)
}

pub fn parse_memory_workflow_output(
    text: &str,
    action: MemoryWorkflowAction,
) -> Result<MemoryWorkflowOutput> {
    let json_text = extract_json_object(text)
        .ok_or_else(|| TutorError::Internal("memory LLM output did not contain JSON".into()))?;
    let parsed: MemoryWorkflowJson = serde_json::from_str(json_text)
        .map_err(|err| TutorError::Internal(format!("invalid memory workflow JSON: {err}")))?;
    let report_markdown = parsed.report_markdown.trim().to_string();
    if report_markdown.is_empty() {
        return Err(TutorError::Internal(
            "memory workflow report is empty".into(),
        ));
    }
    let proposed_markdown = parsed
        .proposed_markdown
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if action == MemoryWorkflowAction::Check && proposed_markdown.is_some() {
        return Err(TutorError::Internal(
            "check workflow must not return proposed_markdown".into(),
        ));
    }
    if parsed.changed && action != MemoryWorkflowAction::Check && proposed_markdown.is_none() {
        return Err(TutorError::Internal(
            "changed memory workflow requires proposed_markdown".into(),
        ));
    }
    Ok(MemoryWorkflowOutput {
        report_markdown,
        proposed_markdown,
        changed: parsed.changed,
    })
}

fn system_prompt() -> String {
    "You maintain visible learner memory for a tutor product. Return only valid JSON. Memory is for personalization, learning state, preferences, weaknesses, scope, and teaching strategy; it is not a factual source. Preserve useful Markdown structure, remove duplicates, avoid inventing unsupported personal facts, and keep changes concise and auditable.".into()
}

fn memory_prompt(input: &MemoryWorkflowInput) -> String {
    let action_rules = match input.action {
        MemoryWorkflowAction::Update => {
            "- Produce a revised Markdown document in proposed_markdown.\n- Consolidate recent events into stable learner memory, not a raw event log.\n- Prefer durable observations over one-off details.\n- Keep existing useful content and improve organization."
        }
        MemoryWorkflowAction::Check => {
            "- Do not change the document.\n- proposed_markdown must be null.\n- Report contradictions, stale facts, missing evidence markers, duplicate points, unclear wording, and risky overgeneralizations."
        }
        MemoryWorkflowAction::Dedupe => {
            "- Produce a revised Markdown document in proposed_markdown.\n- Merge duplicate or overlapping bullets.\n- Preserve source markers and references when still useful.\n- Do not delete unique useful memory."
        }
    };
    let events = if input.recent_events_markdown.trim().is_empty() {
        "(none)".to_string()
    } else {
        input.recent_events_markdown.clone()
    };
    format!(
        "Target memory file: {target_path}\nAction: {action:?}\n\nRules:\n{action_rules}\n\nCurrent Markdown:\n```markdown\n{current}\n```\n\nRecent workspace events:\n```markdown\n{events}\n```\n\nReturn JSON exactly like:\n{{\"report_markdown\":\"# Memory report\\n...\",\"proposed_markdown\":\"# Updated markdown\\n... or null\",\"changed\":true}}",
        target_path = input.target_path,
        action = input.action,
        current = input.current_markdown,
    )
}

fn response_text(content: &[ResponseContent]) -> String {
    content
        .iter()
        .filter_map(|block| match block {
            ResponseContent::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (start <= end).then_some(&text[start..=end])
}

fn memory_schema() -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "report_markdown": { "type": "string" },
            "proposed_markdown": {
                "anyOf": [
                    { "type": "string" },
                    { "type": "null" }
                ]
            },
            "changed": { "type": "boolean" }
        },
        "required": ["report_markdown", "proposed_markdown", "changed"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use llm_adapter::types::{ChatResponse, StopReason};
    use llm_adapter::{LlmError, ProviderCapabilities, StreamHandle};

    struct MockProvider {
        text: String,
        json_schema: bool,
    }

    #[async_trait]
    impl Provider for MockProvider {
        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities::new(false, false, self.json_schema)
        }

        async fn chat(&self, _req: &ChatRequest) -> std::result::Result<ChatResponse, LlmError> {
            Ok(ChatResponse {
                id: "mock".into(),
                model: "mock".into(),
                content: vec![ResponseContent::Text(self.text.clone())],
                stop_reason: StopReason::EndTurn,
                usage: Default::default(),
            })
        }

        async fn chat_stream(
            &self,
            _req: &ChatRequest,
        ) -> std::result::Result<StreamHandle, LlmError> {
            Err(LlmError::InvalidRequest(
                "streaming is not used in this test".into(),
            ))
        }
    }

    #[test]
    fn parses_check_output_without_proposal() {
        let output = parse_memory_workflow_output(
            r##"{"report_markdown":"# Report\n\nLooks consistent.","proposed_markdown":null,"changed":false}"##,
            MemoryWorkflowAction::Check,
        )
        .unwrap();
        assert!(!output.changed);
        assert!(output.proposed_markdown.is_none());
    }

    #[test]
    fn rejects_changed_update_without_proposal() {
        let err = parse_memory_workflow_output(
            r##"{"report_markdown":"# Report","proposed_markdown":null,"changed":true}"##,
            MemoryWorkflowAction::Update,
        )
        .unwrap_err();
        assert!(err.to_string().contains("requires proposed_markdown"));
    }

    #[tokio::test]
    async fn workflow_uses_llm_json_response() {
        let provider = Arc::new(MockProvider {
            text: r##"{"report_markdown":"# Memory report\n\nMerged recent quiz weakness.","proposed_markdown":"# Quiz memory\n\n- Learner should review OPC distractors.","changed":true}"##.into(),
            json_schema: true,
        });
        let input = MemoryWorkflowInput {
            target_path: "L2/quiz.md".into(),
            action: MemoryWorkflowAction::Update,
            current_markdown: "# Quiz memory\n\n".into(),
            recent_events_markdown: "- Missed an OPC distractor.".into(),
        };
        let output = run_memory_workflow_with_client(provider, "mock-model", &input)
            .await
            .unwrap();
        assert!(output.changed);
        assert!(
            output
                .proposed_markdown
                .unwrap()
                .contains("OPC distractors")
        );
    }
}
