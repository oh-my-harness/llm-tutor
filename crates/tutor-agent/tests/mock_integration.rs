use std::sync::Arc;

use llm_harness_loop::test_utils::{MockLlmClient, MockResponse, NoOpEnv};
use llm_harness_runtime::audit::AuditSink;
use llm_harness_runtime::budget::BudgetControlAdapter;
use llm_harness_runtime::cost::{PricingProvider, TokenPrice};
use llm_harness_runtime_audit_jsonl::JsonlAuditSink;
use llm_harness_types::ExecutionEnv;
use tempfile::TempDir;
use tutor_agent::capability::Capability;
use tutor_agent::governance::GovernanceConfig;
use tutor_agent::{CapabilityRouter, LlmConfig};

struct NoPricing;
impl PricingProvider for NoPricing {
    fn price_for(&self, _model: &str, _provider: &str) -> Option<TokenPrice> {
        Some(TokenPrice {
            input_per_mtok: 0.0,
            output_per_mtok: 0.0,
            cache_read_per_mtok: 0.0,
            cache_write_per_mtok: 0.0,
        })
    }
}

fn make_governance(audit: Option<Arc<dyn AuditSink>>) -> GovernanceConfig {
    let budget = Arc::new(BudgetControlAdapter::new(Arc::new(NoPricing), 100.0, None));
    GovernanceConfig::new(budget, audit, false)
}

fn make_router(responses: Vec<MockResponse>, governance: GovernanceConfig) -> CapabilityRouter {
    let client = Arc::new(MockLlmClient::new(responses));
    let env = Arc::new(NoOpEnv) as Arc<dyn ExecutionEnv>;
    let llm = LlmConfig::anthropic("mock-model", "");
    CapabilityRouter::new(env, llm, governance).with_client(client)
}

#[tokio::test]
async fn smoke_chat_text_only() {
    let responses = vec![MockResponse::text("Hello from mock tutor.")];
    let router = make_router(responses, make_governance(None));
    let answer = router.run(Capability::Chat, "what is 2+2?").await.unwrap();
    assert!(!answer.is_empty());
}

#[tokio::test]
async fn chat_tool_call_then_text() {
    let responses = vec![
        MockResponse::tool_use("use-1", "rag_search", r#"{"query":"Newton"}"#),
        MockResponse::text("Newton's first law: an object at rest stays at rest."),
    ];
    let router = make_router(responses, make_governance(None));
    let answer = router
        .run(Capability::Chat, "explain Newton's first law")
        .await
        .unwrap();
    assert!(answer.contains("Newton"));
}

#[tokio::test]
async fn smoke_deep_solve_one_step() {
    let plan_json =
        r#"{"analysis":"simple addition","steps":[{"id":"s1","goal":"compute 2 plus 2"}]}"#;
    let responses = vec![
        MockResponse::text(plan_json),
        MockResponse::text("FINISH: the answer is 4"),
        MockResponse::text("The final answer is 4."),
    ];
    let router = make_router(responses, make_governance(None));
    let answer = router
        .run(Capability::DeepSolve, "what is 2+2?")
        .await
        .unwrap();
    assert!(!answer.is_empty());
}

#[tokio::test]
async fn audit_captures_deep_solve_state_transitions() {
    let dir = TempDir::new().unwrap();
    let audit_path = dir.path().join("audit.jsonl");
    let sink: Arc<dyn AuditSink> = Arc::new(JsonlAuditSink::new(audit_path.clone()));

    let plan_json = r#"{"analysis":"simple","steps":[{"id":"s1","goal":"compute the value"}]}"#;
    let responses = vec![
        MockResponse::text(plan_json),
        MockResponse::text("FINISH: computed"),
        MockResponse::text("The answer is 42."),
    ];
    let router = make_router(responses, make_governance(Some(sink)));
    router
        .run(Capability::DeepSolve, "what is 6 times 7?")
        .await
        .unwrap();

    let content = std::fs::read_to_string(&audit_path).unwrap();
    assert!(
        content.contains("\"plan\""),
        "audit missing plan phase: {content}"
    );
    assert!(
        content.contains("\"solve_steps\""),
        "audit missing solve_steps phase: {content}"
    );
    assert!(
        content.contains("\"synthesize\""),
        "audit missing synthesize phase: {content}"
    );

    let count = JsonlAuditSink::validate(&audit_path).await.unwrap();
    assert!(count >= 3, "expected at least 3 audit entries, got {count}");
}

#[tokio::test]
async fn code_exec_returns_unsupported() {
    let router = make_router(vec![], make_governance(None));
    let result = router.run(Capability::CodeExec, "run some code").await;
    assert!(result.is_err());
}
