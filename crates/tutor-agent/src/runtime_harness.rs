use std::sync::{Arc, Mutex};

use llm_harness_agent::{AgentHarness, AgentHarnessOptions, HarnessHooks, ModelInfo, Session};
use llm_harness_loop::{FinalAnswerMode, LlmClient};
use llm_harness_runtime::cost_hook::CostAccumulatorHook;
use llm_harness_types::{
    AfterProviderResponseHook, BeforeToolCallHook, CostAggregate, ExecutionEnv,
    PrepareNextTurnHook, Tool,
};

use crate::error::Result;

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

    let cost = Arc::new(Mutex::new(CostAggregate::default()));
    let accumulator: Arc<dyn AfterProviderResponseHook> =
        Arc::new(CostAccumulatorHook::new(cost.clone(), None));
    hooks.after_provider_response.push(accumulator);

    let mut opts = AgentHarnessOptions::new(config.model.clone());
    opts.model_info = Some(config.model_info);
    opts.tools = config.tools;
    opts.hooks = hooks;
    opts.system_prompt = Some(config.system_prompt);
    opts.final_answer_mode = FinalAnswerMode::tool_with_text_fallback();
    opts.cost = cost;

    match session {
        Some(session) => Ok(AgentHarness::with_session(client, env, session, opts)),
        None => Ok(AgentHarness::new_in_memory(client, env, opts).await),
    }
}
