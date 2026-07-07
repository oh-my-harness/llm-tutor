use std::sync::Arc;

use llm_harness_agent::{AgentHarness, HarnessHooks, ModelInfo, Plugin, Session};
use llm_harness_loop::{FinalAnswerMode, LlmClient};
use llm_harness_runtime::builder::HarnessBuilder;
use llm_harness_types::{BeforeToolCallHook, ExecutionEnv, PrepareNextTurnHook, Tool};

use crate::error::{Result, TutorError};

pub struct RuntimeHarnessConfig {
    pub model: String,
    pub model_info: ModelInfo,
    pub tools: Vec<Arc<dyn Tool>>,
    pub system_prompt: String,
    pub before_tool_call: Vec<Arc<dyn BeforeToolCallHook>>,
    pub prepare_next_turn: Vec<Arc<dyn PrepareNextTurnHook>>,
}

struct HookPlugin {
    before_tool_call: Vec<Arc<dyn BeforeToolCallHook>>,
    prepare_next_turn: Vec<Arc<dyn PrepareNextTurnHook>>,
}

impl Plugin for HookPlugin {
    fn name(&self) -> &str {
        "tutor-runtime-harness-hooks"
    }

    fn register_hooks(&self, hooks: &mut HarnessHooks) {
        hooks
            .before_tool_call
            .extend(self.before_tool_call.iter().cloned());
        hooks
            .prepare_next_turn
            .extend(self.prepare_next_turn.iter().cloned());
    }
}

pub async fn build_runtime_harness(
    client: Arc<dyn LlmClient>,
    env: Arc<dyn ExecutionEnv>,
    session: Option<Session>,
    config: RuntimeHarnessConfig,
) -> Result<AgentHarness> {
    let mut builder = HarnessBuilder::new(config.model.clone())
        .provider(config.model.clone(), client)
        .model_info(Some(config.model_info))
        .system_prompt(Some(config.system_prompt))
        .final_answer_mode(FinalAnswerMode::tool_with_text_fallback());

    for tool in config.tools {
        builder = builder.tool(tool);
    }
    if !config.before_tool_call.is_empty() || !config.prepare_next_turn.is_empty() {
        let hooks = HookPlugin {
            before_tool_call: config.before_tool_call,
            prepare_next_turn: config.prepare_next_turn,
        };
        builder = builder.install(&hooks);
    }

    match session {
        Some(session) => builder
            .build_with_session(env, session)
            .map_err(|err| TutorError::Internal(format!("failed to build runtime harness: {err}"))),
        None => builder
            .build(env)
            .await
            .map_err(|err| TutorError::Internal(format!("failed to build runtime harness: {err}"))),
    }
}
