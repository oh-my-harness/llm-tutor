use std::sync::{Arc, Mutex};

use llm_harness_types::ExecutionEnv;

use crate::error::{Result, TutorError};
use crate::solve_context::{Plan, SolveContext, StepResult};

/// Drives the four-phase Deep Solve pipeline.
#[allow(dead_code)]
pub struct SolveOrchestrator {
    context: Arc<Mutex<SolveContext>>,
    env: Arc<dyn ExecutionEnv>,
    model: String,
    anthropic_api_key: String,
}

impl SolveOrchestrator {
    pub fn new(
        question: impl Into<String>,
        env: Arc<dyn ExecutionEnv>,
        model: impl Into<String>,
        anthropic_api_key: impl Into<String>,
    ) -> Self {
        Self {
            context: Arc::new(Mutex::new(SolveContext::new(question))),
            env,
            model: model.into(),
            anthropic_api_key: anthropic_api_key.into(),
        }
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
        self.context.lock().unwrap().kb_summary = Some(format!("[KB summary for: {kb}]"));
        Ok(())
    }

    async fn run_plan(&mut self) -> Result<()> {
        use llm_adapter::anthropic::AnthropicProvider;
        use llm_harness::{AgentHarness, AgentHarnessEvent, AgentHarnessOptions};
        use llm_harness_runtime_auth::EnvAuthHook;
        use llm_harness_types::{AgentEvent, ContentBlock};
        use std::sync::Arc;

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

        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| TutorError::Internal("ANTHROPIC_API_KEY not set".into()))?;
        let client = Arc::new(AnthropicProvider::builder(api_key).build());

        let opts = AgentHarnessOptions {
            model: self.model.clone(),
            tools: vec![],
            system_prompt: Some(
                "You are a math tutor planning a structured solution. \
                 Respond only with the requested JSON."
                    .into(),
            ),
            auth: Some(Arc::new(EnvAuthHook::for_provider("anthropic"))),
            ..AgentHarnessOptions::new(self.model.clone())
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
        use llm_adapter::anthropic::AnthropicProvider;
        use llm_harness::{AgentHarness, AgentHarnessEvent, AgentHarnessOptions, HarnessHooks};
        use llm_harness_runtime_auth::EnvAuthHook;
        use llm_harness_types::{AgentEvent, ContentBlock};
        use std::sync::Arc;
        use tutor_tools::{CodeExecTool, RagSearchTool, WebSearchTool};

        use crate::phase_manager::PhaseManager;
        use crate::replan_hook::ReplanHook;
        use crate::replan_tool::ReplanTool;

        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| TutorError::Internal("ANTHROPIC_API_KEY not set".into()))?;

        let steps = self
            .context
            .lock()
            .unwrap()
            .plan
            .as_ref()
            .ok_or_else(|| TutorError::Internal("no plan".into()))?
            .steps
            .clone();

        // ReplanHook shares the orchestrator's context directly via Arc.
        let replan_hook = Arc::new(ReplanHook::new(self.context.clone()));

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
                model: self.model.clone(),
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
                auth: Some(Arc::new(EnvAuthHook::for_provider("anthropic"))),
                hooks: HarnessHooks {
                    before_tool_call: Some(replan_hook.clone()),
                    prepare_next_turn: Some(phase_mgr),
                    ..HarnessHooks::none()
                },
                ..AgentHarnessOptions::new(self.model.clone())
            };

            let client = Arc::new(AnthropicProvider::builder(api_key.clone()).build());
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
        use llm_adapter::anthropic::AnthropicProvider;
        use llm_harness::{AgentHarness, AgentHarnessEvent, AgentHarnessOptions};
        use llm_harness_runtime_auth::EnvAuthHook;
        use llm_harness_types::{AgentEvent, ContentBlock};
        use std::sync::Arc;

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

        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| TutorError::Internal("ANTHROPIC_API_KEY not set".into()))?;
        let client = Arc::new(AnthropicProvider::builder(api_key).build());

        let opts = AgentHarnessOptions {
            model: self.model.clone(),
            tools: vec![],
            system_prompt: Some(
                "You are a math tutor writing a final answer synthesis. \
                 Be clear, structured, and educational."
                    .into(),
            ),
            auth: Some(Arc::new(EnvAuthHook::for_provider("anthropic"))),
            ..AgentHarnessOptions::new(self.model.clone())
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
