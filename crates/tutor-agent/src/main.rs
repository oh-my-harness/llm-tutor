use std::sync::Arc;

use llm_harness_runtime_sandbox_os::OsEnv;
use tutor_agent::{Capability, CapabilityRouter};

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
    let router = CapabilityRouter::new(env, "claude-haiku-4-5-20251001", &api_key);

    println!("Question: {question}");
    let answer = router.run(Capability::Chat, &question).await?;
    println!("Answer:\n{answer}");

    Ok(())
}
