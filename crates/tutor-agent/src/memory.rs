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
    pub consolidation_input_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryWorkflowOutput {
    pub report_markdown: String,
    pub proposed_markdown: Option<String>,
    pub facts: Vec<MemoryWorkflowFact>,
    pub edits: Vec<MemoryWorkflowEdit>,
    pub changed: bool,
}

#[derive(Debug, Deserialize)]
struct MemoryWorkflowJson {
    report_markdown: String,
    proposed_markdown: Option<String>,
    #[serde(default)]
    facts: Vec<MemoryWorkflowFact>,
    #[serde(default)]
    edits: Vec<MemoryWorkflowEdit>,
    changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryWorkflowFact {
    pub text: String,
    pub section: String,
    pub refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryWorkflowEdit {
    pub op: MemoryWorkflowEditOp,
    pub start_line: usize,
    pub end_line: Option<usize>,
    pub text: Option<String>,
    #[serde(default)]
    pub refs: Vec<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryWorkflowEditOp {
    Replace,
    Delete,
    Insert,
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
    if proposed_markdown.is_some() {
        return Err(TutorError::Internal(
            "memory workflow must not return proposed_markdown".into(),
        ));
    }
    if action == MemoryWorkflowAction::Update {
        for fact in &parsed.facts {
            if fact.text.trim().is_empty() || fact.section.trim().is_empty() || fact.refs.is_empty()
            {
                return Err(TutorError::Internal(
                    "update workflow returned an invalid memory fact".into(),
                ));
            }
        }
        if parsed.changed && parsed.facts.is_empty() {
            return Err(TutorError::Internal(
                "changed update workflow requires facts".into(),
            ));
        }
        if !parsed.edits.is_empty() {
            return Err(TutorError::Internal(
                "update workflow must not return edits".into(),
            ));
        }
    } else {
        if !parsed.facts.is_empty() {
            return Err(TutorError::Internal(
                "check and dedupe workflows must not return facts".into(),
            ));
        }
        for edit in &parsed.edits {
            validate_workflow_edit(edit)?;
        }
        if action == MemoryWorkflowAction::Dedupe && parsed.changed && parsed.edits.is_empty() {
            return Err(TutorError::Internal(
                "changed dedupe workflow requires edits".into(),
            ));
        }
    }
    Ok(MemoryWorkflowOutput {
        report_markdown,
        proposed_markdown,
        facts: parsed.facts,
        edits: parsed.edits,
        changed: parsed.changed,
    })
}

fn validate_workflow_edit(edit: &MemoryWorkflowEdit) -> Result<()> {
    if edit.start_line == 0 {
        return Err(TutorError::Internal(
            "memory edit start_line must be >= 1".into(),
        ));
    }
    match edit.op {
        MemoryWorkflowEditOp::Replace => {
            let end_line = edit.end_line.unwrap_or(edit.start_line);
            if end_line < edit.start_line {
                return Err(TutorError::Internal(
                    "memory replace edit has invalid line range".into(),
                ));
            }
            if edit.text.as_deref().unwrap_or_default().trim().is_empty() {
                return Err(TutorError::Internal(
                    "memory replace edit requires text".into(),
                ));
            }
        }
        MemoryWorkflowEditOp::Delete => {
            let end_line = edit.end_line.unwrap_or(edit.start_line);
            if end_line < edit.start_line {
                return Err(TutorError::Internal(
                    "memory delete edit has invalid line range".into(),
                ));
            }
        }
        MemoryWorkflowEditOp::Insert => {
            if edit.text.as_deref().unwrap_or_default().trim().is_empty() {
                return Err(TutorError::Internal(
                    "memory insert edit requires text".into(),
                ));
            }
        }
    }
    Ok(())
}

fn system_prompt() -> String {
    "You maintain visible learner memory for a tutor product. Return only valid JSON. Memory is for personalization, learning state, preferences, weaknesses, scope, and teaching strategy; it is not a factual source. Preserve useful Markdown structure, remove duplicates, avoid inventing unsupported personal facts, and keep changes concise and auditable.".into()
}

fn memory_prompt(input: &MemoryWorkflowInput) -> String {
    let action_rules = match input.action {
        MemoryWorkflowAction::Update => {
            "- Return memory facts in facts; proposed_markdown must be null.\n- Extract durable observations from the normalized consolidation input, not a raw event log.\n- Prefer learning-relevant facts over one-off chatter.\n- Each fact must cite only refs listed in chunk.citeableRefs.\n- Use only sections listed in target.allowedSections."
        }
        MemoryWorkflowAction::Check => {
            "- Do not return proposed_markdown.\n- Report contradictions, stale facts, missing evidence markers, duplicate points, unclear wording, and risky overgeneralizations.\n- If useful, include suggested line edits in edits, but the product may show them as recommendations instead of applying them.\n- Each replace or insert edit should include refs from chunk.citeableRefs when evidence exists and a short reason."
        }
        MemoryWorkflowAction::Dedupe => {
            "- Return line edits in edits; proposed_markdown must be null.\n- Merge duplicate or overlapping bullets.\n- Preserve source markers and references when still useful.\n- Do not delete unique useful memory.\n- Use replace/delete/insert edits against the current Markdown line numbers.\n- Include a short reason for each edit; include refs when they remain relevant."
        }
    };
    let source = if input.consolidation_input_json.trim().is_empty() {
        "(none)".to_string()
    } else {
        input.consolidation_input_json.clone()
    };
    format!(
        "Target memory file: {target_path}\nAction: {action:?}\n\nRules:\n{action_rules}\n\nCurrent Markdown with line numbers:\n```text\n{numbered_current}\n```\n\nNormalized consolidation input:\n```json\n{source}\n```\n\nReturn JSON exactly like:\n{{\"report_markdown\":\"# Memory report\\n...\",\"proposed_markdown\":null,\"facts\":[{{\"text\":\"concise learner fact\",\"section\":\"one allowed section\",\"refs\":[\"chat:source-id\"]}}],\"edits\":[{{\"op\":\"delete\",\"start_line\":7,\"end_line\":7,\"text\":null,\"refs\":[\"chat:source-id\"],\"reason\":\"duplicate of line 5\"}}],\"changed\":true}}\n\nFor update, edits must be [] and proposed_markdown must be null. For check and dedupe, facts must be [] and proposed_markdown must be null.",
        target_path = input.target_path,
        action = input.action,
        numbered_current = line_numbered_markdown(&input.current_markdown),
        source = source,
    )
}

fn line_numbered_markdown(markdown: &str) -> String {
    markdown
        .lines()
        .enumerate()
        .map(|(index, line)| format!("{:>4}: {}", index + 1, line))
        .collect::<Vec<_>>()
        .join("\n")
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
            "facts": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "text": { "type": "string" },
                        "section": { "type": "string" },
                        "refs": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    },
                    "required": ["text", "section", "refs"]
                }
            },
            "edits": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "op": { "type": "string", "enum": ["replace", "delete", "insert"] },
                        "start_line": { "type": "integer", "minimum": 1 },
                        "end_line": {
                            "anyOf": [
                                { "type": "integer", "minimum": 1 },
                                { "type": "null" }
                            ]
                        },
                        "text": {
                            "anyOf": [
                                { "type": "string" },
                                { "type": "null" }
                            ]
                        },
                        "refs": {
                            "type": "array",
                            "items": { "type": "string" }
                        },
                        "reason": {
                            "anyOf": [
                                { "type": "string" },
                                { "type": "null" }
                            ]
                        }
                    },
                    "required": ["op", "start_line", "end_line", "text", "refs", "reason"]
                }
            },
            "changed": { "type": "boolean" }
        },
        "required": ["report_markdown", "proposed_markdown", "facts", "edits", "changed"]
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
            r##"{"report_markdown":"# Report\n\nLooks consistent.","proposed_markdown":null,"facts":[],"edits":[],"changed":false}"##,
            MemoryWorkflowAction::Check,
        )
        .unwrap();
        assert!(!output.changed);
        assert!(output.proposed_markdown.is_none());
    }

    #[test]
    fn rejects_changed_update_without_facts() {
        let err = parse_memory_workflow_output(
            r##"{"report_markdown":"# Report","proposed_markdown":null,"facts":[],"edits":[],"changed":true}"##,
            MemoryWorkflowAction::Update,
        )
        .unwrap_err();
        assert!(err.to_string().contains("requires facts"));
    }

    #[test]
    fn rejects_changed_dedupe_without_edits() {
        let err = parse_memory_workflow_output(
            r##"{"report_markdown":"# Report","proposed_markdown":null,"facts":[],"edits":[],"changed":true}"##,
            MemoryWorkflowAction::Dedupe,
        )
        .unwrap_err();
        assert!(err.to_string().contains("requires edits"));
    }

    #[tokio::test]
    async fn workflow_uses_llm_json_response() {
        let provider = Arc::new(MockProvider {
            text: r##"{"report_markdown":"# Memory report\n\nExtracted recent quiz weakness.","proposed_markdown":null,"facts":[{"text":"Learner should review OPC distractors.","section":"Weak topics","refs":["quiz:q1"]}],"edits":[],"changed":true}"##.into(),
            json_schema: true,
        });
        let input = MemoryWorkflowInput {
            target_path: "L2/quiz.md".into(),
            action: MemoryWorkflowAction::Update,
            current_markdown: "# Quiz memory\n\n".into(),
            consolidation_input_json: r#"{"chunk":{"citeableRefs":["quiz:q1"]},"target":{"allowedSections":["Weak topics"]}}"#.into(),
        };
        let output = run_memory_workflow_with_client(provider, "mock-model", &input)
            .await
            .unwrap();
        assert!(output.changed);
        assert_eq!(output.facts[0].refs, vec!["quiz:q1"]);
    }

    #[test]
    fn parses_dedupe_edits() {
        let output = parse_memory_workflow_output(
            r##"{"report_markdown":"# Report\n\nRemoved duplicate.","proposed_markdown":null,"facts":[],"edits":[{"op":"delete","start_line":4,"end_line":4,"text":null,"refs":["quiz:q1"],"reason":"duplicate of line 3"}],"changed":true}"##,
            MemoryWorkflowAction::Dedupe,
        )
        .unwrap();

        assert_eq!(output.edits.len(), 1);
        assert_eq!(output.edits[0].op, MemoryWorkflowEditOp::Delete);
        assert_eq!(output.edits[0].refs, vec!["quiz:q1"]);
        assert_eq!(
            output.edits[0].reason.as_deref(),
            Some("duplicate of line 3")
        );
    }
}
