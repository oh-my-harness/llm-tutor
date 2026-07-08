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

pub async fn build_runtime_harness(
    client: Arc<dyn LlmClient>,
    env: Arc<dyn ExecutionEnv>,
    session: Option<Session>,
    config: RuntimeHarnessConfig,
) -> Result<AgentHarness> {
    let mut hooks = HarnessHooks::none();
    hooks.before_tool_call = config.before_tool_call;
    hooks.prepare_next_turn = config.prepare_next_turn;

    let hook_plugin = HookPlugin { hooks };
    let mut builder = HarnessBuilder::new(config.model.clone())
        .provider(config.model.clone(), client)
        .install(&hook_plugin)
        .system_prompt(Some(config.system_prompt))
        .model_info(Some(config.model_info))
        .final_answer_mode(FinalAnswerMode::tool_with_text_fallback());

    for tool in config.tools {
        builder = builder.tool(tool);
    }

    match session {
        Some(session) => builder
            .build_with_session(env, session)
            .map_err(|err| TutorError::Internal(err.to_string())),
        None => builder
            .build(env)
            .await
            .map_err(|err| TutorError::Internal(err.to_string())),
    }
}

struct HookPlugin {
    hooks: HarnessHooks,
}

impl Plugin for HookPlugin {
    fn name(&self) -> &str {
        "tutor-runtime-hooks"
    }

    fn register_hooks(&self, target: &mut HarnessHooks) {
        target
            .before_tool_call
            .extend(self.hooks.before_tool_call.clone());
        target
            .prepare_next_turn
            .extend(self.hooks.prepare_next_turn.clone());
    }
}
