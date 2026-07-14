use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use futures::future::BoxFuture;
use llm_harness_types::{ContentBlock, Tool, ToolContext, ToolError, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::memory_store::{MemoryEvent, MemoryStore};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryEvidenceActivity {
    pub stage: String,
    pub tool: String,
    pub summary: String,
    pub refs: Vec<String>,
}

#[derive(Clone, Default)]
pub struct MemoryEvidenceTracker {
    read_refs: Arc<Mutex<BTreeSet<String>>>,
    activities: Arc<Mutex<Vec<MemoryEvidenceActivity>>>,
    activity_sender: Option<tokio::sync::mpsc::UnboundedSender<MemoryEvidenceActivity>>,
}

impl MemoryEvidenceTracker {
    pub fn with_activity_sender(
        activity_sender: tokio::sync::mpsc::UnboundedSender<MemoryEvidenceActivity>,
    ) -> Self {
        Self {
            activity_sender: Some(activity_sender),
            ..Self::default()
        }
    }

    pub fn read_refs(&self) -> Vec<String> {
        self.read_refs
            .lock()
            .map(|refs| refs.iter().cloned().collect())
            .unwrap_or_default()
    }

    fn record(&self, stage: &str, tool: &str, summary: String, refs: Vec<String>) {
        if let Ok(mut read_refs) = self.read_refs.lock() {
            read_refs.extend(refs.iter().cloned());
        }
        let activity = MemoryEvidenceActivity {
            stage: stage.into(),
            tool: tool.into(),
            summary,
            refs,
        };
        if let Ok(mut activities) = self.activities.lock() {
            activities.push(activity.clone());
        }
        if let Some(sender) = &self.activity_sender {
            let _ = sender.send(activity);
        }
    }
}

#[derive(Clone)]
pub struct ListMemoryEventsTool {
    store: Arc<MemoryStore>,
    tracker: MemoryEvidenceTracker,
}

#[derive(Clone)]
pub struct SearchMemoryEventsTool {
    store: Arc<MemoryStore>,
    tracker: MemoryEvidenceTracker,
}

#[derive(Clone)]
pub struct ReadMemoryEventTool {
    store: Arc<MemoryStore>,
    tracker: MemoryEvidenceTracker,
}

#[derive(Clone)]
pub struct ReadMemoryContextTool {
    store: Arc<MemoryStore>,
    tracker: MemoryEvidenceTracker,
}

#[derive(Clone)]
pub struct ReadMemorySourceTool {
    store: Arc<MemoryStore>,
    tracker: MemoryEvidenceTracker,
}

macro_rules! tool_constructor {
    ($tool:ident) => {
        impl $tool {
            pub fn new(store: Arc<MemoryStore>, tracker: MemoryEvidenceTracker) -> Self {
                Self { store, tracker }
            }
        }
    };
}

tool_constructor!(ListMemoryEventsTool);
tool_constructor!(SearchMemoryEventsTool);
tool_constructor!(ReadMemoryEventTool);
tool_constructor!(ReadMemoryContextTool);
tool_constructor!(ReadMemorySourceTool);

impl Tool for ListMemoryEventsTool {
    fn name(&self) -> &str {
        "list_memory_events"
    }

    fn description(&self) -> &str {
        "List a bounded page of L1 learner activity summaries. Start with the target surface before expanding to other surfaces. Use read_memory_event for full evidence."
    }

    fn parameters_schema(&self) -> &Value {
        static SCHEMA: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
        SCHEMA.get_or_init(|| event_page_schema(false))
    }

    fn execute<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        Box::pin(async move {
            let surface = optional_string(&args, "surface");
            let session_id = optional_string(&args, "session_id");
            let cursor = optional_string(&args, "cursor");
            let limit = args["limit"].as_u64().unwrap_or(20) as usize;
            let page = self
                .store
                .query_events(surface, None, session_id, cursor, limit)
                .map_err(|err| ToolError::Execution(err.to_string()))?;
            self.tracker.record(
                "discovering_sources",
                self.name(),
                format!(
                    "Listed {} of {} matching {} L1 events",
                    page.events.len(),
                    page.total,
                    surface.unwrap_or("all-surface")
                ),
                Vec::new(),
            );
            Ok(json_tool_result(json!({
                "events": event_summaries(&page.events),
                "next_cursor": page.next_cursor,
                "total": page.total,
            })))
        })
    }
}

impl Tool for SearchMemoryEventsTool {
    fn name(&self) -> &str {
        "search_memory_events"
    }

    fn description(&self) -> &str {
        "Search L1 learner activity summaries by text, surface, or session. Search results are candidates; call read_memory_event before citing one."
    }

    fn parameters_schema(&self) -> &Value {
        static SCHEMA: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
        SCHEMA.get_or_init(|| event_page_schema(true))
    }

    fn execute<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        Box::pin(async move {
            let query = optional_string(&args, "query")
                .filter(|value| !value.is_empty())
                .ok_or_else(|| ToolError::InvalidArguments("query is required".into()))?;
            let surface = optional_string(&args, "surface");
            let session_id = optional_string(&args, "session_id");
            let cursor = optional_string(&args, "cursor");
            let limit = args["limit"].as_u64().unwrap_or(20) as usize;
            let page = self
                .store
                .query_events(surface, Some(query), session_id, cursor, limit)
                .map_err(|err| ToolError::Execution(err.to_string()))?;
            self.tracker.record(
                "discovering_sources",
                self.name(),
                format!(
                    "Found {} {} L1 event candidates for `{query}`",
                    page.total,
                    surface.unwrap_or("all-surface")
                ),
                Vec::new(),
            );
            Ok(json_tool_result(json!({
                "events": event_summaries(&page.events),
                "next_cursor": page.next_cursor,
                "total": page.total,
            })))
        })
    }
}

impl Tool for ReadMemoryEventTool {
    fn name(&self) -> &str {
        "read_memory_event"
    }

    fn description(&self) -> &str {
        "Read one complete L1 event by its event id. Only events read with this tool or a context/source read may be cited in proposed memory changes."
    }

    fn parameters_schema(&self) -> &Value {
        static SCHEMA: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
        SCHEMA.get_or_init(|| {
            json!({
                "type": "object",
                "properties": {"event_id": {"type": "string"}},
                "required": ["event_id"]
            })
        })
    }

    fn execute<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        Box::pin(async move {
            let event_id = required_string(&args, "event_id")?;
            let event = self
                .store
                .read_event(event_id)
                .map_err(|err| ToolError::Execution(err.to_string()))?;
            let reference = event_reference(&event);
            self.tracker.record(
                "reading_evidence",
                self.name(),
                format!("Read {} evidence", reference),
                vec![reference.clone()],
            );
            Ok(json_tool_result(
                json!({"reference": reference, "event": event}),
            ))
        })
    }
}

impl Tool for ReadMemoryContextTool {
    fn name(&self) -> &str {
        "read_memory_context"
    }

    fn description(&self) -> &str {
        "Read bounded events before and after one L1 event in the same source session. Use this when an isolated event lacks enough context."
    }

    fn parameters_schema(&self) -> &Value {
        static SCHEMA: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
        SCHEMA.get_or_init(|| {
            json!({
                "type": "object",
                "properties": {
                    "event_id": {"type": "string"},
                    "before": {"type": "integer", "minimum": 0, "maximum": 20},
                    "after": {"type": "integer", "minimum": 0, "maximum": 20}
                },
                "required": ["event_id"]
            })
        })
    }

    fn execute<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        Box::pin(async move {
            let event_id = required_string(&args, "event_id")?;
            let context = self
                .store
                .event_context(
                    event_id,
                    args["before"].as_u64().unwrap_or(2) as usize,
                    args["after"].as_u64().unwrap_or(2) as usize,
                )
                .map_err(|err| ToolError::Execution(err.to_string()))?;
            let refs = std::iter::once(&context.event)
                .chain(context.before.iter())
                .chain(context.after.iter())
                .map(event_reference)
                .collect::<Vec<_>>();
            self.tracker.record(
                "reading_evidence",
                self.name(),
                format!("Read {} contextual L1 events", refs.len()),
                refs.clone(),
            );
            Ok(json_tool_result(
                json!({"references": refs, "context": context}),
            ))
        })
    }
}

impl Tool for ReadMemorySourceTool {
    fn name(&self) -> &str {
        "read_memory_source"
    }

    fn description(&self) -> &str {
        "Resolve a memory source reference to its complete L1 event snapshot. Supports legacy session references, but returns the canonical event-level reference."
    }

    fn parameters_schema(&self) -> &Value {
        static SCHEMA: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
        SCHEMA.get_or_init(|| {
            json!({
                "type": "object",
                "properties": {"reference": {"type": "string"}},
                "required": ["reference"]
            })
        })
    }

    fn execute<'a>(
        &'a self,
        args: Value,
        _ctx: &'a ToolContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        Box::pin(async move {
            let reference = required_string(&args, "reference")?;
            let source = self
                .store
                .resolve_source_ref(reference)
                .map_err(|err| ToolError::Execution(err.to_string()))?;
            let canonical_reference = event_reference(&source.event);
            self.tracker.record(
                "reading_evidence",
                self.name(),
                format!("Resolved {reference} to {canonical_reference}"),
                vec![canonical_reference.clone()],
            );
            Ok(json_tool_result(json!({
                "requested_reference": reference,
                "canonical_reference": canonical_reference,
                "event": source.event,
            })))
        })
    }
}

fn event_page_schema(require_query: bool) -> Value {
    let mut schema = json!({
        "type": "object",
        "properties": {
            "surface": {"type": "string", "enum": ["chat", "quiz", "notebook", "knowledge"]},
            "session_id": {"type": "string"},
            "cursor": {"type": "string"},
            "limit": {"type": "integer", "minimum": 1, "maximum": 100}
        }
    });
    if require_query {
        schema["properties"]["query"] = json!({"type": "string"});
        schema["required"] = json!(["query"]);
    }
    schema
}

fn event_summaries(events: &[MemoryEvent]) -> Vec<Value> {
    events
        .iter()
        .map(|event| {
            json!({
                "event_id": event.id,
                "reference": event_reference(event),
                "surface": format!("{:?}", event.category).to_lowercase(),
                "action": event.action,
                "summary": event.summary,
                "session_id": event.source_id,
                "created_at": event.created_at,
            })
        })
        .collect()
}

fn event_reference(event: &MemoryEvent) -> String {
    let surface = format!("{:?}", event.category).to_lowercase();
    format!("{surface}:{}", event.id)
}

fn optional_string<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args[key]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn required_string<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    optional_string(args, key)
        .ok_or_else(|| ToolError::InvalidArguments(format!("{key} is required")))
}

fn json_tool_result(value: Value) -> ToolResult {
    ToolResult {
        content: vec![ContentBlock::Text {
            text: value.to_string(),
        }],
        details: value,
        terminate: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use llm_harness_types::UnsupportedEnv;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    fn make_ctx() -> ToolContext {
        let (update_tx, _update_rx) = mpsc::channel(1);
        ToolContext {
            env: Arc::new(UnsupportedEnv::new()),
            abort: CancellationToken::new(),
            tool_use_id: "memory-tool-test".into(),
            turn_index: 0,
            assistant_message: Arc::new(llm_harness_types::AssistantMessage {
                kind: llm_harness_types::AssistantMessageKind::FinalAnswer,
                message_id: "message".into(),
                turn_id: "turn".into(),
                content: vec![],
                usage: None,
                stop_reason: None,
                timestamp: Utc::now(),
                provider: None,
                api: None,
                model: None,
                error_message: None,
            }),
            update_tx,
        }
    }

    #[test]
    fn event_schema_only_exposes_active_l1_surfaces() {
        let schema = event_page_schema(false);
        let values = schema["properties"]["surface"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(values, vec!["chat", "quiz", "notebook", "knowledge"]);
    }

    #[tokio::test]
    async fn listing_does_not_make_unread_events_citeable() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(MemoryStore::new_with_root(dir.path().join("memory")));
        store
            .record_event(
                crate::memory_store::MemoryEventCategory::Chat,
                "asked",
                "Asked about vector addition",
                Some("session-1".into()),
                json!({ "content": "full question" }),
            )
            .unwrap();
        let tracker = MemoryEvidenceTracker::default();
        let tool = ListMemoryEventsTool::new(store, tracker.clone());

        let result = tool
            .execute(json!({ "surface": "chat", "limit": 10 }), &make_ctx())
            .await
            .unwrap();

        assert_eq!(result.details["total"], 1);
        assert_eq!(tracker.read_refs(), Vec::<String>::new());
        assert!(result.details["events"][0].get("payload").is_none());
    }

    #[tokio::test]
    async fn reading_tracks_the_canonical_event_reference() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(MemoryStore::new_with_root(dir.path().join("memory")));
        let event = store
            .record_event(
                crate::memory_store::MemoryEventCategory::Quiz,
                "answered",
                "Answered a vector question",
                Some("quiz-1".into()),
                json!({ "answer": "complete answer" }),
            )
            .unwrap();
        let tracker = MemoryEvidenceTracker::default();
        let tool = ReadMemoryEventTool::new(store, tracker.clone());

        let result = tool
            .execute(json!({ "event_id": event.id }), &make_ctx())
            .await
            .unwrap();
        let reference = result.details["reference"].as_str().unwrap().to_string();

        assert_eq!(tracker.read_refs(), vec![reference]);
        assert_eq!(
            result.details["event"]["payload"]["answer"],
            "complete answer"
        );
    }
}
