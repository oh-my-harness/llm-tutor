use std::sync::{Arc, Mutex};

use llm_adapter::provider::Provider;
use llm_harness_runtime::observability::audit::AuditEventType;
use llm_harness_loop::CompositeBeforeToolCallHook;
use llm_harness_types::ExecutionEnv;

use crate::deep_solve_events as deep_events;
use crate::error::{Result, TutorError};
use crate::event_sink::{SharedEventSink, emit_content, emit_trace};
use crate::governance::GovernanceConfig;
use crate::llm_provider::LlmConfig;
use crate::runtime_harness::{RuntimeHarnessConfig, build_runtime_harness};
use crate::runtime_workflow::{DEEP_SOLVE_WORKFLOW_ID, validate_deep_solve_workflow};
use crate::solve_context::{Plan, SolveContext, StepResult};
use tutor_tools::WebSearchConfig;

/// Drives the four-phase Deep Solve pipeline.
pub struct SolveOrchestrator {
    context: Arc<Mutex<SolveContext>>,
    env: Arc<dyn ExecutionEnv>,
    llm: LlmConfig,
    governance: GovernanceConfig,
    event_sink: Option<SharedEventSink>,
    web_search: Option<WebSearchConfig>,
    client: Option<Arc<dyn Provider>>,
}

impl SolveOrchestrator {
    pub fn new(
        question: impl Into<String>,
        env: Arc<dyn ExecutionEnv>,
        llm: LlmConfig,
        governance: GovernanceConfig,
    ) -> Self {
        Self {
            context: Arc::new(Mutex::new(SolveContext::new(question))),
            env,
            llm,
            governance,
            event_sink: None,
            web_search: None,
            client: None,
        }
    }

    /// Inject a custom LLM client; skips `LlmConfig::build_client()` and auth.
    pub fn with_client(mut self, client: Arc<dyn Provider>) -> Self {
        self.client = Some(client);
        self
    }

    pub fn with_event_sink(mut self, sink: Option<SharedEventSink>) -> Self {
        self.event_sink = sink;
        self
    }

    pub fn with_web_search(mut self, config: Option<WebSearchConfig>) -> Self {
        self.web_search = config;
        self
    }

    fn make_client(&self) -> Arc<dyn Provider> {
        if let Some(c) = &self.client {
            return c.clone();
        }
        self.llm.build_client()
    }

    /// Run the full pipeline: [Pre-retrieve] -> Plan -> (Solve -> [REPLAN])* -> Synthesize.
    pub async fn run(&mut self, kb: Option<&str>) -> Result<String> {
        validate_deep_solve_workflow()?;
        emit_trace(
            &self.event_sink,
            "workflow_validated",
            serde_json::json!({
                "capability": "deep_solve",
                "workflow": DEEP_SOLVE_WORKFLOW_ID,
                "runtime": "llm-harness-runtime",
            }),
        )
        .await;

        if let Some(kb_text) = kb {
            self.run_pre_retrieve(kb_text).await?;
        }

        loop {
            self.run_plan().await?;
            self.run_solve_steps().await?;

            if !should_replan(&self.context.lock().unwrap()) {
                self.context.lock().unwrap().replan_reason = None;
                break;
            }
            self.context.lock().unwrap().reset_for_replan();
        }

        self.run_synthesize().await
    }

    async fn run_pre_retrieve(&mut self, kb: &str) -> Result<()> {
        deep_events::stage_start(
            &self.event_sink,
            deep_events::DeepSolveStage::Retrieve,
            "Retrieve knowledge",
        )
        .await;
        emit_trace(
            &self.event_sink,
            "phase_start",
            serde_json::json!({ "capability": "deep_solve", "phase": "pre_retrieve" }),
        )
        .await;

        // v0.1 stub for pre-retrieve phase
        self.context.lock().unwrap().kb_summary = Some(format!("[KB summary for: {kb}]"));

        crate::governance::record_audit(
            &self.governance.audit,
            AuditEventType::StateTransition,
            serde_json::json!({"phase": "pre_retrieve", "kb": kb}),
        )
        .await;

        emit_trace(
            &self.event_sink,
            "phase_end",
            serde_json::json!({ "capability": "deep_solve", "phase": "pre_retrieve" }),
        )
        .await;
        deep_events::stage_done(
            &self.event_sink,
            deep_events::DeepSolveStage::Retrieve,
            "Retrieve knowledge",
            "Knowledge summary prepared",
        )
        .await;

        Ok(())
    }

    async fn run_plan(&mut self) -> Result<()> {
        use llm_harness_agent::AgentHarnessEvent;
        use llm_harness_types::{AgentEvent, ContentBlock};

        crate::governance::record_audit(
            &self.governance.audit,
            AuditEventType::StateTransition,
            serde_json::json!({"phase": "plan"}),
        )
        .await;
        deep_events::stage_start(
            &self.event_sink,
            deep_events::DeepSolveStage::Plan,
            "Create solve plan",
        )
        .await;
        emit_trace(
            &self.event_sink,
            "phase_start",
            serde_json::json!({"capability": "deep_solve", "phase": "plan"}),
        )
        .await;

        let (question, kb_summary, replan_reason, prev_plan) = {
            let ctx = self.context.lock().unwrap();
            (
                ctx.question.clone(),
                ctx.kb_summary.clone(),
                ctx.replan_reason.clone(),
                ctx.plan.clone(),
            )
        };

        let prompt = if let Some(reason) = &replan_reason {
            let prev = prev_plan
                .as_ref()
                .map(|p| serde_json::to_string_pretty(p).unwrap_or_default())
                .unwrap_or_default();
            format!(
                "Question: {}\nKB summary: {}\n\nPrevious plan (now abandoned):\n{prev}\n\
                 Replan reason: {reason}\n\n\
                 Create a NEW step-by-step plan in JSON: \
                 {{\"analysis\":\"...\",\"steps\":[{{\"id\":\"s1\",\"goal\":\"...\"}},...]}}\n\
                 Output ONLY the JSON, no prose.",
                question,
                kb_summary.as_deref().unwrap_or("none")
            )
        } else {
            format!(
                "Question: {}\nKB summary: {}\n\n\
                 Create a step-by-step plan in JSON: \
                 {{\"analysis\":\"...\",\"steps\":[{{\"id\":\"s1\",\"goal\":\"...\"}},...]}}\n\
                 Output ONLY the JSON, no prose.",
                question,
                kb_summary.as_deref().unwrap_or("none")
            )
        };

        let client = self.make_client();

        let harness = build_runtime_harness(
            client,
            self.env.clone(),
            None,
            RuntimeHarnessConfig {
                model: self.llm.model.clone(),
                model_info: self.llm.model_info(8192),
                tools: vec![],
                system_prompt: "You are a math tutor planning a structured solution. \
                     Respond only with the requested JSON."
                    .into(),
                before_tool_call: vec![],
                prepare_next_turn: vec![],
            },
        )
        .await?;
        let mut rx = harness.subscribe();
        let prompt_task = tokio::spawn(async move { harness.prompt(prompt).await });

        let mut raw = String::new();
        while let Ok(event) = rx.recv().await {
            match event.as_ref() {
                AgentHarnessEvent::Agent(AgentEvent::MessageEnd { message, .. }) => {
                    for block in &message.content {
                        if let ContentBlock::Text { text } = block {
                            raw = text.clone();
                        }
                    }
                }
                AgentHarnessEvent::Agent(AgentEvent::TextDelta { text, .. }) => {
                    raw.push_str(text);
                }
                AgentHarnessEvent::Agent(AgentEvent::AgentEnd { new_messages }) => {
                    if raw.is_empty() {
                        raw = last_assistant_text(new_messages).unwrap_or_default();
                    }
                }
                AgentHarnessEvent::Settled | AgentHarnessEvent::Aborted => break,
                _ => {}
            }
        }
        prompt_task
            .await
            .map_err(|err| TutorError::Internal(format!("agent prompt task failed: {err}")))??;

        if raw.is_empty() {
            return Err(TutorError::Internal("no plan output".into()));
        }

        // Extract JSON even if the LLM added surrounding prose
        let json_start = raw.find('{').unwrap_or(0);
        let json_end = raw.rfind('}').map(|i| i + 1).unwrap_or(raw.len());
        let json_str = &raw[json_start..json_end];

        let plan: Plan = serde_json::from_str(json_str)
            .map_err(|e| TutorError::Internal(format!("plan parse error: {e}\nraw: {raw}")))?;

        deep_events::plan(&self.event_sink, &plan).await;
        self.context.lock().unwrap().plan = Some(plan);
        let step_count = self
            .context
            .lock()
            .unwrap()
            .plan
            .as_ref()
            .map_or(0, |p| p.steps.len());
        emit_trace(
            &self.event_sink,
            "phase_end",
            serde_json::json!({
                "capability": "deep_solve",
                "phase": "plan",
                "step_count": step_count,
            }),
        )
        .await;
        deep_events::stage_done(
            &self.event_sink,
            deep_events::DeepSolveStage::Plan,
            "Create solve plan",
            format!("Generated {step_count} solve steps"),
        )
        .await;
        Ok(())
    }

    async fn run_solve_steps(&mut self) -> Result<()> {
        use llm_harness_agent::AgentHarnessEvent;
        use llm_harness_types::{
            AgentEvent, BeforeToolCallHook, ContentBlock, PrepareNextTurnHook,
        };
        use std::sync::Arc;
        use tutor_tools::{
            CodeExecTool, RagSearchTool, ReadMemoryTool, WebFetchTool, WebSearchTool,
            WriteMemoryTool,
        };

        use crate::phase_manager::PhaseManager;
        use crate::replan_hook::ReplanHook;
        use crate::replan_tool::ReplanTool;

        let steps = self
            .context
            .lock()
            .unwrap()
            .plan
            .as_ref()
            .ok_or_else(|| TutorError::Internal("no plan".into()))?
            .steps
            .clone();

        crate::governance::record_audit(
            &self.governance.audit,
            AuditEventType::StateTransition,
            serde_json::json!({"phase": "solve_steps", "step_count": steps.len()}),
        )
        .await;
        deep_events::stage_start(
            &self.event_sink,
            deep_events::DeepSolveStage::Solve,
            "Solve step by step",
        )
        .await;
        emit_trace(
            &self.event_sink,
            "phase_start",
            serde_json::json!({
                "capability": "deep_solve",
                "phase": "solve_steps",
                "step_count": steps.len(),
            }),
        )
        .await;

        // ReplanHook shares the orchestrator's context directly via Arc.
        let replan_hook = Arc::new(ReplanHook::new(self.context.clone()));

        // Compose hooks: approval wrapper (if configured) then replan hook
        let before_tool_call: Vec<Arc<dyn BeforeToolCallHook>> = {
            if let Some(approval) = &self.governance.approval {
                vec![Arc::new(CompositeBeforeToolCallHook::new(vec![
                    approval.clone() as Arc<dyn BeforeToolCallHook>,
                    replan_hook.clone() as Arc<dyn BeforeToolCallHook>,
                ]))]
            } else {
                vec![replan_hook.clone() as Arc<dyn BeforeToolCallHook>]
            }
        };

        for step in &steps {
            deep_events::step_start(&self.event_sink, &step.id, &step.goal).await;
            emit_trace(
                &self.event_sink,
                "phase_start",
                serde_json::json!({
                    "capability": "deep_solve",
                    "phase": "solve_step",
                    "step_id": step.id,
                    "goal": step.goal,
                }),
            )
            .await;

            let solve_tools: Vec<Arc<dyn llm_harness_types::Tool>> = vec![
                Arc::new(ReadMemoryTool::new()),
                Arc::new(WriteMemoryTool::new()),
                Arc::new(RagSearchTool::new()),
                Arc::new(match self.web_search.clone() {
                    Some(config) => WebSearchTool::with_config(config),
                    None => WebSearchTool::new(),
                }),
                Arc::new(match self.web_search.clone() {
                    Some(config) => WebFetchTool::with_config(config),
                    None => WebFetchTool::new(),
                }),
                Arc::new(CodeExecTool::new()),
                Arc::new(ReplanTool),
            ];

            let phase_mgr = Arc::new(PhaseManager::new(vec![
                "rag_search".into(),
                "read_memory".into(),
                "write_memory".into(),
                "web_search".into(),
                "web_fetch".into(),
                "code_exec".into(),
                "replan".into(),
            ]));

            let client = self.make_client();
            let harness = build_runtime_harness(
                client,
                self.env.clone(),
                None,
                RuntimeHarnessConfig {
                    model: self.llm.model.clone(),
                    model_info: self.llm.model_info(8192),
                    tools: solve_tools,
                    system_prompt: format!(
                        "You are solving step {id}: {goal}\n\
                         Use read_memory when the step should adapt to the learner's prior weaknesses, \
                         preferences, recent learning state, or teaching strategy. Memory is learner context, \
                         not a factual source. Use write_memory only when the user explicitly asks you to remember \
                         a durable preference or approves recording it. Use rag_search for course knowledge, web_search for external discovery, \
                         web_fetch to read important source pages, and code_exec to run code.\n\
                         For non-trivial numeric calculations, approximations, transcendental functions, \
                         statistics, or simulations, use code_exec with Python to compute or verify the result.\n\
                         If the current plan is fundamentally wrong, call replan(reason) - \
                         this aborts the step and triggers a new plan.\n\
                         When done, write FINISH: followed by your conclusion for this step.",
                        id = step.id,
                        goal = step.goal,
                    ),
                    before_tool_call: before_tool_call.clone(),
                    prepare_next_turn: vec![phase_mgr as Arc<dyn PrepareNextTurnHook>],
                },
            )
            .await?;
            let mut rx = harness.subscribe();
            let step_prompt = format!("Solve step {}: {}", step.id, step.goal);
            let prompt_task = tokio::spawn(async move { harness.prompt(step_prompt).await });

            let mut raw = String::new();
            while let Ok(event) = rx.recv().await {
                match event.as_ref() {
                    AgentHarnessEvent::Agent(AgentEvent::MessageEnd { message, .. }) => {
                        for block in &message.content {
                            if let ContentBlock::Text { text } = block {
                                raw = text.clone();
                            }
                        }
                    }
                    AgentHarnessEvent::Agent(AgentEvent::TextDelta { text, .. }) => {
                        raw.push_str(text);
                    }
                    AgentHarnessEvent::Agent(AgentEvent::AgentEnd { new_messages }) => {
                        if raw.is_empty() {
                            raw = last_assistant_text(new_messages).unwrap_or_default();
                        }
                    }
                    AgentHarnessEvent::Agent(AgentEvent::ToolExecutionStart {
                        tool_use_id,
                        tool_name,
                        args,
                    }) => {
                        emit_trace(
                            &self.event_sink,
                            "tool_call",
                            serde_json::json!({
                                "capability": "deep_solve",
                                "stage": "solve",
                                "phase": "solve_step",
                                "step_id": step.id,
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
                            &self.event_sink,
                            "tool_result",
                            serde_json::json!({
                                "capability": "deep_solve",
                                "stage": "solve",
                                "phase": "solve_step",
                                "step_id": step.id,
                                "tool_use_id": tool_use_id,
                                "ok": result.is_ok(),
                            }),
                        )
                        .await;
                    }
                    AgentHarnessEvent::Settled | AgentHarnessEvent::Aborted => break,
                    _ => {}
                }
            }
            prompt_task.await.map_err(|err| {
                TutorError::Internal(format!("agent prompt task failed: {err}"))
            })??;

            // Check if replan was triggered
            let step_reason = self.context.lock().unwrap().replan_reason.clone();
            if step_reason.is_some() {
                crate::governance::record_audit(
                    &self.governance.audit,
                    AuditEventType::StateTransition,
                    serde_json::json!({"event": "replan", "step": step.id}),
                )
                .await;
                emit_trace(
                    &self.event_sink,
                    "replan",
                    serde_json::json!({
                        "capability": "deep_solve",
                        "stage": "solve",
                        "step_id": step.id,
                        "reason": step_reason,
                    }),
                )
                .await;
                return Ok(());
            }

            let finish_text = raw
                .lines()
                .skip_while(|l| !l.starts_with("FINISH:"))
                .collect::<Vec<_>>()
                .join("\n");

            let final_step_text = if finish_text.is_empty() {
                raw
            } else {
                finish_text
            };
            self.context.lock().unwrap().step_results.push(StepResult {
                step_id: step.id.clone(),
                finish_text: final_step_text.clone(),
            });
            deep_events::step_done(&self.event_sink, &step.id, &step.goal, final_step_text).await;
            emit_trace(
                &self.event_sink,
                "phase_end",
                serde_json::json!({
                    "capability": "deep_solve",
                    "phase": "solve_step",
                    "step_id": step.id,
                }),
            )
            .await;
        }
        emit_trace(
            &self.event_sink,
            "phase_end",
            serde_json::json!({
                "capability": "deep_solve",
                "phase": "solve_steps",
            }),
        )
        .await;
        deep_events::stage_done(
            &self.event_sink,
            deep_events::DeepSolveStage::Solve,
            "Solve step by step",
            format!("Completed {} steps", steps.len()),
        )
        .await;
        Ok(())
    }

    async fn run_synthesize(&mut self) -> Result<String> {
        use llm_harness_agent::AgentHarnessEvent;
        use llm_harness_types::{AgentEvent, ContentBlock};

        crate::governance::record_audit(
            &self.governance.audit,
            AuditEventType::StateTransition,
            serde_json::json!({"phase": "synthesize"}),
        )
        .await;
        deep_events::stage_start(
            &self.event_sink,
            deep_events::DeepSolveStage::Synthesize,
            "Synthesize final answer",
        )
        .await;
        emit_trace(
            &self.event_sink,
            "phase_start",
            serde_json::json!({"capability": "deep_solve", "phase": "synthesize"}),
        )
        .await;

        let (question, steps_summary) = {
            let ctx = self.context.lock().unwrap();
            (ctx.question.clone(), format_step_results(&ctx.step_results))
        };
        let prompt = format!(
            "Question: {}\n\nStep-by-step work:\n{steps_summary}\n\n\
             Synthesize a clear, complete final answer for the student. \
             Start with the direct answer, then provide explanation.",
            question
        );

        let client = self.make_client();

        let harness = build_runtime_harness(
            client,
            self.env.clone(),
            None,
            RuntimeHarnessConfig {
                model: self.llm.model.clone(),
                model_info: self.llm.model_info(8192),
                tools: vec![],
                system_prompt: "You are a math tutor writing a final answer synthesis. \
                     Be clear, structured, and educational."
                    .into(),
                before_tool_call: vec![],
                prepare_next_turn: vec![],
            },
        )
        .await?;
        let mut rx = harness.subscribe();
        let prompt_task = tokio::spawn(async move { harness.prompt(prompt).await });

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
                AgentHarnessEvent::Agent(AgentEvent::TextDelta { text, .. }) => {
                    last_text.push_str(text);
                    emit_content(&self.event_sink, text.clone(), true).await;
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
            &self.event_sink,
            "phase_end",
            serde_json::json!({"capability": "deep_solve", "phase": "synthesize"}),
        )
        .await;

        let answer = if last_text.is_empty() {
            "No synthesis generated.".into()
        } else {
            last_text
        };
        deep_events::final_answer(&self.event_sink, &answer).await;
        deep_events::stage_done(
            &self.event_sink,
            deep_events::DeepSolveStage::Synthesize,
            "Synthesize final answer",
            "Final explanation generated",
        )
        .await;

        Ok(answer)
    }
}

/// True if a replan should be triggered: reason is set AND under the limit.
pub fn should_replan(ctx: &SolveContext) -> bool {
    ctx.replan_reason.is_some() && ctx.replan_count < ctx.max_replans
}

fn format_step_results(results: &[StepResult]) -> String {
    results
        .iter()
        .map(|r| format!("Step {}: {}", r.step_id, r.finish_text))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn last_assistant_text(messages: &[llm_harness_types::AgentMessage]) -> Option<String> {
    messages.iter().rev().find_map(|message| {
        let llm_harness_types::AgentMessage::Assistant(message) = message else {
            return None;
        };

        message.content.iter().rev().find_map(|block| {
            if let llm_harness_types::ContentBlock::Text { text } = block {
                Some(text.clone())
            } else {
                None
            }
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solve_context::SolveContext;

    #[test]
    fn should_replan_when_reason_set_and_under_limit() {
        let mut ctx = SolveContext::new("q");
        ctx.replan_reason = Some("try another way".into());
        ctx.replan_count = 0;
        assert!(should_replan(&ctx));
    }

    #[test]
    fn should_not_replan_when_limit_reached() {
        let mut ctx = SolveContext::new("q");
        ctx.replan_reason = Some("try again".into());
        ctx.replan_count = 2;
        assert!(!should_replan(&ctx));
    }

    #[test]
    fn should_not_replan_when_no_reason() {
        let ctx = SolveContext::new("q");
        assert!(!should_replan(&ctx));
    }

    #[test]
    fn step_results_format_for_synthesis() {
        let results = vec![
            StepResult {
                step_id: "s1".into(),
                finish_text: "The integral is 8/3.".into(),
            },
            StepResult {
                step_id: "s2".into(),
                finish_text: "Simplified: 2.67.".into(),
            },
        ];
        let formatted = format_step_results(&results);
        assert!(formatted.contains("s1"));
        assert!(formatted.contains("8/3"));
    }

    #[test]
    fn parse_plan_from_json() {
        let raw = r#"{"analysis":"use calculus","steps":[{"id":"s1","goal":"integrate"},{"id":"s2","goal":"simplify"}]}"#;
        let plan: Plan = serde_json::from_str(raw).unwrap();
        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].id, "s1");
    }
}
