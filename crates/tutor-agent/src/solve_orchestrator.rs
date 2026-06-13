use std::sync::{Arc, Mutex};

use llm_adapter::provider::Provider;
use llm_harness_runtime::audit::AuditEventType;
use llm_harness_runtime::composite::CompositeBeforeToolCallHook;
use llm_harness_types::ExecutionEnv;

use crate::error::{Result, TutorError};
use crate::governance::GovernanceConfig;
use crate::llm_provider::LlmConfig;
use crate::solve_context::{Plan, SolveContext, StepResult};

/// Drives the four-phase Deep Solve pipeline.
pub struct SolveOrchestrator {
    context: Arc<Mutex<SolveContext>>,
    env: Arc<dyn ExecutionEnv>,
    llm: LlmConfig,
    governance: GovernanceConfig,
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
            client: None,
        }
    }

    /// Inject a custom LLM client; skips `LlmConfig::build_client()` and auth.
    pub fn with_client(mut self, client: Arc<dyn Provider>) -> Self {
        self.client = Some(client);
        self
    }

    fn make_client(&self) -> Arc<dyn Provider> {
        if let Some(c) = &self.client {
            return c.clone();
        }
        self.llm.build_client()
    }

    fn auth_hook(&self) -> Option<Arc<dyn llm_harness_types::AuthHook>> {
        if self.client.is_some() {
            return None;
        }
        use llm_harness_types::AuthHook;
        self.llm
            .auth_hook()
            .map(|h| Arc::new(h) as Arc<dyn AuthHook>)
    }

    /// Run the full pipeline: [Pre-retrieve] → Plan → (Solve → [REPLAN])* → Synthesize.
    pub async fn run(&mut self, kb: Option<&str>) -> Result<String> {
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
        // v0.1 stub for pre-retrieve phase
        self.context.lock().unwrap().kb_summary = Some(format!("[KB summary for: {kb}]"));

        crate::governance::record_audit(
            &self.governance.audit,
            AuditEventType::StateTransition,
            serde_json::json!({"phase": "pre_retrieve", "kb": kb}),
        )
        .await;

        Ok(())
    }

    async fn run_plan(&mut self) -> Result<()> {
        use llm_harness::{AgentHarness, AgentHarnessEvent, AgentHarnessOptions, HarnessHooks};
        use llm_harness_types::{AgentEvent, ContentBlock};

        crate::governance::record_audit(
            &self.governance.audit,
            AuditEventType::StateTransition,
            serde_json::json!({"phase": "plan"}),
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

        let opts = AgentHarnessOptions {
            model: self.llm.model.clone(),
            tools: vec![],
            system_prompt: Some(
                "You are a math tutor planning a structured solution. \
                 Respond only with the requested JSON."
                    .into(),
            ),
            auth: self.auth_hook(),
            hooks: HarnessHooks {
                after_provider_response: Some(self.governance.budget.clone()),
                ..HarnessHooks::none()
            },
            ..AgentHarnessOptions::new(self.llm.model.clone())
        };

        let harness = AgentHarness::new_in_memory(client, self.env.clone(), opts).await;
        let mut rx = harness.subscribe();

        harness.prompt(prompt).await?;

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
                AgentHarnessEvent::Settled | AgentHarnessEvent::Aborted => break,
                _ => {}
            }
        }

        if raw.is_empty() {
            return Err(TutorError::Internal("no plan output".into()));
        }

        // Extract JSON even if the LLM added surrounding prose
        let json_start = raw.find('{').unwrap_or(0);
        let json_end = raw.rfind('}').map(|i| i + 1).unwrap_or(raw.len());
        let json_str = &raw[json_start..json_end];

        let plan: Plan = serde_json::from_str(json_str)
            .map_err(|e| TutorError::Internal(format!("plan parse error: {e}\nraw: {raw}")))?;

        self.context.lock().unwrap().plan = Some(plan);
        Ok(())
    }

    async fn run_solve_steps(&mut self) -> Result<()> {
        use llm_harness::{AgentHarness, AgentHarnessEvent, AgentHarnessOptions, HarnessHooks};
        use llm_harness_types::{AgentEvent, BeforeToolCallHook, ContentBlock};
        use std::sync::Arc;
        use tutor_tools::{CodeExecTool, RagSearchTool, WebSearchTool};

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

        // ReplanHook shares the orchestrator's context directly via Arc.
        let replan_hook = Arc::new(ReplanHook::new(self.context.clone()));

        // Compose hooks: approval wrapper (if configured) then replan hook
        let before_tool_call: Option<Arc<dyn BeforeToolCallHook>> = {
            if let Some(approval) = &self.governance.approval {
                Some(Arc::new(CompositeBeforeToolCallHook::new(vec![
                    approval.clone() as Arc<dyn BeforeToolCallHook>,
                    replan_hook.clone() as Arc<dyn BeforeToolCallHook>,
                ])))
            } else {
                Some(replan_hook.clone() as Arc<dyn BeforeToolCallHook>)
            }
        };

        for step in &steps {
            let solve_tools: Vec<Arc<dyn llm_harness_types::Tool>> = vec![
                Arc::new(RagSearchTool::new()),
                Arc::new(WebSearchTool::new()),
                Arc::new(CodeExecTool::new()),
                Arc::new(ReplanTool),
            ];

            let phase_mgr = Arc::new(PhaseManager::new(vec![
                "rag_search".into(),
                "web_search".into(),
                "code_exec".into(),
                "replan".into(),
            ]));

            let opts = AgentHarnessOptions {
                model: self.llm.model.clone(),
                tools: solve_tools,
                system_prompt: Some(format!(
                    "You are solving step {id}: {goal}\n\
                     Use rag_search and web_search for information, code_exec to run code.\n\
                     If the current plan is fundamentally wrong, call replan(reason) — \
                     this aborts the step and triggers a new plan.\n\
                     When done, write FINISH: followed by your conclusion for this step.",
                    id = step.id,
                    goal = step.goal,
                )),
                auth: self.auth_hook(),
                hooks: HarnessHooks {
                    after_provider_response: Some(self.governance.budget.clone()),
                    before_tool_call: before_tool_call.clone(),
                    prepare_next_turn: Some(phase_mgr),
                    ..HarnessHooks::none()
                },
                ..AgentHarnessOptions::new(self.llm.model.clone())
            };

            let client = self.make_client();
            let harness = AgentHarness::new_in_memory(client, self.env.clone(), opts).await;
            let mut rx = harness.subscribe();

            harness
                .prompt(format!("Solve step {}: {}", step.id, step.goal))
                .await?;

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
                    AgentHarnessEvent::Settled | AgentHarnessEvent::Aborted => break,
                    _ => {}
                }
            }

            // Check if replan was triggered
            let step_reason = self.context.lock().unwrap().replan_reason.clone();
            if step_reason.is_some() {
                crate::governance::record_audit(
                    &self.governance.audit,
                    AuditEventType::StateTransition,
                    serde_json::json!({"event": "replan", "step": step.id}),
                )
                .await;
                return Ok(());
            }

            let finish_text = raw
                .lines()
                .skip_while(|l| !l.starts_with("FINISH:"))
                .collect::<Vec<_>>()
                .join("\n");

            self.context.lock().unwrap().step_results.push(StepResult {
                step_id: step.id.clone(),
                finish_text: if finish_text.is_empty() {
                    raw
                } else {
                    finish_text
                },
            });
        }
        Ok(())
    }

    async fn run_synthesize(&mut self) -> Result<String> {
        use llm_harness::{AgentHarness, AgentHarnessEvent, AgentHarnessOptions, HarnessHooks};
        use llm_harness_types::{AgentEvent, ContentBlock};

        crate::governance::record_audit(
            &self.governance.audit,
            AuditEventType::StateTransition,
            serde_json::json!({"phase": "synthesize"}),
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

        let opts = AgentHarnessOptions {
            model: self.llm.model.clone(),
            tools: vec![],
            system_prompt: Some(
                "You are a math tutor writing a final answer synthesis. \
                 Be clear, structured, and educational."
                    .into(),
            ),
            auth: self.auth_hook(),
            hooks: HarnessHooks {
                after_provider_response: Some(self.governance.budget.clone()),
                ..HarnessHooks::none()
            },
            ..AgentHarnessOptions::new(self.llm.model.clone())
        };

        let harness = AgentHarness::new_in_memory(client, self.env.clone(), opts).await;
        let mut rx = harness.subscribe();

        harness.prompt(prompt).await?;

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
            "No synthesis generated.".into()
        } else {
            last_text
        })
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
