use llm_harness_runtime::workflow::model::{ConditionExpr, Edge, EdgeCondition, Step, Workflow};
use llm_harness_runtime::workflow::plan::validate_workflow;

use crate::error::{Result, TutorError};

pub const DEEP_SOLVE_WORKFLOW_ID: &str = "tutor.deep_solve";
pub const QUIZ_GENERATION_WORKFLOW_ID: &str = "tutor.quiz_generation";
pub const MEMORY_WORKFLOW_ID: &str = "tutor.memory";

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
                "Read the workflow Context for `question` and optional `kb_summary`. \
                 Create a concise, grounded step-by-step plan for solving the learner question. \
                 Output the plan as readable text; the next step will receive this step history.",
                vec![],
            ),
            Step::llm(
                "solve",
                "Solve steps",
                "Read the workflow Context and prior step history, then execute the current solve plan. \
                 Use available tools when verification, calculation, memory, RAG, or fresh evidence is needed. \
                 For non-trivial numeric calculations, approximations, transcendental functions, statistics, \
                 or simulations, use code_exec to compute or verify the result. \
                 When this step is complete, call submit_step_result with a JSON object. \
                 Use {\"route\":\"finish\",\"summary\":\"...\"} when the work is ready for synthesis. \
                 Use {\"route\":\"replan\",\"reason\":\"...\"} only if the current plan is fundamentally wrong.",
                vec![
                    "rag_search".into(),
                    "read_memory".into(),
                    "write_memory".into(),
                    "web_search".into(),
                    "web_fetch".into(),
                    "code_exec".into(),
                    "replan".into(),
                ],
            ),
            Step::llm(
                "synthesize",
                "Synthesize answer",
                "Read the workflow Context and prior step history. Synthesize the verified work into a clear final answer for the learner. \
                 Start with the direct answer, then provide the explanation.",
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
                condition: Some(route_condition("replan")),
            },
            Edge {
                from: "solve".into(),
                to: "synthesize".into(),
                condition: Some(route_condition("finish")),
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
                "Read the workflow Context. The `quiz_generation_prompt` variable contains the full source-grounded quiz generation instruction. \
                 Generate grounded single-choice quiz questions from those sources. If prior step history includes verifier repair feedback, repair the draft. \
                 When done, call submit_step_result with {\"questions\":[...]} using the exact question schema requested in Context.",
                vec![],
            ),
            Step::llm(
                "verify_questions",
                "Verify generated questions",
                "Read the workflow Context and prior generate_questions structured result. Strictly verify every question against its cited source chunks. \
                 When done, call submit_step_result with {\"verdict\":\"pass\",\"issues\":[]} if all questions are grounded. \
                 If any question is unsupported, contradictory, or wrongly cited, call submit_step_result with \
                 {\"verdict\":\"fail\",\"action\":\"repair\",\"issues\":[\"...\"]}.",
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
                condition: Some(verdict_condition("pass")),
            },
            Edge {
                from: "verify_questions".into(),
                to: "generate_questions".into(),
                condition: Some(action_condition("repair")),
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

pub fn memory_workflow() -> Workflow {
    Workflow {
        entry_step: "prepare_memory".into(),
        steps: vec![
            Step::executor(
                "prepare_memory",
                "Prepare memory workflow input",
                "tutor.memory.prepare",
                None,
            ),
            Step::llm(
                "run_memory",
                "Run memory workflow",
                "Read the workflow Context. The `memory_prompt` variable contains the full memory maintenance instruction, including target file, action, current Markdown, normalized evidence, and output schema. \
                 Maintain learner memory according to that instruction. When done, call submit_step_result with the JSON object requested by `memory_prompt`.",
                vec![],
            ),
        ],
        edges: vec![Edge {
            from: "prepare_memory".into(),
            to: "run_memory".into(),
            condition: None,
        }],
    }
}

pub fn validate_memory_workflow() -> Result<()> {
    validate_workflow(&memory_workflow()).map_err(|err| {
        TutorError::Internal(format!(
            "runtime workflow validation failed for {MEMORY_WORKFLOW_ID}: {err}"
        ))
    })
}

fn route_condition(route: &str) -> EdgeCondition {
    EdgeCondition::Expr(ConditionExpr::Eq {
        pointer: "/route".into(),
        value: serde_json::json!(route),
    })
}

fn verdict_condition(verdict: &str) -> EdgeCondition {
    EdgeCondition::Expr(ConditionExpr::Eq {
        pointer: "/verdict".into(),
        value: serde_json::json!(verdict),
    })
}

fn action_condition(action: &str) -> EdgeCondition {
    EdgeCondition::Expr(ConditionExpr::Eq {
        pointer: "/action".into(),
        value: serde_json::json!(action),
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

    #[test]
    fn memory_workflow_is_valid_runtime_workflow() {
        validate_memory_workflow().unwrap();
    }

    #[test]
    fn workflows_use_runtime_evaluable_edge_conditions() {
        for workflow in [deep_solve_workflow(), quiz_generation_workflow()] {
            for edge in workflow.edges {
                assert!(
                    !matches!(edge.condition, Some(EdgeCondition::Label(_))),
                    "workflow {} -> {} should use Expr conditions so runtime EdgeConditionJudge can route it",
                    edge.from,
                    edge.to
                );
            }
        }
    }

    #[test]
    fn deep_solve_workflow_routes_on_structured_route_field() {
        let workflow = deep_solve_workflow();
        let conditions = workflow
            .edges
            .iter()
            .filter(|edge| edge.from == "solve")
            .map(|edge| (&edge.to, edge.condition.as_ref().unwrap()))
            .collect::<Vec<_>>();

        assert!(conditions.contains(&(
            &"plan".to_string(),
            &EdgeCondition::Expr(ConditionExpr::Eq {
                pointer: "/route".into(),
                value: serde_json::json!("replan"),
            })
        )));
        assert!(conditions.contains(&(
            &"synthesize".to_string(),
            &EdgeCondition::Expr(ConditionExpr::Eq {
                pointer: "/route".into(),
                value: serde_json::json!("finish"),
            })
        )));
    }

    #[test]
    fn quiz_workflow_routes_on_verifier_structured_fields() {
        let workflow = quiz_generation_workflow();
        let conditions = workflow
            .edges
            .iter()
            .filter(|edge| edge.from == "verify_questions")
            .map(|edge| (&edge.to, edge.condition.as_ref().unwrap()))
            .collect::<Vec<_>>();

        assert!(conditions.contains(&(
            &"publish_questions".to_string(),
            &EdgeCondition::Expr(ConditionExpr::Eq {
                pointer: "/verdict".into(),
                value: serde_json::json!("pass"),
            })
        )));
        assert!(conditions.contains(&(
            &"generate_questions".to_string(),
            &EdgeCondition::Expr(ConditionExpr::Eq {
                pointer: "/action".into(),
                value: serde_json::json!("repair"),
            })
        )));
    }
}
