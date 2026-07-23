use std::path::{Path, PathBuf};
use std::sync::Arc;

use llm_harness_loop::LlmClient;
use llm_harness_runtime::spawn::spawner::{EnvFactory, JsonlSessionFactory};
use llm_harness_runtime::workflow::engine::WorkflowEngineConfig;
use llm_harness_runtime::workflow::judge::{StepCtx, StepTransitionJudge};
use llm_harness_runtime::workflow::model::Transition;
use llm_harness_types::{AgentError, ExecutionEnv};

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
        customize_builder: None,
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

/// Marker judge that asks `WorkflowEngine` to select runtime's built-in
/// declarative edge judge for `EdgeCondition::Expr` workflows.
///
/// Runtime's actual no-op judge is not public, so this tiny marker implements
/// `is_noop()` and should be replaced by `WorkflowEngine::new` before it is
/// ever asked to decide a transition.
pub struct RuntimeDeclarativeJudge;

impl StepTransitionJudge for RuntimeDeclarativeJudge {
    fn decide<'a>(&'a self, ctx: &StepCtx<'a>) -> futures::future::BoxFuture<'a, Transition> {
        let current_step = ctx.current_step.id().clone();
        Box::pin(async move {
            Transition::Fail {
                reason: format!(
                    "runtime declarative judge marker was not replaced for step '{current_step}'"
                ),
            }
        })
    }

    fn is_noop(&self) -> bool {
        true
    }
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
    use llm_harness_runtime::workflow::model::{
        ConditionExpr, Edge, EdgeCondition, Step, StepResult, StructuredStatus, Workflow,
    };

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
                    structured_status: StructuredStatus::NotRequired,
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
    async fn workflow_engine_uses_runtime_declarative_judge_for_expr_edges() {
        struct FinishExecutor;

        impl StepExecutor for FinishExecutor {
            fn execute<'a>(
                &'a self,
                _ctx: &'a ExecutorCtx<'a>,
            ) -> BoxFuture<'a, anyhow::Result<StepResult>> {
                Box::pin(async {
                    Ok(StepResult {
                        output: "runtime declarative route executed".into(),
                        structured: None,
                        structured_status: StructuredStatus::NotRequired,
                        tool_calls_count: 0,
                        session_id: String::new(),
                        cost: CostAggregate::default(),
                        started_at: None,
                        ended_at: None,
                    })
                })
            }
        }

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
            entry_step: "s1".into(),
            steps: vec![
                Step::executor("s1", "Step 1", "tutor.test.fixed", None),
                Step::executor("s2", "Step 2", "tutor.test.finish", None),
            ],
            edges: vec![Edge {
                from: "s1".into(),
                to: "s2".into(),
                condition: Some(EdgeCondition::Expr(ConditionExpr::Eq {
                    pointer: "/ok".into(),
                    value: serde_json::json!(true),
                })),
            }],
        };
        let engine = WorkflowEngine::new(workflow, config, Arc::new(RuntimeDeclarativeJudge))
            .unwrap()
            .with_executor("tutor.test.fixed", Arc::new(FixedExecutor))
            .with_executor("tutor.test.finish", Arc::new(FinishExecutor));

        let result = engine.run().await.unwrap();

        assert_eq!(
            result.final_message.as_deref(),
            Some("runtime declarative route executed")
        );
        assert_eq!(result.turns, 2);
    }
}
