use std::sync::Arc;
use std::sync::Mutex;

use futures::future::BoxFuture;
use llm_harness_loop::{
    LlmError,
    test_utils::{MockLlmClient, MockResponse, NoOpEnv},
};
use llm_harness_runtime::audit::AuditSink;
use llm_harness_runtime::budget::BudgetControlAdapter;
use llm_harness_runtime::cost::{PricingProvider, TokenPrice};
use llm_harness_runtime_audit_jsonl::JsonlAuditSink;
use llm_harness_runtime_sandbox_os::OsEnv;
use llm_harness_types::ExecutionEnv;
use tempfile::TempDir;
use tutor_agent::capability::Capability;
use tutor_agent::event_sink::EventSink;
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

fn make_router_with_env(
    responses: Vec<MockResponse>,
    governance: GovernanceConfig,
    env: Arc<dyn ExecutionEnv>,
) -> CapabilityRouter {
    let client = Arc::new(MockLlmClient::new(responses));
    let llm = LlmConfig::anthropic("mock-model", "");
    CapabilityRouter::new(env, llm, governance).with_client(client)
}

#[derive(Default)]
struct TraceRecorder {
    events: Mutex<Vec<(String, serde_json::Value)>>,
}

impl TraceRecorder {
    fn events(&self) -> Vec<(String, serde_json::Value)> {
        self.events.lock().unwrap().clone()
    }
}

impl EventSink for TraceRecorder {
    fn trace(&self, kind: String, data: serde_json::Value) -> BoxFuture<'static, ()> {
        self.events.lock().unwrap().push((kind, data));
        Box::pin(async {})
    }
}

#[tokio::test]
async fn smoke_chat_text_only() {
    let responses = vec![MockResponse::text("Hello from mock tutor.")];
    let router = make_router(responses, make_governance(None));
    let answer = router.run(Capability::Chat, "what is 2+2?").await.unwrap();
    assert!(!answer.is_empty());
}

#[tokio::test]
async fn chat_returns_error_instead_of_no_response() {
    let responses = vec![MockResponse {
        events: vec![Err(LlmError::InvalidRequest("bad request".into()))],
        model: "mock-model".into(),
    }];
    let router = make_router(responses, make_governance(None));

    let err = router
        .run(Capability::Chat, "trigger error")
        .await
        .unwrap_err();

    assert!(
        err.to_string().contains("bad request"),
        "expected provider error to be surfaced, got {err}"
    );
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
async fn deep_solve_emits_structured_ux_events() {
    let sink = Arc::new(TraceRecorder::default());
    let plan_json =
        r#"{"analysis":"simple addition","steps":[{"id":"s1","goal":"compute 2 plus 2"}]}"#;
    let responses = vec![
        MockResponse::text(plan_json),
        MockResponse::text("FINISH: the answer is 4"),
        MockResponse::text("The final answer is 4."),
    ];
    let router = make_router(responses, make_governance(None)).with_event_sink(sink.clone());

    router
        .run(Capability::DeepSolve, "what is 2+2?")
        .await
        .unwrap();

    let events = sink.events();
    assert!(
        events.iter().any(|(kind, data)| {
            kind == "deep_solve_stage_start"
                && data["capability"] == "deep_solve"
                && data["stage"] == "plan"
        }),
        "missing plan stage start: {events:?}"
    );
    assert!(
        events.iter().any(|(kind, data)| {
            kind == "deep_solve_plan"
                && data["analysis"] == "simple addition"
                && data["steps"]
                    .as_array()
                    .is_some_and(|steps| steps.len() == 1)
        }),
        "missing structured plan: {events:?}"
    );
    assert!(
        events.iter().any(|(kind, data)| {
            kind == "deep_solve_step_done"
                && data["stage"] == "solve"
                && data["step_id"] == "s1"
                && data["summary"]
                    .as_str()
                    .is_some_and(|text| text.contains("4"))
        }),
        "missing step done event: {events:?}"
    );
    assert!(
        events.iter().any(|(kind, data)| {
            kind == "deep_solve_final"
                && data["stage"] == "synthesize"
                && data["summary"]
                    .as_str()
                    .is_some_and(|text| text.contains("final answer"))
        }),
        "missing final event: {events:?}"
    );
}

#[tokio::test]
async fn code_exec_runs_tool_and_explains_result() {
    let dir = TempDir::new().unwrap();
    let responses = vec![
        MockResponse::tool_use(
            "exec-1",
            "code_exec",
            r#"{"language":"python","code":"print('hello code exec')"}"#,
        ),
        MockResponse::text("The script printed hello code exec."),
    ];
    let router = make_router_with_env(
        responses,
        make_governance(None),
        Arc::new(OsEnv::new(dir.path())) as Arc<dyn ExecutionEnv>,
    );
    let answer = router
        .run(Capability::CodeExec, "run python that prints hello")
        .await
        .unwrap();
    assert!(answer.contains("hello code exec"));
}

#[tokio::test]
async fn chat_emits_trace_events() {
    let sink = Arc::new(TraceRecorder::default());
    let router = make_router(
        vec![MockResponse::text("traced answer")],
        make_governance(None),
    )
    .with_event_sink(sink.clone());

    router.run(Capability::Chat, "trace this").await.unwrap();

    let events = sink.events();
    assert!(
        events
            .iter()
            .any(|(kind, data)| { kind == "phase_start" && data["capability"] == "chat" }),
        "missing chat phase_start trace: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|(kind, data)| { kind == "phase_end" && data["capability"] == "chat" }),
        "missing chat phase_end trace: {events:?}"
    );
}

#[tokio::test]
async fn research_emits_research_trace_events() {
    let sink = Arc::new(TraceRecorder::default());
    let router = make_router(
        vec![MockResponse::text(
            "# Report\n\n## Sources\n\n[1] Mock source",
        )],
        make_governance(None),
    )
    .with_event_sink(sink.clone());

    let answer = router
        .run(Capability::Research, "research a topic")
        .await
        .unwrap();
    assert!(answer.contains("Report"));

    let events = sink.events();
    assert!(
        events.iter().any(|(kind, data)| {
            kind == "research_stage_start" && data["capability"] == "research"
        }),
        "missing research stage event: {events:?}"
    );
    assert!(
        events.iter().any(|(kind, data)| {
            kind == "research_report_done" && data["capability"] == "research"
        }),
        "missing research report event: {events:?}"
    );
}
