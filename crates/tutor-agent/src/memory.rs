use std::sync::Arc;

use futures::future::BoxFuture;
use llm_adapter::Provider;
use llm_harness_runtime::control::cost::CostAggregate;
use llm_harness_runtime::workflow::engine::{WorkflowEngine, WorkflowEngineConfig};
use llm_harness_runtime::workflow::executor::{ExecutorCtx, StepExecutor};
use llm_harness_runtime::workflow::model::StepResult;
use serde::{Deserialize, Serialize};

use crate::error::{Result, TutorError};
use crate::runtime_engine::RuntimeDeclarativeJudge;
use crate::runtime_workflow::{memory_workflow, validate_memory_workflow};

const MAX_MEMORY_FACT_TEXT_CHARS: usize = 500;

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

pub async fn run_memory_workflow_with_runtime(
    _client: Arc<dyn Provider>,
    _model: &str,
    input: &MemoryWorkflowInput,
    engine_config: WorkflowEngineConfig,
) -> Result<MemoryWorkflowOutput> {
    validate_memory_workflow()?;
    let workflow = memory_workflow();
    let engine = WorkflowEngine::new(
        workflow.clone(),
        engine_config,
        Arc::new(RuntimeDeclarativeJudge),
    )
    .map_err(|err| TutorError::Internal(format!("memory workflow initialization failed: {err}")))?
    .with_executor(
        "tutor.memory.prepare",
        Arc::new(PrepareMemoryWorkflowExecutor {
            input: input.clone(),
        }),
    );

    engine
        .run()
        .await
        .map_err(|err| TutorError::Internal(format!("memory workflow failed: {err}")))?;
    let structured = engine
        .step_history()
        .await
        .into_iter()
        .rev()
        .find(|record| record.step_id == "run_memory")
        .and_then(|record| record.result)
        .and_then(|result| result.structured)
        .ok_or_else(|| TutorError::Internal("memory workflow did not return output".into()))?;
    let output: MemoryWorkflowOutput = serde_json::from_value(structured)
        .map_err(|err| TutorError::Internal(format!("invalid memory workflow output: {err}")))?;
    validate_memory_workflow_output(output, input.action)
}

struct PrepareMemoryWorkflowExecutor {
    input: MemoryWorkflowInput,
}

impl StepExecutor for PrepareMemoryWorkflowExecutor {
    fn execute<'a>(
        &'a self,
        ctx: &'a ExecutorCtx<'a>,
    ) -> BoxFuture<'a, anyhow::Result<StepResult>> {
        Box::pin(async move {
            {
                let mut context = ctx.context.lock().await;
                context.variables.insert(
                    "memory_prompt".into(),
                    serde_json::json!(memory_prompt(&self.input)),
                );
                context
                    .variables
                    .insert("memory_action".into(), serde_json::json!(self.input.action));
                context.variables.insert(
                    "memory_target_path".into(),
                    serde_json::json!(self.input.target_path.clone()),
                );
            }
            Ok(StepResult {
                output: "memory workflow input prepared".into(),
                structured: Some(serde_json::json!({ "prepared": true })),
                tool_calls_count: 0,
                session_id: String::new(),
                cost: CostAggregate::default(),
                started_at: None,
                ended_at: None,
            })
        })
    }
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
    validate_memory_workflow_output(
        MemoryWorkflowOutput {
            report_markdown,
            proposed_markdown,
            facts: parsed.facts,
            edits: parsed.edits,
            changed: parsed.changed,
        },
        action,
    )
}

fn validate_memory_workflow_output(
    output: MemoryWorkflowOutput,
    action: MemoryWorkflowAction,
) -> Result<MemoryWorkflowOutput> {
    let report_markdown = output.report_markdown.trim().to_string();
    if report_markdown.is_empty() {
        return Err(TutorError::Internal(
            "memory workflow report is empty".into(),
        ));
    }
    let proposed_markdown = output
        .proposed_markdown
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if proposed_markdown.is_some() {
        return Err(TutorError::Internal(
            "memory workflow must not return proposed_markdown".into(),
        ));
    }

    if action == MemoryWorkflowAction::Update {
        for fact in &output.facts {
            if fact.text.trim().is_empty() || fact.section.trim().is_empty() || fact.refs.is_empty()
            {
                return Err(TutorError::Internal(
                    "update workflow returned an invalid memory fact".into(),
                ));
            }
            if fact.text.chars().count() > MAX_MEMORY_FACT_TEXT_CHARS {
                return Err(TutorError::Internal(
                    "update workflow returned an overlong memory fact".into(),
                ));
            }
        }
        if output.changed && output.facts.is_empty() {
            return Err(TutorError::Internal(
                "changed update workflow requires facts".into(),
            ));
        }
        if !output.edits.is_empty() {
            return Err(TutorError::Internal(
                "update workflow must not return edits".into(),
            ));
        }
    } else {
        if !output.facts.is_empty() {
            return Err(TutorError::Internal(
                "check and dedupe workflows must not return facts".into(),
            ));
        }
        for edit in &output.edits {
            validate_workflow_edit(edit)?;
        }
        if action == MemoryWorkflowAction::Dedupe
            && output
                .edits
                .iter()
                .any(|edit| edit.op == MemoryWorkflowEditOp::Insert)
        {
            return Err(TutorError::Internal(
                "dedupe workflow must not insert new facts".into(),
            ));
        }
        if action == MemoryWorkflowAction::Dedupe && output.changed && output.edits.is_empty() {
            return Err(TutorError::Internal(
                "changed dedupe workflow requires edits".into(),
            ));
        }
    }
    Ok(MemoryWorkflowOutput {
        report_markdown,
        proposed_markdown,
        facts: output.facts,
        edits: output.edits,
        changed: output.changed,
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

fn extract_json_object(text: &str) -> Option<&str> {
    let text = text.trim();
    (text.starts_with('{') && text.ends_with('}')).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_harness_loop::test_utils::{MockLlmClient, MockResponse, NoOpEnv};
    use llm_harness_types::ExecutionEnv;

    use crate::runtime_engine::build_workflow_engine_config;

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
    fn rejects_non_json_prose_around_workflow_output() {
        let err = parse_memory_workflow_output(
            r##"Here is the JSON: {"report_markdown":"# Report","proposed_markdown":null,"facts":[],"edits":[],"changed":false}"##,
            MemoryWorkflowAction::Check,
        )
        .unwrap_err();
        assert!(err.to_string().contains("did not contain JSON"));
    }

    #[test]
    fn rejects_malformed_workflow_json() {
        let err = parse_memory_workflow_output(
            r##"{"report_markdown":"# Report","proposed_markdown":null,"facts":[],"edits":[],"changed":false"##,
            MemoryWorkflowAction::Check,
        )
        .unwrap_err();
        assert!(err.to_string().contains("did not contain JSON"));
    }

    #[test]
    fn rejects_workflow_proposed_markdown() {
        let err = parse_memory_workflow_output(
            r##"{"report_markdown":"# Report","proposed_markdown":"# Model wrote markdown","facts":[],"edits":[],"changed":false}"##,
            MemoryWorkflowAction::Check,
        )
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("must not return proposed_markdown")
        );
    }

    #[test]
    fn rejects_update_edits() {
        let err = parse_memory_workflow_output(
            r##"{"report_markdown":"# Report","proposed_markdown":null,"facts":[{"text":"Learner should review OPC.","section":"Weak topics","refs":["quiz:q1"]}],"edits":[{"op":"delete","start_line":4,"end_line":4,"text":null,"refs":["quiz:q1"],"reason":"not allowed in update"}],"changed":true}"##,
            MemoryWorkflowAction::Update,
        )
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("update workflow must not return edits")
        );
    }

    #[test]
    fn rejects_check_facts() {
        let err = parse_memory_workflow_output(
            r##"{"report_markdown":"# Report","proposed_markdown":null,"facts":[{"text":"Learner should review OPC.","section":"Weak topics","refs":["quiz:q1"]}],"edits":[],"changed":false}"##,
            MemoryWorkflowAction::Check,
        )
        .unwrap_err();
        assert!(err.to_string().contains("must not return facts"));
    }

    #[test]
    fn rejects_overlong_update_fact() {
        let text = "x".repeat(MAX_MEMORY_FACT_TEXT_CHARS + 1);
        let payload = serde_json::json!({
            "report_markdown": "# Report",
            "proposed_markdown": null,
            "facts": [{ "text": text, "section": "Weak topics", "refs": ["quiz:q1"] }],
            "edits": [],
            "changed": true
        });

        let err = parse_memory_workflow_output(&payload.to_string(), MemoryWorkflowAction::Update)
            .unwrap_err();

        assert!(err.to_string().contains("overlong memory fact"));
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

    #[test]
    fn rejects_dedupe_insert() {
        let err = parse_memory_workflow_output(
            r##"{"report_markdown":"# Report","proposed_markdown":null,"facts":[],"edits":[{"op":"insert","start_line":4,"end_line":null,"text":"- New fact.","refs":["quiz:q1"],"reason":"not allowed"}],"changed":true}"##,
            MemoryWorkflowAction::Dedupe,
        )
        .unwrap_err();

        assert!(err.to_string().contains("must not insert"));
    }

    #[tokio::test]
    async fn runtime_workflow_runs_memory_llm_step() {
        let dir = tempfile::TempDir::new().unwrap();
        let client = Arc::new(MockLlmClient::new(vec![
            MockResponse::tool_use(
                "memory-submit",
                "submit_step_result",
                r##"{"result":{"report_markdown":"# Memory report\n\nExtracted recent quiz weakness.","proposed_markdown":null,"facts":[{"text":"Learner should review OPC distractors.","section":"Weak topics","refs":["quiz:q1"]}],"edits":[],"changed":true}}"##,
            ),
            MockResponse::text("Memory update submitted."),
        ]));
        let input = MemoryWorkflowInput {
            target_path: "L2/quiz.md".into(),
            action: MemoryWorkflowAction::Update,
            current_markdown: "# Quiz memory\n\n".into(),
            consolidation_input_json:
                r#"{"chunk":{"citeableRefs":["quiz:q1"]},"target":{"allowedSections":["Weak topics"]}}"#
                    .into(),
        };
        let engine_config = build_workflow_engine_config(
            client.clone(),
            "mock-model",
            Arc::new(NoOpEnv) as Arc<dyn ExecutionEnv>,
            dir.path().join("memory-workflow-sessions"),
        );
        let output =
            run_memory_workflow_with_runtime(client.clone(), "mock-model", &input, engine_config)
                .await
                .unwrap();

        assert_eq!(
            client.call_count.load(std::sync::atomic::Ordering::SeqCst),
            2
        );
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
