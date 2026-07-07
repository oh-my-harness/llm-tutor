use llm_harness_runtime::workflow::model::{Edge, EdgeCondition, Step, Workflow};
use llm_harness_runtime::workflow::plan::validate_workflow;

use crate::error::{Result, TutorError};

pub const DEEP_SOLVE_WORKFLOW_ID: &str = "tutor.deep_solve";

pub fn deep_solve_workflow() -> Workflow {
    Workflow {
        entry_step: "retrieve".into(),
        steps: vec![
            Step::executor(
                "retrieve",
                "Retrieve context",
                "tutor.deep_solve.retrieve",
                None,
            ),
            Step::llm(
                "plan",
                "Create solve plan",
                "Create a concise, grounded plan for solving the learner question.",
                vec![],
            ),
            Step::llm(
                "solve",
                "Solve steps",
                "Execute the current solve plan. Use available tools when verification or fresh evidence is needed.",
                vec![
                    "rag_search".into(),
                    "web_search".into(),
                    "code_exec".into(),
                    "replan".into(),
                ],
            ),
            Step::llm(
                "synthesize",
                "Synthesize answer",
                "Synthesize the verified work into a clear final answer for the learner.",
                vec![],
            ),
        ],
        edges: vec![
            Edge {
                from: "retrieve".into(),
                to: "plan".into(),
                condition: None,
            },
            Edge {
                from: "plan".into(),
                to: "solve".into(),
                condition: None,
            },
            Edge {
                from: "solve".into(),
                to: "plan".into(),
                condition: Some(EdgeCondition::Label("replan_requested".into())),
            },
            Edge {
                from: "solve".into(),
                to: "synthesize".into(),
                condition: Some(EdgeCondition::Label("finish".into())),
            },
        ],
    }
}

pub fn validate_deep_solve_workflow() -> Result<()> {
    validate_workflow(&deep_solve_workflow()).map_err(|err| {
        TutorError::Internal(format!(
            "runtime workflow validation failed for {DEEP_SOLVE_WORKFLOW_ID}: {err}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deep_solve_workflow_is_valid_runtime_workflow() {
        validate_deep_solve_workflow().unwrap();
    }
}
