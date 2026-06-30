use std::sync::Arc;

use futures::future::BoxFuture;
use llm_harness_types::{ContentBlock, Tool, ToolContext, ToolError, ToolResult};
use serde_json::json;

use crate::notebook_store::NotebookStore;
use crate::quiz_store::QuizStore;
use crate::routes::space::{SpaceMention, SpaceMentionType, resolve_space_mention_markdown};

static SCHEMA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();

pub struct ReadSpaceItemTool {
    notebook: Arc<NotebookStore>,
    quizzes: Arc<QuizStore>,
}

impl ReadSpaceItemTool {
    pub fn new(notebook: Arc<NotebookStore>, quizzes: Arc<QuizStore>) -> Self {
        Self { notebook, quizzes }
    }
}

impl Tool for ReadSpaceItemTool {
    fn name(&self) -> &str {
        "read_space_item"
    }

    fn description(&self) -> &str {
        "Read a user-mentioned Space artifact, such as a Notebook entry, Quiz session, or Quiz question. Use this when the user references @Space content and you need its exact content."
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        SCHEMA.get_or_init(|| {
            json!({
                "type": "object",
                "properties": {
                    "item_type": {
                        "type": "string",
                        "enum": ["notebook_entry", "quiz_session", "quiz_question"],
                        "description": "Type of Space artifact to read."
                    },
                    "target_id": {
                        "type": "string",
                        "description": "Notebook entry id or Quiz session id."
                    },
                    "question_id": {
                        "type": "string",
                        "description": "Required when item_type is quiz_question."
                    },
                    "mention_id": {
                        "type": "string",
                        "description": "Optional full mention id, e.g. notebook_entry:<id> or quiz_question:<quiz_id>:<question_id>."
                    }
                },
                "required": ["item_type", "target_id"]
            })
        })
    }

    fn execute<'a>(
        &'a self,
        args: serde_json::Value,
        _ctx: &'a ToolContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        Box::pin(async move {
            let mention = mention_from_args(args)?;
            let Some((resolved_id, markdown)) =
                resolve_space_mention_markdown(&self.notebook, &self.quizzes, &mention)
            else {
                return Ok(ToolResult {
                    content: vec![ContentBlock::Text {
                        text: "Space item not found. Ask the user to choose the item again from the Space picker.".into(),
                    }],
                    details: json!({
                        "found": false,
                        "requested": mention,
                    }),
                    terminate: false,
                });
            };

            Ok(ToolResult {
                content: vec![ContentBlock::Text {
                    text: markdown.clone(),
                }],
                details: json!({
                    "found": true,
                    "id": resolved_id,
                    "item_type": mention.mention_type,
                    "target_id": mention.target_id,
                    "question_id": mention.question_id,
                    "title": mention.title,
                    "markdown": markdown,
                }),
                terminate: false,
            })
        })
    }
}

fn mention_from_args(args: serde_json::Value) -> Result<SpaceMention, ToolError> {
    let item_type = args["item_type"]
        .as_str()
        .or_else(|| args["type"].as_str())
        .ok_or_else(|| ToolError::InvalidArguments("item_type is required".into()))?;
    let mention_type = match item_type {
        "notebook_entry" => SpaceMentionType::NotebookEntry,
        "quiz_session" => SpaceMentionType::QuizSession,
        "quiz_question" => SpaceMentionType::QuizQuestion,
        other => {
            return Err(ToolError::InvalidArguments(format!(
                "unsupported space item type `{other}`"
            )));
        }
    };
    let target_id = args["target_id"]
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string());
    let question_id = args["question_id"]
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string());
    let mention_id = args["mention_id"]
        .as_str()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .or_else(|| {
            let target_id = target_id.as_deref()?;
            Some(match mention_type {
                SpaceMentionType::NotebookEntry => format!("notebook_entry:{target_id}"),
                SpaceMentionType::QuizSession => format!("quiz_session:{target_id}"),
                SpaceMentionType::QuizQuestion => {
                    format!(
                        "quiz_question:{}:{}",
                        target_id,
                        question_id.as_deref().unwrap_or_default()
                    )
                }
            })
        })
        .ok_or_else(|| ToolError::InvalidArguments("target_id is required".into()))?;

    Ok(SpaceMention {
        id: mention_id,
        mention_type,
        target_id,
        question_id,
        title: args["title"]
            .as_str()
            .unwrap_or("Space item")
            .trim()
            .to_string(),
        preview: None,
        metadata: json!({}),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notebook_store::{NotebookEntryInput, NotebookEntryType};
    use chrono::Utc;
    use llm_harness_types::UnsupportedEnv;
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
    async fn read_space_item_returns_notebook_markdown() {
        let dir = tempfile::tempdir().unwrap();
        let notebook = Arc::new(NotebookStore::new_with_path(
            dir.path().join("notebook.json"),
        ));
        let quizzes = Arc::new(QuizStore::new_with_path(dir.path().join("quizzes.json")));
        let entry = notebook
            .create(NotebookEntryInput {
                space_id: None,
                entry_type: NotebookEntryType::Note,
                title: "Mask notes".into(),
                markdown: "Alignment marks matter.".into(),
                metadata: None,
                source_session_id: None,
                source_message_id: None,
            })
            .unwrap();
        let tool = ReadSpaceItemTool::new(notebook, quizzes);

        let result = tool
            .execute(
                json!({
                    "item_type": "notebook_entry",
                    "target_id": entry.id,
                }),
                &make_ctx(),
            )
            .await
            .unwrap();

        assert_eq!(result.details["found"], true);
        match &result.content[0] {
            ContentBlock::Text { text } => assert!(text.contains("Alignment marks")),
            _ => panic!("expected text content"),
        }
    }
}
