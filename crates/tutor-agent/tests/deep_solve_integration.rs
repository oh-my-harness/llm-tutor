// Run with: cargo test --test deep_solve_integration -- --ignored
// Requires provider env vars to be set.
// Example DeepSeek:
//   LLM_PROVIDER=deepseek DEEPSEEK_API_KEY=... LLM_MODEL=deepseek-v4-flash

use std::sync::Arc;

use llm_harness_runtime::budget::BudgetControlAdapter;
use llm_harness_runtime::cost::{PricingProvider, TokenPrice};
use llm_harness_runtime_sandbox_os::OsEnv;
use tutor_agent::governance::GovernanceConfig;
use tutor_agent::{Capability, CapabilityRouter, LlmConfig};

struct NoOpPricing;
impl PricingProvider for NoOpPricing {
    fn price_for(&self, _model: &str, _provider: &str) -> Option<TokenPrice> {
        Some(TokenPrice {
            input_per_mtok: 0.0,
            output_per_mtok: 0.0,
            cache_read_per_mtok: 0.0,
            cache_write_per_mtok: 0.0,
        })
    }
}

fn make_governance() -> GovernanceConfig {
    let budget = Arc::new(BudgetControlAdapter::new(Arc::new(NoOpPricing), 2.0, None));
    GovernanceConfig::new(budget, None, false)
}

#[tokio::test]
#[ignore = "requires LLM provider API key and network"]
async fn deep_solve_end_to_end() {
    let llm = LlmConfig::from_env().expect("LLM provider config required");
    let tmp = tempfile::tempdir().unwrap();
    let env = Arc::new(OsEnv::new(tmp.path()));
    let gov = make_governance();
    let router = CapabilityRouter::new(env, llm, gov);

    let result = router
        .run(
            Capability::DeepSolve,
            "What is the integral of x^2 from 0 to 2?",
        )
        .await;
    assert!(result.is_ok(), "error: {:?}", result.err());
    let answer = result.unwrap();
    assert!(!answer.is_empty());
    println!("Deep Solve answer:\n{answer}");
}

#[tokio::test]
#[ignore = "requires LLM provider API key and network"]
async fn deep_solve_replan_triggers_and_recovers() {
    let llm = LlmConfig::from_env().expect("LLM provider config required");
    let tmp = tempfile::tempdir().unwrap();
    let env = Arc::new(OsEnv::new(tmp.path()));
    let gov = make_governance();
    let router = CapabilityRouter::new(env, llm, gov);

    let result = router
        .run(
            Capability::DeepSolve,
            "Solve x^3 - 6x^2 + 11x - 6 = 0 and verify using code_exec",
        )
        .await;
    assert!(result.is_ok(), "error: {:?}", result.err());
    println!("Answer:\n{}", result.unwrap());
}
