use std::path::PathBuf;
use std::sync::Arc;

use futures::future::BoxFuture;
use llm_adapter::provider::Provider;
use llm_harness_agent::HarnessHooks;
use llm_harness_runtime::control::cost::CostAggregate;
use llm_harness_runtime::observability::audit::AuditEventType;
use llm_harness_runtime::workflow::engine::{WorkflowEngine, WorkflowEvent};
use llm_harness_runtime::workflow::executor::{ExecutorCtx, StepExecutor};
use llm_harness_runtime::workflow::model::StepResult as RuntimeStepResult;
use llm_harness_types::{BeforeToolCallHook, ExecutionEnv, Tool};

use crate::deep_solve_events as deep_events;
use crate::error::{Result, TutorError};
use crate::event_sink::{SharedEventSink, emit_content, emit_trace};
use crate::governance::GovernanceConfig;
use crate::llm_provider::LlmConfig;
use crate::runtime_engine::{RuntimeDeclarativeJudge, build_workflow_engine_config};
use crate::runtime_workflow::{
    DEEP_SOLVE_WORKFLOW_ID, deep_solve_workflow_with_memory,
    validate_deep_solve_workflow_with_memory,
};
use tutor_tools::WebSearchConfig;

/// Drives the four-phase Deep Solve pipeline.
pub struct SolveOrchestrator {
    question: String,
    env: Arc<dyn ExecutionEnv>,
    llm: LlmConfig,
    governance: GovernanceConfig,
    event_sink: Option<SharedEventSink>,
    web_search: Option<WebSearchConfig>,
    client: Option<Arc<dyn Provider>>,
    workflow_root: Option<PathBuf>,
    memory_root: Option<PathBuf>,
    learner_memory_access: bool,
    product_instruction: Option<String>,
}

impl SolveOrchestrator {
    pub fn new(
        question: impl Into<String>,
        env: Arc<dyn ExecutionEnv>,
        llm: LlmConfig,
        governance: GovernanceConfig,
    ) -> Self {
        Self {
            question: question.into(),
            env,
            llm,
            governance,
            event_sink: None,
            web_search: None,
            client: None,
            workflow_root: None,
            memory_root: None,
            learner_memory_access: true,
            product_instruction: None,
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

    pub fn with_workflow_root(mut self, root: Option<PathBuf>) -> Self {
        self.workflow_root = root;
        self
    }

    pub fn with_memory_root(mut self, root: Option<PathBuf>) -> Self {
        self.memory_root = root;
        self
    }

    pub fn with_learner_memory_access(mut self, allowed: bool) -> Self {
        self.learner_memory_access = allowed;
        self
    }

    pub fn with_product_instruction(mut self, instruction: Option<String>) -> Self {
        self.product_instruction = instruction;
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
        validate_deep_solve_workflow_with_memory(self.learner_memory_access)?;
        let workflow = deep_solve_workflow_with_memory(self.learner_memory_access);
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

        deep_events::stage_start(
            &self.event_sink,
            deep_events::DeepSolveStage::Plan,
            "Run Deep Solve workflow",
        )
        .await;

        let question = self.question.clone();
        let client = self.make_client();
        let session_root = self
            .workflow_root
            .clone()
            .unwrap_or_else(|| std::env::temp_dir().join("llm-tutor-workflow-sessions"))
            .join("deep-solve");
        let config = build_workflow_engine_config(
            client.clone(),
            self.llm.model.clone(),
            self.env.clone(),
            session_root,
        );

        let mut engine =
            WorkflowEngine::new(workflow.clone(), config, Arc::new(RuntimeDeclarativeJudge))
                .map_err(|err| {
                    TutorError::Internal(format!("deep solve workflow init failed: {err}"))
                })?
                .with_executor(
                    "tutor.deep_solve.retrieve",
                    Arc::new(DeepSolveRetrieveExecutor {
                        question,
                        product_instruction: self.product_instruction.clone(),
                        kb: kb.map(str::to_string),
                        event_sink: self.event_sink.clone(),
                        governance: self.governance.clone(),
                    }),
                )
                .with_hooks(HarnessHooks {
                    before_tool_call: self.deep_solve_before_tool_hooks(),
                    ..HarnessHooks::none()
                });

        for tool in self.deep_solve_tools() {
            engine = engine.with_tool(tool);
        }

        let event_task =
            relay_deep_solve_workflow_events(engine.subscribe(), self.event_sink.clone());
        let result = engine
            .run()
            .await
            .map_err(|err| TutorError::Internal(format!("deep solve workflow failed: {err}")))?;
        emit_workflow_runtime_usage(&self.event_sink, &result.cost).await;
        drop(engine);
        let _ = event_task.await;
        let answer = result
            .final_message
            .filter(|message| !message.trim().is_empty())
            .unwrap_or_else(|| "No synthesis generated.".into());
        emit_content(&self.event_sink, answer.clone(), false).await;
        deep_events::final_answer(&self.event_sink, &answer).await;
        deep_events::stage_done(
            &self.event_sink,
            deep_events::DeepSolveStage::Synthesize,
            "Run Deep Solve workflow",
            "Final explanation generated",
        )
        .await;
        Ok(answer)
    }

    fn deep_solve_before_tool_hooks(&self) -> Vec<Arc<dyn BeforeToolCallHook>> {
        if let Some(approval) = &self.governance.approval {
            vec![approval.clone() as Arc<dyn BeforeToolCallHook>]
        } else {
            vec![]
        }
    }

    fn deep_solve_tools(&self) -> Vec<Arc<dyn Tool>> {
        use tutor_tools::{
            CodeExecTool, RagSearchTool, ReadMemoryTool, WebFetchTool, WebSearchTool,
            WriteMemoryTool,
        };

        let mut tools: Vec<Arc<dyn Tool>> = vec![
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
        ];
        if self.learner_memory_access {
            tools.push(Arc::new(match self.memory_root.clone() {
                Some(root) => ReadMemoryTool::with_root(root),
                None => ReadMemoryTool::new(),
            }));
            tools.push(Arc::new(match self.memory_root.clone() {
                Some(root) => WriteMemoryTool::with_root(root),
                None => WriteMemoryTool::new(),
            }));
        }
        tools
    }
}

async fn emit_workflow_runtime_usage(sink: &Option<SharedEventSink>, cost: &CostAggregate) {
    emit_trace(
        sink,
        "runtime_usage",
        serde_json::json!({
            "capability": "deep_solve",
            "input_tokens": cost.total_input_tokens,
            "output_tokens": cost.total_output_tokens,
            "cache_read_tokens": cost.total_cache_read_tokens,
            "cache_write_tokens": cost.total_cache_write_tokens,
            "cost_usd": cost.total_cost,
        }),
    )
    .await;
}

fn relay_deep_solve_workflow_events(
    mut rx: tokio::sync::broadcast::Receiver<WorkflowEvent>,
    sink: Option<SharedEventSink>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match event {
                WorkflowEvent::StepStarted { step_id, step_name } => {
                    let stage = deep_solve_stage_for_step(&step_id);
                    deep_events::stage_start(&sink, stage, &step_name).await;
                }
                WorkflowEvent::StepFinished { step_id, result } => {
                    let stage = deep_solve_stage_for_step(&step_id);
                    let summary = result
                        .structured
                        .as_ref()
                        .and_then(|value| {
                            value
                                .get("summary")
                                .or_else(|| value.get("reason"))
                                .and_then(serde_json::Value::as_str)
                        })
                        .unwrap_or(&result.output)
                        .to_string();
                    deep_events::stage_done(&sink, stage, &step_id, summary).await;
                }
                WorkflowEvent::Failed { error } => {
                    emit_trace(
                        &sink,
                        "workflow_failed",
                        serde_json::json!({
                            "capability": "deep_solve",
                            "error": error,
                        }),
                    )
                    .await;
                }
                WorkflowEvent::Paused { .. }
                | WorkflowEvent::Resumed
                | WorkflowEvent::Cancelled { .. }
                | WorkflowEvent::StepProgress { .. } => {}
            }
        }
    })
}

fn deep_solve_stage_for_step(step_id: &str) -> deep_events::DeepSolveStage {
    match step_id {
        "retrieve" => deep_events::DeepSolveStage::Retrieve,
        "plan" => deep_events::DeepSolveStage::Plan,
        "solve" => deep_events::DeepSolveStage::Solve,
        "synthesize" => deep_events::DeepSolveStage::Synthesize,
        _ => deep_events::DeepSolveStage::Solve,
    }
}

struct DeepSolveRetrieveExecutor {
    question: String,
    product_instruction: Option<String>,
    kb: Option<String>,
    event_sink: Option<SharedEventSink>,
    governance: GovernanceConfig,
}

impl StepExecutor for DeepSolveRetrieveExecutor {
    fn execute<'a>(
        &'a self,
        ctx: &'a ExecutorCtx<'a>,
    ) -> BoxFuture<'a, anyhow::Result<RuntimeStepResult>> {
        Box::pin(async move {
            deep_events::stage_start(
                &self.event_sink,
                deep_events::DeepSolveStage::Retrieve,
                "Retrieve knowledge",
            )
            .await;
            emit_trace(
                &self.event_sink,
                "phase_start",
                serde_json::json!({ "capability": "deep_solve", "phase": "retrieve" }),
            )
            .await;

            let kb_summary = self
                .kb
                .as_ref()
                .map(|kb| format!("[KB summary for: {kb}]"))
                .unwrap_or_else(|| "none".into());
            {
                let mut context = ctx.context.lock().await;
                context
                    .variables
                    .insert("question".into(), serde_json::json!(self.question.clone()));
                if let Some(instruction) = self.product_instruction.as_deref() {
                    context
                        .variables
                        .insert("tutor_instruction".into(), serde_json::json!(instruction));
                }
                context
                    .variables
                    .insert("kb_summary".into(), serde_json::json!(kb_summary.clone()));
            }

            crate::governance::record_audit(
                &self.governance.audit,
                AuditEventType::StateTransition,
                serde_json::json!({"phase": "retrieve", "has_kb": self.kb.is_some()}),
            )
            .await;

            emit_trace(
                &self.event_sink,
                "phase_end",
                serde_json::json!({ "capability": "deep_solve", "phase": "retrieve" }),
            )
            .await;
            deep_events::stage_done(
                &self.event_sink,
                deep_events::DeepSolveStage::Retrieve,
                "Retrieve knowledge",
                "Knowledge summary prepared",
            )
            .await;

            Ok(RuntimeStepResult {
                output: "Knowledge summary prepared".into(),
                structured: Some(serde_json::json!({ "retrieved": true })),
                tool_calls_count: 0,
                session_id: String::new(),
                cost: CostAggregate::default(),
                started_at: None,
                ended_at: None,
            })
        })
    }
}
