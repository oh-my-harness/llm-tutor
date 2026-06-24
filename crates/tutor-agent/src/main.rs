use std::sync::Arc;

use llm_harness_runtime::budget::BudgetControlAdapter;
use llm_harness_runtime::cost::{PricingProvider, TokenPrice};
use llm_harness_runtime_audit_jsonl::JsonlAuditSink;
use llm_harness_runtime_sandbox_os::OsEnv;
use tutor_agent::governance::GovernanceConfig;
use tutor_agent::{Capability, CapabilityRouter, LlmConfig};

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
    let (capability, question) = parse_args()?;

    let env = Arc::new(OsEnv::new(std::env::current_dir()?));
    let llm = match LlmConfig::from_env() {
        Ok(config) => config,
        Err(err) => {
            eprintln!("Error: {err}");
            std::process::exit(1);
        }
    };

    // Governance: $2.00 session budget + JSONL audit log
    let budget = Arc::new(BudgetControlAdapter::new(Arc::new(NoOpPricing), 2.00, None));

    let audit_path = std::env::temp_dir().join("tutor_audit.jsonl");
    let audit = Arc::new(JsonlAuditSink::new(&audit_path));

    let governance = GovernanceConfig::new(budget, Some(audit), false);

    let router = CapabilityRouter::new(env, llm, governance);

    println!("Question: {question}");
    let answer = router.run(capability, &question).await?;
    println!("Answer:\n{answer}");

    Ok(())
}

fn parse_args() -> anyhow::Result<(Capability, String)> {
    let mut capability = Capability::Chat;
    let mut question_parts = Vec::new();
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--capability" | "-c" => {
                let value = args
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("missing value for {arg}"))?;
                capability = value.parse()?;
            }
            "--help" | "-h" => {
                println!(
                    "Usage: tutor-agent [--capability chat|deep_solve|code_exec|quiz] <question>"
                );
                std::process::exit(0);
            }
            _ => question_parts.push(arg),
        }
    }

    let question = if question_parts.is_empty() {
        "What is integration by parts?".into()
    } else {
        question_parts.join(" ")
    };

    Ok((capability, question))
}
