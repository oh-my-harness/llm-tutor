use futures::future::BoxFuture;
use llm_harness_types::{DataBlock, Tool, ToolContext, ToolFailure, ToolResult};
use serde_json::json;
use std::sync::Arc;
use tutor_rag::KnowledgeRetriever;

static SCHEMA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();

/// RAG knowledge-base search tool backed by a product retriever.
pub struct RagSearchTool {
    retriever: Option<Arc<dyn KnowledgeRetriever>>,
    associated_kb: Option<String>,
}

impl RagSearchTool {
    pub fn new() -> Self {
        Self {
            retriever: None,
            associated_kb: None,
        }
    }

    pub fn with_retriever(retriever: Arc<dyn KnowledgeRetriever>) -> Self {
        Self {
            retriever: Some(retriever),
            associated_kb: None,
        }
    }

    pub fn with_associated_kb(mut self, kb: impl Into<String>) -> Self {
        let kb = kb.into();
        if !kb.trim().is_empty() {
            self.associated_kb = Some(kb);
        }
        self
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
    ) -> BoxFuture<'a, Result<ToolResult, ToolFailure>> {
        Box::pin(async move {
            let query = args["query"].as_str().unwrap_or("").to_string();
            let kb = args["kb"]
                .as_str()
                .filter(|value| !value.trim().is_empty())
                .map(ToString::to_string)
                .or_else(|| self.associated_kb.clone());

            let Some(kb) = kb else {
                return Ok(ToolResult::projected(
                    vec![DataBlock::text(
                        "RAG is not associated with this conversation. Continue without course knowledge, or ask the user to select a knowledge base.",
                    )],
                    vec![DataBlock::text(
                        "RAG is not associated with this conversation.",
                    )],
                    json!({ "query": query, "kb": null, "hits": 0, "configured": false }),
                    false,
                ));
            };

            let Some(retriever) = &self.retriever else {
                return Ok(ToolResult::projected(
                    vec![DataBlock::text(
                        "RAG is not associated with this conversation. Continue without course knowledge, or ask the user to select a knowledge base.",
                    )],
                    vec![DataBlock::text(
                        "RAG is not associated with this conversation.",
                    )],
                    json!({ "query": query, "kb": kb, "hits": 0, "configured": false }),
                    false,
                ));
            };

            let hits = retriever
                .search(Some(&kb), &query, 5)
                .await
                .map_err(|err| ToolFailure::new("rag_search_failed", err.to_string()))?;

            if hits.is_empty() {
                let content = vec![DataBlock::text(format!(
                    "[RAG:{kb}] No relevant passages found for \"{query}\"."
                ))];
                return Ok(ToolResult::projected(
                    content.clone(),
                    content,
                    json!({ "query": query, "kb": kb, "hits": 0, "configured": true }),
                    false,
                ));
            }

            let text = hits
                .iter()
                .enumerate()
                .map(|(index, hit)| {
                    format!(
                        "[{}] source={} score={}\n{}",
                        index + 1,
                        hit.source,
                        hit.score
                            .map(|score| format!("{score:.4}"))
                            .unwrap_or_else(|| "n/a".into()),
                        hit.text
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n");
            let details_hits = hits
                .iter()
                .enumerate()
                .map(|(index, hit)| {
                    json!({
                        "index": index + 1,
                        "id": hit.id,
                        "kb": hit.kb,
                        "source": hit.source,
                        "raw_source": hit.raw_source,
                        "document_id": hit.document_id,
                        "chunk_id": hit.id,
                        "title": hit.source,
                        "text": hit.text,
                        "score": hit.score,
                    })
                })
                .collect::<Vec<_>>();

            Ok(ToolResult::ephemeral(
                vec![DataBlock::text(text)],
                format!(
                    "Searched course knowledge `{kb}` for `{query}` and found {} result(s).",
                    hits.len()
                ),
                json!({
                    "query": query,
                    "kb": kb,
                    "hits": hits.len(),
                    "configured": true,
                    "sources": details_hits,
                }),
                false,
            ))
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
            run: Arc::new(llm_harness_types::RunContext::new(
                llm_harness_types::RunRequest::default(),
            )),
            abort: CancellationToken::new(),
            tool_use_id: "test-id".into(),
            turn_index: 0,
            assistant_message: Arc::new(llm_harness_types::AssistantMessage {
                kind: llm_harness_types::AssistantMessageKind::FinalAnswer,
                message_id: "test-message".into(),
                turn_id: "test-turn".into(),
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
        assert!(!result.model_content.is_empty());
        match &result.model_content[0] {
            DataBlock::Text { text, .. } => assert!(!text.is_empty()),
            _ => panic!("expected text content"),
        }
    }
}
