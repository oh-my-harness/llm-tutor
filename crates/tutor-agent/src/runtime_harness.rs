use std::sync::Arc;

use llm_harness_agent::{AgentHarness, HarnessHooks, ModelInfo, Plugin, Session};
use llm_harness_loop::LlmClient;
use llm_harness_runtime::builder::HarnessBuilder;
use llm_harness_types::{BeforeToolCallHook, ExecutionEnv, Tool};

use crate::error::{Result, TutorError};

pub struct RuntimeHarnessConfig {
    pub model: String,
    pub model_info: ModelInfo,
    pub tools: Vec<Arc<dyn Tool>>,
    pub system_prompt: String,
    pub before_tool_call: Vec<Arc<dyn BeforeToolCallHook>>,
}

struct HookPlugin {
    before_tool_call: Vec<Arc<dyn BeforeToolCallHook>>,
}

impl Plugin for HookPlugin {
    fn name(&self) -> &str {
        "tutor-runtime-harness-hooks"
    }

    fn register_hooks(&self, hooks: &mut HarnessHooks) {
        hooks
            .before_tool_call
            .extend(self.before_tool_call.iter().cloned());
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
        .system_prompt(Some(config.system_prompt));

    for tool in config.tools {
        builder = builder.tool(tool);
    }
    if !config.before_tool_call.is_empty() {
        let hooks = HookPlugin {
            before_tool_call: config.before_tool_call,
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
