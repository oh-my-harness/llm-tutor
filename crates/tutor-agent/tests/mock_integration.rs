use std::sync::Arc;
use std::sync::Mutex;

use futures::future::BoxFuture;
use llm_adapter::types::{ContentKind, StopReason, StreamEvent, Usage};
use llm_harness_agent::{JsonlSessionRepo, Session, SessionRepo, session::CreateSessionOptions};
use llm_harness_loop::{
    LlmError,
    test_utils::{MockLlmClient, MockResponse, NoOpEnv},
};
use llm_harness_runtime::observability::audit::AuditSink;
use llm_harness_runtime_audit_jsonl::JsonlAuditSink;
use llm_harness_runtime_sandbox_os::OsEnv;
use llm_harness_types::{AgentMessage, AssistantMessageKind, ExecutionEnv};
use tempfile::TempDir;
use tutor_agent::capability::Capability;
use tutor_agent::chat::{assistant_message, user_message};
use tutor_agent::event_sink::EventSink;
use tutor_agent::governance::GovernanceConfig;
use tutor_agent::{CapabilityRouter, LlmConfig};

fn make_governance(audit: Option<Arc<dyn AuditSink>>) -> GovernanceConfig {
    GovernanceConfig::new(100.0, audit, false)
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

fn progress_text_response(text: &str) -> MockResponse {
    MockResponse {
        model: "mock-model".into(),
        stream_error: None,
        events: vec![
            Ok(StreamEvent::ContentStart {
                index: 0,
                kind: ContentKind::Text,
            }),
            Ok(StreamEvent::TextDelta {
                index: 0,
                text: text.into(),
            }),
            Ok(StreamEvent::ContentStop {
                index: 0,
                signature: None,
            }),
            Ok(StreamEvent::MessageStop {
                stop_reason: StopReason::ToolUse,
                usage: Usage::default(),
            }),
        ],
    }
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
        stream_error: None,
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
async fn chat_can_call_read_memory_then_text() {
    let responses = vec![
        MockResponse::tool_use("use-memory", "read_memory", r#"{"scope":"profile"}"#),
        MockResponse::text("I will adapt the next explanation to your profile."),
    ];
    let router = make_router(responses, make_governance(None));
    let answer = router
        .run(Capability::Chat, "review this based on my profile")
        .await
        .unwrap();
    assert!(answer.contains("profile"));
}

#[tokio::test]
async fn chat_returns_runtime_final_answer_not_progress_text() {
    let sink = Arc::new(TraceRecorder::default());
    let router = make_router(
        vec![
            progress_text_response("checking context first"),
            MockResponse::text("final answer only"),
        ],
        make_governance(None),
    )
    .with_event_sink(sink.clone());

    let answer = router
        .run(Capability::Chat, "answer after progress")
        .await
        .unwrap();

    assert_eq!(answer, "final answer only");
    let events = sink.events();
    assert!(
        events.iter().any(|(kind, data)| {
            kind == "assistant_progress"
                && data["summary"]
                    .as_str()
                    .is_some_and(|text| text.contains("checking context first"))
        }),
        "missing runtime progress trace: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|(kind, data)| { kind == "final_answer" && data["capability"] == "chat" }),
        "missing runtime final answer trace: {events:?}"
    );
}

#[tokio::test]
async fn smoke_deep_solve_one_step() {
    let plan_json =
        r#"{"analysis":"simple addition","steps":[{"id":"s1","goal":"compute 2 plus 2"}]}"#;
    let responses = vec![
        MockResponse::text(plan_json),
        MockResponse::tool_use(
            "submit-solve",
            "submit_step_result",
            r#"{"result":{"route":"finish","summary":"the answer is 4"}}"#,
        ),
        MockResponse::text("Solved: the answer is 4."),
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
        MockResponse::tool_use(
            "submit-solve",
            "submit_step_result",
            r#"{"result":{"route":"finish","summary":"computed"}}"#,
        ),
        MockResponse::text("Solved: computed."),
        MockResponse::text("The answer is 42."),
    ];
    let router = make_router(responses, make_governance(Some(sink)));
    router
        .run(Capability::DeepSolve, "what is 6 times 7?")
        .await
        .unwrap();

    let content = std::fs::read_to_string(&audit_path).unwrap();
    assert!(
        content.contains("\"retrieve\""),
        "audit missing retrieve phase: {content}"
    );
    assert!(
        content.contains("\"has_kb\""),
        "audit missing runtime retrieve metadata: {content}"
    );

    let count = JsonlAuditSink::validate(&audit_path).await.unwrap();
    assert!(count >= 1, "expected at least 1 audit entry, got {count}");
}

#[tokio::test]
async fn deep_solve_emits_structured_ux_events() {
    let sink = Arc::new(TraceRecorder::default());
    let plan_json =
        r#"{"analysis":"simple addition","steps":[{"id":"s1","goal":"compute 2 plus 2"}]}"#;
    let responses = vec![
        MockResponse::text(plan_json),
        MockResponse::tool_use(
            "submit-solve",
            "submit_step_result",
            r#"{"result":{"route":"finish","summary":"the answer is 4"}}"#,
        ),
        MockResponse::text("Solved: the answer is 4."),
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
            kind == "deep_solve_stage_done"
                && data["stage"] == "solve"
                && data["summary"]
                    .as_str()
                    .is_some_and(|text| text.contains("4"))
        }),
        "missing runtime solve stage done: {events:?}"
    );
    assert!(
        events.iter().any(|(kind, data)| {
            kind == "deep_solve_final"
                && data["stage"] == "synthesize"
                && data["summary"]
                    .as_str()
                    .is_some_and(|text| text.contains("4"))
        }),
        "missing final event: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|(kind, data)| { kind == "runtime_usage" && data["capability"] == "deep_solve" }),
        "missing deep solve runtime usage trace: {events:?}"
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
async fn code_exec_returns_runtime_final_answer_not_progress_text() {
    let dir = TempDir::new().unwrap();
    let sink = Arc::new(TraceRecorder::default());
    let router = make_router_with_env(
        vec![
            progress_text_response("checking code execution plan"),
            MockResponse::text("final code answer"),
        ],
        make_governance(None),
        Arc::new(OsEnv::new(dir.path())) as Arc<dyn ExecutionEnv>,
    )
    .with_event_sink(sink.clone());

    let answer = router
        .run(Capability::CodeExec, "answer after code progress")
        .await
        .unwrap();

    assert_eq!(answer, "final code answer");
    let events = sink.events();
    assert!(
        events.iter().any(|(kind, data)| {
            kind == "assistant_progress"
                && data["summary"]
                    .as_str()
                    .is_some_and(|text| text.contains("checking code execution plan"))
        }),
        "missing runtime progress trace: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|(kind, data)| { kind == "final_answer" && data["capability"] == "code_exec" }),
        "missing runtime final answer trace: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|(kind, data)| { kind == "runtime_usage" && data["capability"] == "code_exec" }),
        "missing runtime usage trace: {events:?}"
    );
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
    assert!(
        events
            .iter()
            .any(|(kind, data)| { kind == "runtime_usage" && data["capability"] == "chat" }),
        "missing chat runtime_usage trace: {events:?}"
    );
}

#[tokio::test]
async fn research_emits_research_trace_events() {
    let sink = Arc::new(TraceRecorder::default());
    let router = make_router(
        vec![MockResponse::text(
            "# Report\n\n## Summary\n\nShort summary.\n\n## Key Findings\n\n- Finding.\n\n## Analysis\n\nAnalysis.\n\n## Limitations\n\nLimited.\n\n## Follow-up Questions\n\n- Next?\n\n## Sources\n\n[1] Mock source",
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

#[tokio::test]
async fn research_clarification_uses_chat_fallback_without_report_event() {
    let sink = Arc::new(TraceRecorder::default());
    let router = make_router(
        vec![MockResponse::text(
            "Sure. What scope, time range, and output format should I use?",
        )],
        make_governance(None),
    )
    .with_event_sink(sink.clone());

    let answer = router
        .run(
            Capability::Research,
            "hi, can you help me research something?",
        )
        .await
        .unwrap();

    assert!(answer.contains("scope"));
    let events = sink.events();
    assert!(
        events
            .iter()
            .any(|(kind, data)| { kind == "final_answer" && data["capability"] == "research" }),
        "missing research final answer trace: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|(kind, _)| { kind == "research_report_done" }),
        "clarification should not be reported as a research report: {events:?}"
    );
    assert!(
        !events
            .iter()
            .any(|(kind, data)| { kind == "tool_call" && data["tool"] == "web_search" }),
        "clarification should not call web_search: {events:?}"
    );
}

#[tokio::test]
async fn research_explicit_start_enters_search_path() {
    let sink = Arc::new(TraceRecorder::default());
    let router = make_router(
        vec![
            MockResponse::tool_use(
                "submit-search",
                "submit_step_result",
                r#"{"result":{"queries":["agent research workflow"],"source_candidates":[{"title":"Mock","url":"https://example.test","snippet":"Mock source"}],"failures":[]}}"#,
            ),
            MockResponse::tool_use(
                "submit-read",
                "submit_step_result",
                r#"{"result":{"sources":[{"title":"Mock","url":"https://example.test","summary":"Mock source summary","used_for":"workflow architecture"}],"failures":[]}}"#,
            ),
            MockResponse::tool_use(
                "submit-check",
                "submit_step_result",
                r#"{"result":{"verdict":"pass","issues":[]}}"#,
            ),
            MockResponse::tool_use(
                "submit-report",
                "submit_step_result",
                r##"{"result":{"markdown":"# Report\n\n## Summary\n\nSearched the topic.\n\n## Key Findings\n\n- Finding. [1]\n\n## Analysis\n\nAnalysis.\n\n## Limitations\n\nLimited.\n\n## Follow-up Questions\n\n- Next?\n\n## Sources\n\n[1] Mock - https://example.test","sources":[{"title":"Mock","url":"https://example.test"}]}}"##,
            ),
        ],
        make_governance(None),
    )
    .with_event_sink(sink.clone());

    let answer = router
        .run(
            Capability::Research,
            "Start the detailed research workflow for agent research workflow.",
        )
        .await
        .unwrap();

    assert!(answer.contains("Report"));
    let events = sink.events();
    assert!(
        events.iter().any(|(kind, data)| {
            kind == "workflow_validated"
                && data["capability"] == "research"
                && data["workflow"] == "tutor.research"
        }),
        "explicit research start should enter runtime workflow: {events:?}"
    );
    assert!(
        events.iter().any(|(kind, data)| {
            kind == "research_stage_start"
                && data["capability"] == "research"
                && data["stage"] == "search"
        }),
        "explicit research start should emit search stage: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|(kind, _)| { kind == "research_report_done" }),
        "explicit research start should emit report done: {events:?}"
    );
}

#[tokio::test]
async fn research_chinese_confirmation_after_plan_enters_workflow() {
    let dir = TempDir::new().unwrap();
    let repo = JsonlSessionRepo::new(dir.path().join("sessions"));
    let storage = repo.create(CreateSessionOptions::default()).await.unwrap();
    let session = Session::new(storage);
    session
        .append_message(user_message("帮我调研一下transformer架构，我想学习"))
        .await
        .unwrap();
    session
        .append_message(assistant_message(
            "这是调研计划。确认后我就启动详细研究工作流程。",
        ))
        .await
        .unwrap();

    let sink = Arc::new(TraceRecorder::default());
    let router = make_router(
        vec![
            MockResponse::tool_use(
                "submit-search",
                "submit_step_result",
                r#"{"result":{"queries":["Transformer architecture"],"source_candidates":[{"title":"Mock","url":"https://example.test","snippet":"Mock source"}],"failures":[]}}"#,
            ),
            MockResponse::tool_use(
                "submit-read",
                "submit_step_result",
                r#"{"result":{"sources":[{"title":"Mock","url":"https://example.test","summary":"Mock source summary","used_for":"Transformer architecture"}],"failures":[]}}"#,
            ),
            MockResponse::tool_use(
                "submit-check",
                "submit_step_result",
                r#"{"result":{"verdict":"pass","issues":[]}}"#,
            ),
            MockResponse::tool_use(
                "submit-report",
                "submit_step_result",
                r##"{"result":{"markdown":"# Transformer Report\n\n## Summary\n\nConfirmed Chinese request.\n\n## Key Findings\n\n- Finding. [1]\n\n## Analysis\n\nAnalysis.\n\n## Limitations\n\nLimited.\n\n## Follow-up Questions\n\n- Next?\n\n## Sources\n\n[1] Mock - https://example.test","sources":[{"title":"Mock","url":"https://example.test"}]}}"##,
            ),
        ],
        make_governance(None),
    )
    .with_event_sink(sink.clone())
    .with_workflow_root(dir.path().join("workflow-sessions"));

    let answer = router
        .run_with_session(Capability::Research, session, "可以")
        .await
        .unwrap();

    assert!(answer.contains("Transformer Report"));
    let events = sink.events();
    assert!(
        events.iter().any(|(kind, data)| {
            kind == "workflow_validated"
                && data["capability"] == "research"
                && data["workflow"] == "tutor.research"
        }),
        "Chinese confirmation should enter runtime workflow: {events:?}"
    );
}

#[tokio::test]
async fn research_workflow_persists_final_report_to_session() {
    let dir = TempDir::new().unwrap();
    let repo = JsonlSessionRepo::new(dir.path().join("sessions"));
    let storage = repo.create(CreateSessionOptions::default()).await.unwrap();
    let session = Session::new(storage.clone());
    let inspect_session = Session::new(storage);
    let router = make_router(
        vec![
            MockResponse::tool_use(
                "submit-search",
                "submit_step_result",
                r#"{"result":{"queries":["agent research workflow"],"source_candidates":[{"title":"Mock","url":"https://example.test","snippet":"Mock source"}],"failures":[]}}"#,
            ),
            MockResponse::tool_use(
                "submit-read",
                "submit_step_result",
                r#"{"result":{"sources":[{"title":"Mock","url":"https://example.test","summary":"Mock source summary","used_for":"workflow architecture"}],"failures":[]}}"#,
            ),
            MockResponse::tool_use(
                "submit-check",
                "submit_step_result",
                r#"{"result":{"verdict":"pass","issues":[]}}"#,
            ),
            MockResponse::tool_use(
                "submit-report",
                "submit_step_result",
                r##"{"result":{"markdown":"# Report\n\n## Summary\n\nPersisted report.\n\n## Key Findings\n\n- Finding. [1]\n\n## Analysis\n\nAnalysis.\n\n## Limitations\n\nLimited.\n\n## Follow-up Questions\n\n- Next?\n\n## Sources\n\n[1] Mock - https://example.test","sources":[{"title":"Mock","url":"https://example.test"}]}}"##,
            ),
        ],
        make_governance(None),
    )
    .with_workflow_root(dir.path().join("workflow-sessions"));

    let answer = router
        .run_with_session(
            Capability::Research,
            session,
            "Start the detailed research workflow for agent research workflow.",
        )
        .await
        .unwrap();
    assert!(answer.contains("Persisted report"));

    let context = inspect_session.build_context().await.unwrap();
    assert!(
        context.messages.iter().any(|message| matches!(
            message,
            AgentMessage::Assistant(assistant)
                if assistant.kind == AssistantMessageKind::FinalAnswer
                    && assistant.text_content().contains("Persisted report")
        )),
        "research workflow report should be stored as FinalAnswer: {:?}",
        context.messages
    );
}

#[tokio::test]
async fn research_workflow_fails_clearly_after_citation_repair_attempt() {
    let router = make_router(
        vec![
            MockResponse::tool_use(
                "submit-search-1",
                "submit_step_result",
                r#"{"result":{"queries":["agent research workflow"],"source_candidates":[],"failures":["search unavailable"]}}"#,
            ),
            MockResponse::tool_use(
                "submit-read-1",
                "submit_step_result",
                r#"{"result":{"sources":[],"failures":["no sources to fetch"]}}"#,
            ),
            MockResponse::tool_use(
                "submit-check-1",
                "submit_step_result",
                r#"{"result":{"verdict":"fail","issues":["no verified sources"]}}"#,
            ),
            MockResponse::tool_use(
                "submit-search-2",
                "submit_step_result",
                r#"{"result":{"queries":["agent research workflow retry"],"source_candidates":[],"failures":["search still unavailable"]}}"#,
            ),
            MockResponse::tool_use(
                "submit-read-2",
                "submit_step_result",
                r#"{"result":{"sources":[],"failures":["fetch still unavailable"]}}"#,
            ),
            MockResponse::tool_use(
                "submit-check-2",
                "submit_step_result",
                r#"{"result":{"verdict":"fail","issues":["citations still unverified"]}}"#,
            ),
        ],
        make_governance(None),
    );

    let err = router
        .run(
            Capability::Research,
            "Start the detailed research workflow for agent research workflow.",
        )
        .await
        .unwrap_err();

    assert!(
        err.to_string()
            .contains("research citation check failed after repair attempt"),
        "research workflow failure should explain the failed repair attempt: {err}"
    );
}
