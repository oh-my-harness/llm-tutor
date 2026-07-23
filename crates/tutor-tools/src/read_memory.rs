use std::path::PathBuf;

use futures::future::BoxFuture;
use llm_harness_types::{DataBlock, Tool, ToolContext, ToolFailure, ToolResult};
use serde_json::json;

static SCHEMA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();

const L3_FILES: &[(&str, &str)] = &[
    ("recent", "L3/recent.md"),
    ("profile", "L3/profile.md"),
    ("scope", "L3/scope.md"),
    ("preferences", "L3/preferences.md"),
    ("teaching_strategy", "L3/teaching_strategy.md"),
];

/// Read visible learner memory from the product Markdown memory directory.
pub struct ReadMemoryTool {
    root: PathBuf,
}

impl ReadMemoryTool {
    pub fn new() -> Self {
        Self::with_root(default_root().join("memory"))
    }

    pub fn with_root(root: PathBuf) -> Self {
        Self { root }
    }
}

impl Default for ReadMemoryTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ReadMemoryTool {
    fn name(&self) -> &str {
        "read_memory"
    }

    fn description(&self) -> &str {
        "Read the learner's visible Markdown memory silently as internal context. Use it for personalization, prior weaknesses, preferences, recent learning state, scope, and teaching strategy. Do not announce the tool or storage operation in the answer. It is not a factual source."
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        SCHEMA.get_or_init(|| {
            json!({
                "type": "object",
                "properties": {
                    "scope": {
                        "type": "string",
                        "enum": ["recent", "profile", "scope", "preferences", "teaching_strategy", "all"],
                        "description": "Memory scope to read. Defaults to all L3 files."
                    },
                    "query": {
                        "type": "string",
                        "description": "Optional future filter hint. The MVP returns the selected Markdown files without retrieval filtering."
                    }
                }
            })
        })
    }

    fn execute<'a>(
        &'a self,
        args: serde_json::Value,
        _ctx: &'a ToolContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolFailure>> {
        Box::pin(async move {
            let scope = args["scope"].as_str().unwrap_or("all");
            let query = args["query"]
                .as_str()
                .filter(|value| !value.trim().is_empty())
                .map(str::trim)
                .map(ToOwned::to_owned);
            let selected = selected_files(scope).ok_or_else(|| {
                ToolFailure::invalid_arguments(format!("unsupported memory scope `{scope}`"))
            })?;

            let mut files = Vec::new();
            let mut sections = Vec::new();
            for (_, relative_path) in selected {
                let path = self.root.join(relative_path);
                match std::fs::read_to_string(&path) {
                    Ok(markdown) if !markdown.trim().is_empty() => {
                        files.push((*relative_path).to_string());
                        sections.push(format!("## {relative_path}\n\n{}", markdown.trim()));
                    }
                    Ok(_) | Err(_) => {}
                }
            }

            if sections.is_empty() {
                return Ok(ToolResult::ephemeral(
                    vec![DataBlock::text("No learner memory has been recorded yet.")],
                    "Checked learner memory; no entries were available.",
                    json!({
                        "scope": scope,
                        "query": query,
                        "files": [],
                        "empty": true,
                    }),
                    false,
                ));
            }

            let markdown = sections.join("\n\n---\n\n");
            Ok(ToolResult::ephemeral(
                vec![DataBlock::text(markdown.clone())],
                format!(
                    "Read learner memory scope `{scope}` from {} file(s).",
                    files.len()
                ),
                json!({
                    "scope": scope,
                    "query": query,
                    "files": files,
                    "empty": false,
                    "markdown": markdown,
                }),
                false,
            ))
        })
    }
}

fn selected_files(scope: &str) -> Option<Vec<&'static (&'static str, &'static str)>> {
    let scope = scope.trim();
    if scope.is_empty() || scope == "all" {
        return Some(L3_FILES.iter().collect());
    }
    L3_FILES
        .iter()
        .find(|(name, _)| *name == scope)
        .map(|item| vec![item])
}

fn default_root() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".llm-tutor")
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
    async fn read_memory_returns_empty_message_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let tool = ReadMemoryTool::with_root(dir.path().join("memory"));
        let result = tool
            .execute(json!({ "scope": "all" }), &make_ctx())
            .await
            .unwrap();
        assert_eq!(result.details["empty"], true);
        match &result.model_content[0] {
            DataBlock::Text { text, .. } => assert!(text.contains("No learner memory")),
            _ => panic!("expected text content"),
        }
    }

    #[tokio::test]
    async fn read_memory_returns_selected_l3_markdown() {
        let dir = tempfile::tempdir().unwrap();
        let memory = dir.path().join("memory");
        std::fs::create_dir_all(memory.join("L3")).unwrap();
        std::fs::write(
            memory.join(std::path::Path::new("L3/profile.md")),
            "# Student profile\n\n- Needs examples.",
        )
        .unwrap();
        std::fs::write(
            memory.join(std::path::Path::new("L3/preferences.md")),
            "# Preferences\n\n- Concise.",
        )
        .unwrap();

        let tool = ReadMemoryTool::with_root(memory);
        let result = tool
            .execute(json!({ "scope": "profile" }), &make_ctx())
            .await
            .unwrap();
        assert_eq!(result.details["empty"], false);
        assert_eq!(result.details["files"][0], "L3/profile.md");
        match &result.model_content[0] {
            DataBlock::Text { text, .. } => {
                assert!(text.contains("Needs examples"));
                assert!(!text.contains("Concise"));
            }
            _ => panic!("expected text content"),
        }
    }
}
