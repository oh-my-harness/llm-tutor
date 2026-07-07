use llm_harness_runtime::workflow::model::{Edge, EdgeCondition, Step, Workflow};
use llm_harness_runtime::workflow::plan::validate_workflow;

use crate::error::{Result, TutorError};

pub const DEEP_SOLVE_WORKFLOW_ID: &str = "tutor.deep_solve";
pub const QUIZ_GENERATION_WORKFLOW_ID: &str = "tutor.quiz_generation";

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

pub fn quiz_generation_workflow() -> Workflow {
    Workflow {
        entry_step: "collect_sources".into(),
        steps: vec![
            Step::executor(
                "collect_sources",
                "Collect quiz sources",
                "tutor.quiz.collect_sources",
                None,
            ),
            Step::llm(
                "generate_questions",
                "Generate grounded questions",
                "Generate grounded single-choice quiz questions from the collected sources. Return structured JSON only.",
                vec![],
            ),
            Step::llm(
                "verify_questions",
                "Verify generated questions",
                "Strictly verify every question against its cited source chunks. Return structured JSON with pass/fail and issues.",
                vec![],
            ),
            Step::executor(
                "publish_questions",
                "Publish verified questions",
                "tutor.quiz.publish_questions",
                None,
            ),
        ],
        edges: vec![
            Edge {
                from: "collect_sources".into(),
                to: "generate_questions".into(),
                condition: None,
            },
            Edge {
                from: "generate_questions".into(),
                to: "verify_questions".into(),
                condition: None,
            },
            Edge {
                from: "verify_questions".into(),
                to: "publish_questions".into(),
                condition: Some(EdgeCondition::Label("pass".into())),
            },
            Edge {
                from: "verify_questions".into(),
                to: "generate_questions".into(),
                condition: Some(EdgeCondition::Label("repair".into())),
            },
        ],
    }
}

pub fn validate_quiz_generation_workflow() -> Result<()> {
    validate_workflow(&quiz_generation_workflow()).map_err(|err| {
        TutorError::Internal(format!(
            "runtime workflow validation failed for {QUIZ_GENERATION_WORKFLOW_ID}: {err}"
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

    #[test]
    fn quiz_generation_workflow_is_valid_runtime_workflow() {
        validate_quiz_generation_workflow().unwrap();
    }
}
