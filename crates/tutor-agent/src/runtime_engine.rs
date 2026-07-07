use std::path::{Path, PathBuf};
use std::sync::Arc;

use llm_harness_loop::LlmClient;
use llm_harness_runtime::spawn::spawner::{EnvFactory, JsonlSessionFactory};
use llm_harness_runtime::workflow::engine::WorkflowEngineConfig;
use llm_harness_runtime::workflow::judge::{StepCtx, StepTransitionJudge};
use llm_harness_runtime::workflow::model::{
    ConditionExpr, Edge, EdgeCondition, Transition, Workflow,
};
use llm_harness_types::{AgentError, ExecutionEnv};
use serde_json::Value;

/// Product-to-runtime adapter for workflow execution.
///
/// Keep this layer thin: product code supplies the current execution
/// environment, model and client; runtime owns sessions, workflow state,
/// step execution and transition judging.
pub fn build_workflow_engine_config(
    client: Arc<dyn LlmClient>,
    model: impl Into<String>,
    env: Arc<dyn ExecutionEnv>,
    session_base_dir: PathBuf,
) -> WorkflowEngineConfig {
    WorkflowEngineConfig {
        client,
        model: model.into(),
        env_factory: Arc::new(FixedEnvFactory::new(env)),
        session_factory: Arc::new(JsonlSessionFactory),
        session_base_dir,
    }
}

struct FixedEnvFactory {
    env: Arc<dyn ExecutionEnv>,
}

impl FixedEnvFactory {
    fn new(env: Arc<dyn ExecutionEnv>) -> Self {
        Self { env }
    }
}

impl EnvFactory for FixedEnvFactory {
    fn create(&self, _cwd: &Path) -> Result<Arc<dyn ExecutionEnv>, AgentError> {
        Ok(self.env.clone())
    }
}

/// Thin product-side bridge for declarative runtime workflow edges.
///
/// Runtime has a built-in edge judge, but it is not public yet. Keep this
/// adapter behavior equivalent and boring until the framework exposes it.
pub struct DeclarativeEdgeJudge {
    edges: Vec<Edge>,
}

impl DeclarativeEdgeJudge {
    pub fn new(workflow: &Workflow) -> Self {
        Self {
            edges: workflow.edges.clone(),
        }
    }

    fn decide_sync(&self, ctx: &StepCtx<'_>) -> Transition {
        let outgoing = self
            .edges
            .iter()
            .filter(|edge| edge.from.as_str() == ctx.current_step.id().as_str())
            .collect::<Vec<_>>();
        if outgoing.is_empty() {
            return Transition::Abort {
                reason: "workflow complete".into(),
            };
        }

        let mut fallback: Option<&Edge> = None;
        for edge in outgoing {
            match &edge.condition {
                None => {
                    if fallback.is_some() {
                        return Transition::Fail {
                            reason: format!(
                                "multiple unconditional edges from step '{}'",
                                ctx.current_step.id()
                            ),
                        };
                    }
                    fallback = Some(edge);
                }
                Some(EdgeCondition::Expr(expr)) => {
                    if condition_matches(expr, ctx.last_result.structured.as_ref()) {
                        return Transition::To(edge.to.clone());
                    }
                }
                Some(EdgeCondition::Label(label)) => {
                    return Transition::Fail {
                        reason: format!(
                            "workflow edge label '{}' from step '{}' requires a custom judge",
                            label,
                            ctx.current_step.id()
                        ),
                    };
                }
            }
        }

        if let Some(edge) = fallback {
            return Transition::To(edge.to.clone());
        }
        Transition::Fail {
            reason: format!(
                "no edge condition matched for step '{}'",
                ctx.current_step.id()
            ),
        }
    }
}

impl StepTransitionJudge for DeclarativeEdgeJudge {
    fn decide<'a>(&'a self, ctx: &StepCtx<'a>) -> futures::future::BoxFuture<'a, Transition> {
        let transition = self.decide_sync(ctx);
        Box::pin(async move { transition })
    }
}

fn condition_matches(expr: &ConditionExpr, structured: Option<&Value>) -> bool {
    match expr {
        ConditionExpr::Exists { pointer } => read_pointer(structured, pointer).is_some(),
        ConditionExpr::Missing { pointer } => read_pointer(structured, pointer).is_none(),
        ConditionExpr::Eq { pointer, value } => {
            read_pointer(structured, pointer).is_some_and(|found| found == value)
        }
        ConditionExpr::Ne { pointer, value } => {
            read_pointer(structured, pointer).is_some_and(|found| found != value)
        }
        ConditionExpr::Gt { pointer, value } => {
            compare_number(structured, pointer, *value, |found, expected| {
                found > expected
            })
        }
        ConditionExpr::Gte { pointer, value } => {
            compare_number(structured, pointer, *value, |found, expected| {
                found >= expected
            })
        }
        ConditionExpr::Lt { pointer, value } => {
            compare_number(structured, pointer, *value, |found, expected| {
                found < expected
            })
        }
        ConditionExpr::Lte { pointer, value } => {
            compare_number(structured, pointer, *value, |found, expected| {
                found <= expected
            })
        }
    }
}

fn read_pointer<'a>(structured: Option<&'a Value>, pointer: &str) -> Option<&'a Value> {
    structured.and_then(|value| value.pointer(pointer))
}

fn compare_number(
    structured: Option<&Value>,
    pointer: &str,
    expected: f64,
    compare: impl FnOnce(f64, f64) -> bool,
) -> bool {
    read_pointer(structured, pointer)
        .and_then(Value::as_f64)
        .is_some_and(|found| compare(found, expected))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::future::BoxFuture;
    use llm_harness_loop::test_utils::{MockLlmClient, NoOpEnv};
    use llm_harness_runtime::control::cost::CostAggregate;
    use llm_harness_runtime::workflow::engine::WorkflowEngine;
    use llm_harness_runtime::workflow::executor::{ExecutorCtx, StepExecutor};
    use llm_harness_runtime::workflow::judge::{StepCtx, StepTransitionJudge};
    use llm_harness_runtime::workflow::model::{Edge, EdgeCondition, Step, StepResult, Workflow};

    struct FixedExecutor;

    impl StepExecutor for FixedExecutor {
        fn execute<'a>(
            &'a self,
            _ctx: &'a ExecutorCtx<'a>,
        ) -> BoxFuture<'a, anyhow::Result<StepResult>> {
            Box::pin(async {
                Ok(StepResult {
                    output: "runtime workflow executed".into(),
                    structured: Some(serde_json::json!({ "ok": true })),
                    tool_calls_count: 0,
                    session_id: String::new(),
                    cost: CostAggregate::default(),
                    started_at: None,
                    ended_at: None,
                })
            })
        }
    }

    struct FinishJudge;

    impl StepTransitionJudge for FinishJudge {
        fn decide<'a>(&'a self, _ctx: &StepCtx<'a>) -> BoxFuture<'a, Transition> {
            Box::pin(async {
                Transition::Abort {
                    reason: "done".into(),
                }
            })
        }
    }

    #[tokio::test]
    async fn workflow_engine_config_runs_executor_workflow() {
        let dir = tempfile::TempDir::new().unwrap();
        let client = Arc::new(MockLlmClient::new(vec![]));
        let env = Arc::new(NoOpEnv) as Arc<dyn ExecutionEnv>;
        let config = build_workflow_engine_config(
            client,
            "mock-model",
            env,
            dir.path().join("workflow-sessions"),
        );
        let workflow = Workflow {
            entry_step: "publish".into(),
            steps: vec![Step::executor(
                "publish",
                "Publish",
                "tutor.test.fixed",
                None,
            )],
            edges: vec![],
        };
        let engine = WorkflowEngine::new(workflow, config, Arc::new(FinishJudge))
            .unwrap()
            .with_executor("tutor.test.fixed", Arc::new(FixedExecutor));

        let result = engine.run().await.unwrap();

        assert_eq!(
            result.final_message.as_deref(),
            Some("runtime workflow executed")
        );
        assert_eq!(result.turns, 1);
    }

    #[tokio::test]
    async fn declarative_edge_judge_routes_on_structured_result() {
        let workflow = Workflow {
            entry_step: "s1".into(),
            steps: vec![
                Step::llm("s1", "Step 1", "do step 1", vec![]),
                Step::llm("s2", "Step 2", "do step 2", vec![]),
            ],
            edges: vec![Edge {
                from: "s1".into(),
                to: "s2".into(),
                condition: Some(EdgeCondition::Expr(ConditionExpr::Eq {
                    pointer: "/route".into(),
                    value: serde_json::json!("finish"),
                })),
            }],
        };
        let judge = DeclarativeEdgeJudge::new(&workflow);
        let step = workflow.steps[0].clone();
        let result = StepResult {
            output: "ok".into(),
            structured: Some(serde_json::json!({ "route": "finish" })),
            tool_calls_count: 0,
            session_id: "s".into(),
            cost: CostAggregate::default(),
            started_at: None,
            ended_at: None,
        };
        let ctx = StepCtx {
            current_step: &step,
            last_result: &result,
            step_history: &[],
            context: Box::leak(Box::new(Default::default())),
        };

        let transition = judge.decide(&ctx).await;

        assert!(matches!(transition, Transition::To(next) if next == "s2"));
    }
}
