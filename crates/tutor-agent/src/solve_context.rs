use serde::{Deserialize, Serialize};

/// Shared state across all four Deep Solve phases.
pub struct SolveContext {
    pub question: String,
    pub kb_summary: Option<String>,
    pub plan: Option<Plan>,
    pub step_results: Vec<StepResult>,
    pub replan_count: usize,
    pub replan_reason: Option<String>,
    pub max_replans: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Plan {
    pub analysis: String,
    pub steps: Vec<PlanStep>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanStep {
    pub id: String,
    pub goal: String,
}

#[derive(Debug, Clone)]
pub struct StepResult {
    pub step_id: String,
    pub finish_text: String,
}

impl SolveContext {
    pub fn new(question: impl Into<String>) -> Self {
        Self {
            question: question.into(),
            kb_summary: None,
            plan: None,
            step_results: Vec::new(),
            replan_count: 0,
            replan_reason: None,
            max_replans: 2,
        }
    }

    /// Prepare for a new Plan attempt after a REPLAN signal.
    pub fn reset_for_replan(&mut self) {
        self.plan = None;
        self.step_results.clear();
        self.replan_count += 1;
        self.replan_reason = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_starts_empty() {
        let ctx = SolveContext::new("What is e?");
        assert_eq!(ctx.question, "What is e?");
        assert!(ctx.plan.is_none());
        assert!(ctx.replan_reason.is_none());
        assert_eq!(ctx.replan_count, 0);
        assert_eq!(ctx.max_replans, 2);
    }

    #[test]
    fn reset_for_replan_clears_plan_and_steps() {
        let mut ctx = SolveContext::new("q");
        ctx.plan = Some(Plan {
            analysis: "a".into(),
            steps: vec![PlanStep {
                id: "1".into(),
                goal: "g".into(),
            }],
        });
        ctx.step_results.push(StepResult {
            step_id: "1".into(),
            finish_text: "done".into(),
        });
        ctx.replan_reason = Some("better approach".into());
        ctx.reset_for_replan();
        assert!(ctx.plan.is_none());
        assert!(ctx.step_results.is_empty());
        assert!(ctx.replan_reason.is_none());
        assert_eq!(ctx.replan_count, 1);
    }
}
