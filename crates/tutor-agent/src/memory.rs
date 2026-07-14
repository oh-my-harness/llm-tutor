use std::sync::Arc;

use futures::future::BoxFuture;
use llm_harness_runtime::control::cost::CostAggregate;
use llm_harness_runtime::workflow::engine::{WorkflowEngine, WorkflowEngineConfig};
use llm_harness_runtime::workflow::executor::{ExecutorCtx, StepExecutor};
use llm_harness_runtime::workflow::model::StepResult;
use llm_harness_types::Tool;
use serde::{Deserialize, Serialize};

use crate::error::{Result, TutorError};
use crate::runtime_engine::RuntimeDeclarativeJudge;
use crate::runtime_workflow::{memory_workflow, validate_memory_workflow};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryWorkflowAction {
    Update,
    Check,
    Dedupe,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum MemoryOutputLanguage {
    #[serde(rename = "zh-CN")]
    ZhCn,
    #[default]
    #[serde(rename = "en-US")]
    EnUs,
}

#[derive(Debug, Clone)]
pub struct MemoryWorkflowInput {
    pub target_path: String,
    pub action: MemoryWorkflowAction,
    pub output_language: MemoryOutputLanguage,
    pub current_markdown: String,
    pub consolidation_input_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryWorkflowOutput {
    pub summary: String,
    #[serde(default)]
    pub findings: Vec<MemoryWorkflowFinding>,
    #[serde(default)]
    pub changes: Vec<MemoryWorkflowChange>,
}

#[derive(Debug, Clone)]
pub struct MemoryWorkflowRun {
    pub output: MemoryWorkflowOutput,
    pub cost: CostAggregate,
}

#[derive(Debug, Deserialize)]
struct MemoryWorkflowJson {
    summary: String,
    #[serde(default)]
    findings: Vec<MemoryWorkflowFinding>,
    #[serde(default)]
    changes: Vec<MemoryWorkflowChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryWorkflowFinding {
    pub id: String,
    pub entry_id: Option<String>,
    pub severity: String,
    pub kind: String,
    pub message: String,
    #[serde(default)]
    pub refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryWorkflowChange {
    pub id: String,
    pub op: MemoryWorkflowChangeOp,
    pub section: Option<String>,
    pub entry_id: Option<String>,
    pub after_entry_id: Option<String>,
    pub text: Option<String>,
    #[serde(default)]
    pub refs: Vec<String>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryWorkflowChangeOp {
    Replace,
    Delete,
    Insert,
}

pub async fn run_memory_workflow_with_runtime(
    input: &MemoryWorkflowInput,
    engine_config: WorkflowEngineConfig,
) -> Result<MemoryWorkflowRun> {
    run_memory_workflow_with_tools(input, engine_config, Vec::new()).await
}

pub async fn run_memory_workflow_with_tools(
    input: &MemoryWorkflowInput,
    engine_config: WorkflowEngineConfig,
    tools: Vec<Arc<dyn Tool>>,
) -> Result<MemoryWorkflowRun> {
    validate_memory_workflow()?;
    let workflow = memory_workflow();
    let mut engine = WorkflowEngine::new(
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
    for tool in tools {
        engine = engine.with_tool(tool);
    }

    let result = engine
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
    let output = validate_memory_workflow_output(output, input.action)?;
    Ok(MemoryWorkflowRun {
        output,
        cost: result.cost,
    })
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
    validate_memory_workflow_output(
        MemoryWorkflowOutput {
            summary: parsed.summary,
            findings: parsed.findings,
            changes: parsed.changes,
        },
        action,
    )
}

fn validate_memory_workflow_output(
    output: MemoryWorkflowOutput,
    action: MemoryWorkflowAction,
) -> Result<MemoryWorkflowOutput> {
    let summary = output.summary.trim().to_string();
    if summary.is_empty() {
        return Err(TutorError::Internal(
            "memory workflow summary is empty".into(),
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for finding in &output.findings {
        if finding.id.trim().is_empty()
            || finding.message.trim().is_empty()
            || !ids.insert(finding.id.trim())
        {
            return Err(TutorError::Internal(
                "memory workflow returned an invalid finding".into(),
            ));
        }
    }
    for change in &output.changes {
        if change.id.trim().is_empty()
            || change.reason.trim().is_empty()
            || !ids.insert(change.id.trim())
        {
            return Err(TutorError::Internal(
                "memory workflow returned an invalid change".into(),
            ));
        }
        validate_workflow_change(change)?;
        if action == MemoryWorkflowAction::Dedupe && change.op == MemoryWorkflowChangeOp::Insert {
            return Err(TutorError::Internal(
                "dedupe workflow must not insert new facts".into(),
            ));
        }
    }
    Ok(MemoryWorkflowOutput {
        summary,
        findings: output.findings,
        changes: output.changes,
    })
}

fn validate_workflow_change(change: &MemoryWorkflowChange) -> Result<()> {
    if change.refs.is_empty() {
        return Err(TutorError::Internal(
            "memory change requires evidence refs".into(),
        ));
    }
    match change.op {
        MemoryWorkflowChangeOp::Insert => {
            if change
                .section
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
                || change.text.as_deref().unwrap_or_default().trim().is_empty()
            {
                return Err(TutorError::Internal(
                    "memory insert change is incomplete".into(),
                ));
            }
        }
        MemoryWorkflowChangeOp::Replace => {
            if change
                .entry_id
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
                || change.text.as_deref().unwrap_or_default().trim().is_empty()
            {
                return Err(TutorError::Internal(
                    "memory replace change is incomplete".into(),
                ));
            }
        }
        MemoryWorkflowChangeOp::Delete => {
            if change
                .entry_id
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
                || change.text.is_some()
            {
                return Err(TutorError::Internal(
                    "memory delete change is invalid".into(),
                ));
            }
        }
    }
    Ok(())
}

fn memory_prompt(input: &MemoryWorkflowInput) -> String {
    let is_l3 = input.target_path.starts_with("L3/");
    let is_recent = input.target_path == "L3/recent.md";
    let entry_text_limit = if is_l3 { 1_200 } else { 500 };
    let evidence_rules = if is_recent {
        "- Use list_memory_entries or search_memory_entries to discover L2 candidates within the allowed source matrix.\n- Use read_memory_entry before citing an L2 entry.\n- Prefer L2 evidence; use the L1 event tools only for bounded recent chronology or to verify an L2 source.\n- Cite canonical memory:L2/path.md#m_id refs for L2 evidence and canonical event refs only for the recent chronology exception."
    } else if is_l3 {
        "- Use list_memory_entries or search_memory_entries to discover candidates within target.instructions.allowedL2Paths.\n- Use read_memory_entry before citing an L2 entry.\n- Use read_memory_entry_sources only when an L2 summary needs verification or more detail.\n- Every finding and change must cite canonical memory:L2/path.md#m_id refs returned by a read tool.\n- Do not cite or scan L1 events directly."
    } else {
        "- Use list_memory_events or search_memory_events to discover candidates, starting with the target surface.\n- Use read_memory_event, read_memory_context, or read_memory_source before citing evidence.\n- Every finding and change must cite canonical event-level refs returned by a read tool."
    };
    let action_rules = match input.action {
        MemoryWorkflowAction::Update => {
            "- Return insert or evidence-backed replace changes.\n- Prefer durable learning-relevant observations over one-off chatter.\n- Use only sections listed in target.allowedSections."
        }
        MemoryWorkflowAction::Check => {
            "- Resolve existing evidence with the appropriate read tool before judging supported claims.\n- Return compact findings for contradictions, stale facts, missing evidence, duplicates, unclear wording, and risky overgeneralizations.\n- If useful, include stable-entry-id changes.\n- Every change must include a short reason."
        }
        MemoryWorkflowAction::Dedupe => {
            "- Return replace/delete changes against stable entry ids from the Markdown markers.\n- Merge duplicate or overlapping bullets.\n- Do not insert new facts or delete unique useful memory.\n- Include a short reason for each change."
        }
    };
    let source = if input.consolidation_input_json.trim().is_empty() {
        "(none)".to_string()
    } else {
        input.consolidation_input_json.clone()
    };
    let (language_rules, output_example) = match input.output_language {
        MemoryOutputLanguage::ZhCn => (
            "- Write summary, finding.message, change.text, and change.reason in Simplified Chinese.\n- Preserve code, API names, model names, paper titles, and other proper nouns when translation would reduce precision.\n- Do not translate or rewrite existing memory merely to change its language.\n- Keep schema values and section keys exactly as specified, even when they are English.",
            if is_l3 {
                r#"{"summary":"发现一项可审核的更新","findings":[{"id":"finding_1","entry_id":"m_existing","severity":"warning","kind":"unsupported","message":"这条记忆缺少充分证据","refs":["memory:L2/chat.md#m_example"]}],"changes":[{"id":"change_1","op":"insert","section":"one allowed section key","entry_id":null,"after_entry_id":null,"text":"一条简洁的学习者记忆","refs":["memory:L2/chat.md#m_example"],"reason":"这项修改有助于后续个性化教学"}]}"#
            } else {
                r#"{"summary":"发现一项可审核的更新","findings":[{"id":"finding_1","entry_id":"m_existing","severity":"warning","kind":"unsupported","message":"这条记忆缺少充分证据","refs":["chat:event-id"]}],"changes":[{"id":"change_1","op":"insert","section":"one allowed section key","entry_id":null,"after_entry_id":null,"text":"一条简洁的学习者记忆","refs":["chat:event-id"],"reason":"这项修改有助于后续个性化教学"}]}"#
            },
        ),
        MemoryOutputLanguage::EnUs => (
            "- Write summary, finding.message, change.text, and change.reason in English.\n- Preserve code, API names, model names, paper titles, and other proper nouns when translation would reduce precision.\n- Do not translate or rewrite existing memory merely to change its language.\n- Keep schema values and section keys exactly as specified.",
            if is_l3 {
                r#"{"summary":"One reviewable update found","findings":[{"id":"finding_1","entry_id":"m_existing","severity":"warning","kind":"unsupported","message":"This memory lacks sufficient evidence","refs":["memory:L2/chat.md#m_example"]}],"changes":[{"id":"change_1","op":"insert","section":"one allowed section key","entry_id":null,"after_entry_id":null,"text":"A concise learner memory","refs":["memory:L2/chat.md#m_example"],"reason":"This change supports future personalization"}]}"#
            } else {
                r#"{"summary":"One reviewable update found","findings":[{"id":"finding_1","entry_id":"m_existing","severity":"warning","kind":"unsupported","message":"This memory lacks sufficient evidence","refs":["chat:event-id"]}],"changes":[{"id":"change_1","op":"insert","section":"one allowed section key","entry_id":null,"after_entry_id":null,"text":"A concise learner memory","refs":["chat:event-id"],"reason":"This change supports future personalization"}]}"#
            },
        ),
    };
    format!(
        "Target memory file: {target_path}\nAction: {action:?}\nOutput language: {output_language:?}\n\nLanguage contract:\n{language_rules}\n\nEvidence contract:\n{evidence_rules}\n\nAction rules:\n{action_rules}\n- Keep each change.text at or below {entry_text_limit} characters. Each change must contain one coherent memory entry; split unrelated or longer synthesis into multiple evidence-bound changes.\n\nCurrent Markdown with line numbers and stable <!--m_...--> entry ids:\n```text\n{numbered_current}\n```\n\nTarget schema and evidence catalog:\n```json\n{source}\n```\n\nA list/search result is only a candidate and is never sufficient evidence by itself.\n\nReturn JSON exactly like:\n{output_example}\n\nReturn findings and changes as arrays, even when empty. Never return complete Markdown or line-number edits.",
        target_path = input.target_path,
        action = input.action,
        output_language = input.output_language,
        language_rules = language_rules,
        evidence_rules = evidence_rules,
        entry_text_limit = entry_text_limit,
        output_example = output_example,
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
    fn parses_findings_and_stable_changes() {
        let output = parse_memory_workflow_output(
            r##"{"summary":"One unsupported claim.","findings":[{"id":"finding_1","entry_id":"m_1","severity":"warning","kind":"unsupported","message":"Evidence is missing.","refs":[]}],"changes":[{"id":"change_1","op":"delete","section":null,"entry_id":"m_1","after_entry_id":null,"text":null,"refs":["chat:event-1"],"reason":"Unsupported claim"}]}"##,
            MemoryWorkflowAction::Check,
        )
        .unwrap();
        assert_eq!(output.findings.len(), 1);
        assert_eq!(output.changes[0].entry_id.as_deref(), Some("m_1"));
    }

    #[test]
    fn rejects_insert_without_evidence_refs() {
        let err = parse_memory_workflow_output(
            r##"{"summary":"Bad insert.","findings":[],"changes":[{"id":"change_1","op":"insert","section":"Weak topics","entry_id":null,"after_entry_id":null,"text":"Fact","refs":[],"reason":"Missing evidence"}]}"##,
            MemoryWorkflowAction::Update,
        )
        .unwrap_err();
        assert!(err.to_string().contains("requires evidence refs"));
    }

    #[test]
    fn rejects_delete_without_evidence_refs() {
        let err = parse_memory_workflow_output(
            r##"{"summary":"Unsupported claim.","findings":[],"changes":[{"id":"change_1","op":"delete","section":null,"entry_id":"m_1","after_entry_id":null,"text":null,"refs":[],"reason":"Unsupported"}]}"##,
            MemoryWorkflowAction::Check,
        )
        .unwrap_err();
        assert!(err.to_string().contains("requires evidence refs"));
    }

    #[test]
    fn rejects_non_json_prose_around_workflow_output() {
        let err = parse_memory_workflow_output(
            r##"Here is the JSON: {"summary":"No changes.","findings":[],"changes":[]}"##,
            MemoryWorkflowAction::Check,
        )
        .unwrap_err();
        assert!(err.to_string().contains("did not contain JSON"));
    }

    #[test]
    fn rejects_malformed_workflow_json() {
        let err = parse_memory_workflow_output(
            r##"{"summary":"No changes.","findings":[],"changes":[]"##,
            MemoryWorkflowAction::Check,
        )
        .unwrap_err();
        assert!(err.to_string().contains("did not contain JSON"));
    }

    #[test]
    fn rejects_duplicate_change_ids() {
        let err = parse_memory_workflow_output(
            r##"{"summary":"Duplicates.","findings":[],"changes":[{"id":"same","op":"delete","section":null,"entry_id":"m_1","after_entry_id":null,"text":null,"refs":["chat:event-1"],"reason":"duplicate"},{"id":"same","op":"delete","section":null,"entry_id":"m_2","after_entry_id":null,"text":null,"refs":["chat:event-1"],"reason":"duplicate"}]}"##,
            MemoryWorkflowAction::Check,
        )
        .unwrap_err();
        assert!(err.to_string().contains("invalid change"));
    }

    #[test]
    fn leaves_layer_specific_change_length_validation_to_the_product_boundary() {
        let text = "x".repeat(1_201);
        let payload = serde_json::json!({
            "summary": "Too long.",
            "findings": [],
            "changes": [{
                "id": "change_1", "op": "insert", "section": "Weak topics",
                "entry_id": null, "after_entry_id": null, "text": text,
                "refs": ["quiz:event-1"], "reason": "durable evidence"
            }]
        });
        let output =
            parse_memory_workflow_output(&payload.to_string(), MemoryWorkflowAction::Update)
                .unwrap();
        assert_eq!(output.changes[0].text.as_deref().unwrap().len(), 1_201);
    }

    #[test]
    fn rejects_dedupe_insert() {
        let err = parse_memory_workflow_output(
            r##"{"summary":"Bad dedupe.","findings":[],"changes":[{"id":"change_1","op":"insert","section":"Weak topics","entry_id":null,"after_entry_id":null,"text":"New fact","refs":["quiz:event-1"],"reason":"not allowed"}]}"##,
            MemoryWorkflowAction::Dedupe,
        )
        .unwrap_err();

        assert!(err.to_string().contains("must not insert"));
    }

    #[tokio::test]
    async fn runtime_workflow_runs_memory_llm_step() {
        let dir = tempfile::TempDir::new().unwrap();
        let client = Arc::new(MockLlmClient::new(vec![MockResponse::tool_use(
            "memory-submit",
            "submit_step_result",
            r##"{"result":{"summary":"One update ready.","findings":[],"changes":[{"id":"change_1","op":"insert","section":"Weak topics","entry_id":null,"after_entry_id":null,"text":"Learner should review OPC distractors.","refs":["quiz:event-1"],"reason":"Repeated quiz error"}]}}"##,
        )]));
        let input = MemoryWorkflowInput {
            target_path: "L2/quiz.md".into(),
            action: MemoryWorkflowAction::Update,
            output_language: MemoryOutputLanguage::EnUs,
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
        let run = run_memory_workflow_with_runtime(&input, engine_config)
            .await
            .unwrap();

        assert_eq!(
            client.call_count.load(std::sync::atomic::Ordering::SeqCst),
            1
        );
        assert_eq!(run.cost.total_input_tokens, 0);
        let output = run.output;
        assert_eq!(output.changes[0].refs, vec!["quiz:event-1"]);
    }

    #[test]
    fn memory_prompt_pins_generated_fields_to_simplified_chinese() {
        let prompt = memory_prompt(&MemoryWorkflowInput {
            target_path: "L2/chat.md".into(),
            action: MemoryWorkflowAction::Update,
            output_language: MemoryOutputLanguage::ZhCn,
            current_markdown: "# Chat memory\n\n- Existing English fact. <!--m_1-->".into(),
            consolidation_input_json: r#"{"target":{"allowedSections":["Topics"]}}"#.into(),
        });

        assert!(prompt.contains("Output language: ZhCn"));
        assert!(prompt.contains("change.text, and change.reason in Simplified Chinese"));
        assert!(prompt.contains("一条简洁的学习者记忆"));
        assert!(prompt.contains("Do not translate or rewrite existing memory"));
        assert!(prompt.contains("Keep schema values and section keys exactly as specified"));
    }

    #[test]
    fn l3_memory_prompt_requires_read_l2_entry_evidence() {
        let prompt = memory_prompt(&MemoryWorkflowInput {
            target_path: "L3/profile.md".into(),
            action: MemoryWorkflowAction::Update,
            output_language: MemoryOutputLanguage::EnUs,
            current_markdown: "# Student profile".into(),
            consolidation_input_json: r#"{"instructions":{"evidenceLayer":"L2"}}"#.into(),
        });

        assert!(prompt.contains("Use read_memory_entry before citing an L2 entry"));
        assert!(prompt.contains("memory:L2/chat.md#m_example"));
        assert!(prompt.contains("Do not cite or scan L1 events directly"));
        assert!(prompt.contains("at or below 1200 characters"));
    }

    #[test]
    fn recent_memory_prompt_documents_the_bounded_l1_exception() {
        let prompt = memory_prompt(&MemoryWorkflowInput {
            target_path: "L3/recent.md".into(),
            action: MemoryWorkflowAction::Update,
            output_language: MemoryOutputLanguage::EnUs,
            current_markdown: "# Recent learning context".into(),
            consolidation_input_json: "{}".into(),
        });

        assert!(prompt.contains("bounded recent chronology"));
        assert!(prompt.contains("Prefer L2 evidence"));
    }
}
