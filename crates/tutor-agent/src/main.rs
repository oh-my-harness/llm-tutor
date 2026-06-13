use std::sync::Arc;

use llm_harness_runtime::budget::BudgetControlAdapter;
use llm_harness_runtime::cost::{PricingProvider, TokenPrice};
use llm_harness_runtime_audit_jsonl::JsonlAuditSink;
use llm_harness_runtime_sandbox_os::OsEnv;
use tutor_agent::governance::GovernanceConfig;
use tutor_agent::{Capability, CapabilityRouter};

/// Zero-cost pricing provider for v0.1 development.
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let question = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "What is integration by parts?".into());

    // EnvAuthHook reads ANTHROPIC_API_KEY at harness call time.
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("Error: ANTHROPIC_API_KEY not set");
        std::process::exit(1);
    }

    let env = Arc::new(OsEnv::new(std::env::current_dir()?));
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY environment variable required");

    // Governance: $2.00 session budget + JSONL audit log
    let budget = Arc::new(BudgetControlAdapter::new(Arc::new(NoOpPricing), 2.00, None));

    let audit_path = std::env::temp_dir().join("tutor_audit.jsonl");
    let audit = Arc::new(JsonlAuditSink::new(&audit_path));

    let governance = GovernanceConfig::new(budget, Some(audit), false);

    let router = CapabilityRouter::new(env, "claude-haiku-4-5-20251001", api_key, governance);

    println!("Question: {question}");
    let answer = router.run(Capability::Chat, &question).await?;
    println!("Answer:\n{answer}");

    Ok(())
}
