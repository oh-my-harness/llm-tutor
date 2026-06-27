use std::path::PathBuf;

use futures::future::BoxFuture;
use llm_harness_types::{ContentBlock, Tool, ToolContext, ToolError, ToolResult};
use serde_json::json;

static SCHEMA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();

/// Append explicit, user-approved learner preferences to visible Markdown memory.
pub struct WriteMemoryTool {
    root: PathBuf,
}

impl WriteMemoryTool {
    pub fn new() -> Self {
        Self::with_root(default_root().join("memory"))
    }

    pub fn with_root(root: PathBuf) -> Self {
        Self { root }
    }
}

impl Default for WriteMemoryTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for WriteMemoryTool {
    fn name(&self) -> &str {
        "write_memory"
    }

    fn description(&self) -> &str {
        "Append an explicit user-approved learner preference to visible memory. Use only when the user asks you to remember something or clearly confirms a durable preference."
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        SCHEMA.get_or_init(|| {
            json!({
                "type": "object",
                "required": ["text", "approved"],
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "Concise learner preference or user-approved durable fact to remember."
                    },
                    "section": {
                        "type": "string",
                        "description": "Optional Markdown section under preferences. Defaults to Explicit preferences."
                    },
                    "approved": {
                        "type": "boolean",
                        "description": "Must be true only when the user explicitly asked to remember this or approved recording it."
                    }
                }
            })
        })
    }

    fn execute<'a>(
        &'a self,
        args: serde_json::Value,
        _ctx: &'a ToolContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        Box::pin(async move {
            let approved = args["approved"].as_bool().unwrap_or(false);
            if !approved {
                return Err(ToolError::InvalidArguments(
                    "write_memory requires explicit user approval".into(),
                ));
            }

            let text = args["text"]
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| ToolError::InvalidArguments("text is required".into()))?;
            if text.len() > 500 {
                return Err(ToolError::InvalidArguments(
                    "memory text must be 500 characters or fewer".into(),
                ));
            }
            if text.lines().count() > 3 {
                return Err(ToolError::InvalidArguments(
                    "memory text must be concise and at most 3 lines".into(),
                ));
            }

            let section = args["section"]
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("Explicit preferences");
            if section.len() > 80 || section.contains('\n') || section.contains('\r') {
                return Err(ToolError::InvalidArguments(
                    "section must be a short single-line heading".into(),
                ));
            }

            let path = self.root.join("L3").join("preferences.md");
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|err| ToolError::Execution(err.to_string()))?;
            }

            let mut markdown = std::fs::read_to_string(&path).unwrap_or_default();
            if markdown.trim().is_empty() {
                markdown = "# Preferences\n\n".into();
            }
            let marker = memory_marker();
            append_preference(&mut markdown, section, text, &marker);
            std::fs::write(&path, markdown).map_err(|err| ToolError::Execution(err.to_string()))?;

            Ok(ToolResult {
                content: vec![ContentBlock::Text {
                    text: format!("Saved memory to L3/preferences.md: {text}"),
                }],
                details: json!({
                    "file": "L3/preferences.md",
                    "section": section,
                    "marker": marker,
                    "text": text,
                }),
                terminate: false,
            })
        })
    }
}

fn append_preference(markdown: &mut String, section: &str, text: &str, marker: &str) {
    if !markdown.ends_with('\n') {
        markdown.push('\n');
    }
    let heading = format!("## {section}");
    if !markdown.lines().any(|line| line.trim() == heading) {
        markdown.push('\n');
        markdown.push_str(&heading);
        markdown.push_str("\n\n");
    }
    markdown.push_str("- ");
    markdown.push_str(&text.replace('\n', " "));
    markdown.push(' ');
    markdown.push_str(marker);
    markdown.push('\n');
}

fn memory_marker() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("<!--m_{millis}-->")
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
    async fn write_memory_appends_approved_preference() {
        let dir = tempfile::tempdir().unwrap();
        let tool = WriteMemoryTool::with_root(dir.path().join("memory"));
        let result = tool
            .execute(
                json!({
                    "text": "Prefers concise explanations with examples.",
                    "section": "Explanation style",
                    "approved": true
                }),
                &make_ctx(),
            )
            .await
            .unwrap();

        assert_eq!(result.details["file"], "L3/preferences.md");
        let markdown =
            std::fs::read_to_string(dir.path().join("memory/L3/preferences.md")).unwrap();
        assert!(markdown.contains("# Preferences"));
        assert!(markdown.contains("## Explanation style"));
        assert!(markdown.contains("Prefers concise explanations with examples."));
        assert!(markdown.contains("<!--m_"));
    }

    #[tokio::test]
    async fn write_memory_rejects_unapproved_fact() {
        let dir = tempfile::tempdir().unwrap();
        let tool = WriteMemoryTool::with_root(dir.path().join("memory"));
        let err = tool
            .execute(
                json!({
                    "text": "Might like analogies.",
                    "approved": false
                }),
                &make_ctx(),
            )
            .await
            .unwrap_err();

        assert!(err.to_string().contains("explicit user approval"));
        assert!(!dir.path().join("memory/L3/preferences.md").exists());
    }
}
