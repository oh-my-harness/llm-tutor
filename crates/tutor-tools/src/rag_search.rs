use futures::future::BoxFuture;
use llm_harness_types::{ContentBlock, Tool, ToolContext, ToolError, ToolResult};
use serde_json::json;

static SCHEMA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();

/// Stub RAG knowledge-base search tool.
/// v0.1: returns a static snippet keyed on the query.
/// Replace the body of `execute` with a real vector-store call in v0.2.
pub struct RagSearchTool;

impl RagSearchTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RagSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for RagSearchTool {
    fn name(&self) -> &str {
        "rag_search"
    }

    fn description(&self) -> &str {
        "Search the course knowledge base for relevant passages about a topic."
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        SCHEMA.get_or_init(|| {
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "kb": { "type": "string", "description": "Knowledge base name (optional)" }
                },
                "required": ["query"]
            })
        })
    }

    fn execute<'a>(
        &'a self,
        args: serde_json::Value,
        _ctx: &'a ToolContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        Box::pin(async move {
            let query = args["query"].as_str().unwrap_or("").to_string();
            let kb = args["kb"].as_str().unwrap_or("default").to_string();
            // v0.1 stub: echo back query with a placeholder passage
            let text = format!(
                "[RAG:{kb}] Found passage for \"{query}\": \
                 This is a stub result. Replace with real vector-store retrieval in v0.2."
            );
            Ok(ToolResult {
                content: vec![ContentBlock::Text { text }],
                details: json!({ "query": query, "kb": kb, "hits": 1 }),
                terminate: false,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use llm_harness_types::UnsupportedEnv;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    fn make_ctx() -> ToolContext {
        let (tx, _rx) = mpsc::channel(1);
        ToolContext {
            env: Arc::new(UnsupportedEnv::new()),
            abort: CancellationToken::new(),
            tool_use_id: "test-id".into(),
            turn_index: 0,
            assistant_message: Arc::new(llm_harness_types::AssistantMessage {
                content: vec![],
                usage: None,
                stop_reason: None,
                timestamp: Utc::now(),
                provider: None,
                api: None,
                model: None,
                error_message: None,
            }),
            update_tx: tx,
        }
    }

    #[tokio::test]
    async fn rag_search_returns_text_content() {
        let tool = RagSearchTool::new();
        let args = serde_json::json!({ "query": "integration by parts", "kb": "calculus" });
        let ctx = make_ctx();
        let result = tool.execute(args, &ctx).await.unwrap();
        assert!(!result.content.is_empty());
        match &result.content[0] {
            ContentBlock::Text { text } => assert!(!text.is_empty()),
            _ => panic!("expected text content"),
        }
    }
}
