// Run with: cargo test --test deep_solve_integration -- --ignored
// Requires ANTHROPIC_API_KEY to be set.

use std::sync::Arc;

use llm_harness_runtime_sandbox_os::OsEnv;
use tutor_agent::{Capability, CapabilityRouter};

#[tokio::test]
#[ignore = "requires ANTHROPIC_API_KEY and network"]
async fn deep_solve_end_to_end() {
    let api_key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY required");
    let tmp = tempfile::tempdir().unwrap();
    let env = Arc::new(OsEnv::new(tmp.path()));
    let router = CapabilityRouter::new(env, "claude-haiku-4-5-20251001", api_key);

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
#[ignore = "requires ANTHROPIC_API_KEY and network"]
async fn deep_solve_replan_triggers_and_recovers() {
    let api_key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY required");
    let tmp = tempfile::tempdir().unwrap();
    let env = Arc::new(OsEnv::new(tmp.path()));
    let router = CapabilityRouter::new(env, "claude-haiku-4-5-20251001", api_key);

    let result = router
        .run(
            Capability::DeepSolve,
            "Solve x^3 - 6x^2 + 11x - 6 = 0 and verify using code_exec",
        )
        .await;
    assert!(result.is_ok(), "error: {:?}", result.err());
    println!("Answer:\n{}", result.unwrap());
}
