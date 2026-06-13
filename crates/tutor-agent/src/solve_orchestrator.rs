use std::sync::{Arc, Mutex};

use llm_harness_types::ExecutionEnv;

use crate::error::{Result, TutorError};
use crate::solve_context::{Plan, SolveContext, StepResult};

/// Drives the four-phase Deep Solve pipeline.
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
        // Phase 1 stub: set kb_summary to the kb parameter
        self.context.lock().unwrap().kb_summary = Some(format!("[KB summary for: {kb}]"));
        Ok(())
    }

    async fn run_plan(&mut self) -> Result<()> {
        // TODO Task 6: real harness call to generate JSON plan
        let question = self.context.lock().unwrap().question.clone();
        self.context.lock().unwrap().plan = Some(Plan {
            analysis: format!("Analyze: {question}"),
            steps: vec![crate::solve_context::PlanStep {
                id: "step-1".into(),
                goal: format!("Solve: {question}"),
            }],
        });
        Ok(())
    }

    async fn run_solve_steps(&mut self) -> Result<()> {
        // TODO Task 7: real harness per step
        let steps = self
            .context
            .lock()
            .unwrap()
            .plan
            .as_ref()
            .ok_or_else(|| TutorError::Internal("no plan".into()))?
            .steps
            .clone();

        for step in &steps {
            self.context.lock().unwrap().step_results.push(StepResult {
                step_id: step.id.clone(),
                finish_text: format!("[stub result for {}]", step.goal),
            });
        }
        Ok(())
    }

    async fn run_synthesize(&mut self) -> Result<String> {
        // TODO Task 8: real harness call to synthesize
        let summary = self
            .context
            .lock()
            .unwrap()
            .step_results
            .iter()
            .map(|r| r.finish_text.clone())
            .collect::<Vec<_>>()
            .join("\n");
        Ok(format!("Synthesis:\n{summary}"))
    }
}

/// True if a replan should be triggered: reason is set AND under the limit.
pub fn should_replan(ctx: &SolveContext) -> bool {
    ctx.replan_reason.is_some() && ctx.replan_count < ctx.max_replans
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
}
