use std::sync::Arc;

use futures::future::BoxFuture;
use llm_harness_types::{ContentBlock, Tool, ToolContext, ToolError, ToolResult};
use serde_json::json;

use crate::notebook_store::{NotebookEntry, NotebookStore, parse_tags};
use crate::quiz_store::QuizStore;
use crate::routes::space::{SpaceMention, SpaceMentionType, resolve_space_mention_markdown};

static SCHEMA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();

pub struct ReadSpaceItemTool {
    notebook: Arc<NotebookStore>,
    quizzes: Arc<QuizStore>,
}

pub struct ProposeNotebookEditTool {
    notebook: Arc<NotebookStore>,
}

pub struct SearchNotebookTool {
    notebook: Arc<NotebookStore>,
}

impl ReadSpaceItemTool {
    pub fn new(notebook: Arc<NotebookStore>, quizzes: Arc<QuizStore>) -> Self {
        Self { notebook, quizzes }
    }
}

impl ProposeNotebookEditTool {
    pub fn new(notebook: Arc<NotebookStore>) -> Self {
        Self { notebook }
    }
}

impl SearchNotebookTool {
    pub fn new(notebook: Arc<NotebookStore>) -> Self {
        Self { notebook }
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

static SEARCH_NOTEBOOK_SCHEMA: std::sync::OnceLock<serde_json::Value> =
    std::sync::OnceLock::new();

impl Tool for SearchNotebookTool {
    fn name(&self) -> &str {
        "search_notebook"
    }

    fn description(&self) -> &str {
        "Search the user's Notebook as plain Markdown text. Use this when Notebook is associated or the user asks about saved notes without an explicit @ reference. This does not use embeddings or RAG."
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        SEARCH_NOTEBOOK_SCHEMA.get_or_init(|| {
            json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Plain-text query to search in Notebook titles, tags, and Markdown."
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 10,
                        "description": "Maximum number of Notebook entries to return."
                    }
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
            let query = args["query"]
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| ToolError::InvalidArguments("query is required".into()))?;
            let limit = args["limit"]
                .as_u64()
                .map(|value| value.clamp(1, 10) as usize)
                .unwrap_or(5);
            let hits = search_notebook_entries(&self.notebook.list(None), query, limit);
            let text = if hits.is_empty() {
                format!("No Notebook entries matched query: {query}")
            } else {
                hits.iter()
                    .enumerate()
                    .map(|(index, hit)| {
                        format!(
                            "{}. {} ({})\nID: {}\nTags: {}\nScore: {}\nSnippet: {}",
                            index + 1,
                            hit.title,
                            hit.entry_type,
                            hit.id,
                            if hit.tags.is_empty() {
                                "none".into()
                            } else {
                                hit.tags.join(", ")
                            },
                            hit.score,
                            hit.snippet
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n")
            };

            Ok(ToolResult {
                content: vec![ContentBlock::Text { text }],
                details: json!({
                    "query": query,
                    "hits": hits,
                }),
                terminate: false,
            })
        })
    }
}

static PROPOSE_NOTEBOOK_EDIT_SCHEMA: std::sync::OnceLock<serde_json::Value> =
    std::sync::OnceLock::new();

impl Tool for ProposeNotebookEditTool {
    fn name(&self) -> &str {
        "propose_notebook_edit"
    }

    fn description(&self) -> &str {
        "Create a preview-only Notebook edit proposal for a user-mentioned Notebook entry. This tool never writes data. The user must explicitly confirm the proposal before the product updates the Notebook entry."
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        PROPOSE_NOTEBOOK_EDIT_SCHEMA.get_or_init(|| {
            json!({
                "type": "object",
                "properties": {
                    "entry_id": {
                        "type": "string",
                        "description": "Notebook entry id to revise."
                    },
                    "proposed_title": {
                        "type": "string",
                        "description": "Optional replacement title. Omit to keep the current title."
                    },
                    "proposed_markdown": {
                        "type": "string",
                        "description": "Complete replacement Markdown for the Notebook entry."
                    },
                    "summary": {
                        "type": "string",
                        "description": "Short user-facing summary of the proposed change."
                    },
                    "proposal_kind": {
                        "type": "string",
                        "enum": ["edit", "links", "tags", "merge"],
                        "description": "Organization proposal type. Use links for wiki-link suggestions, tags for tag cleanup, merge for duplicate-note consolidation, and edit for general rewrites."
                    },
                    "suggested_links": {
                        "type": "array",
                        "description": "Optional wiki-link suggestions included in this replacement.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "text": { "type": "string" },
                                "target": { "type": "string" },
                                "reason": { "type": "string" }
                            },
                            "required": ["text", "target"]
                        }
                    },
                    "suggested_tags": {
                        "type": "array",
                        "description": "Optional tags to add, keep, or remove.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "tag": { "type": "string" },
                                "action": { "type": "string", "enum": ["add", "keep", "remove"] },
                                "reason": { "type": "string" }
                            },
                            "required": ["tag", "action"]
                        }
                    },
                    "merge_source_entry_ids": {
                        "type": "array",
                        "description": "For merge proposals, duplicate/source Notebook entry ids that were considered. Applying this proposal updates only entry_id; it does not delete these source entries.",
                        "items": { "type": "string" }
                    }
                },
                "required": ["entry_id", "proposed_markdown", "summary"]
            })
        })
    }

    fn execute<'a>(
        &'a self,
        args: serde_json::Value,
        _ctx: &'a ToolContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        Box::pin(async move {
            let entry_id = args["entry_id"]
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| ToolError::InvalidArguments("entry_id is required".into()))?;
            let Some(entry) = self.notebook.get(entry_id) else {
                return Ok(ToolResult {
                    content: vec![ContentBlock::Text {
                        text: "Notebook entry not found. Ask the user to choose the entry again from the Space picker.".into(),
                    }],
                    details: json!({
                        "found": false,
                        "entry_id": entry_id,
                    }),
                    terminate: false,
                });
            };
            let proposed_markdown = args["proposed_markdown"]
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| {
                    ToolError::InvalidArguments("proposed_markdown is required".into())
                })?;
            let proposed_title = args["proposed_title"]
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or(&entry.title);
            let summary = args["summary"]
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("Proposed Notebook update");
            let proposal_kind = args["proposal_kind"]
                .as_str()
                .map(str::trim)
                .filter(|value| matches!(*value, "edit" | "links" | "tags" | "merge"))
                .unwrap_or("edit");
            let suggested_links = args["suggested_links"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let suggested_tags = args["suggested_tags"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let merge_source_entry_ids = args["merge_source_entry_ids"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str())
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(ToOwned::to_owned)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            Ok(ToolResult {
                content: vec![ContentBlock::Text {
                    text: format!(
                        "Notebook {proposal_kind} proposal is ready for user review. It has not been applied. Entry: {}.",
                        entry.title
                    ),
                }],
                details: json!({
                    "found": true,
                    "proposal_kind": proposal_kind,
                    "entry_id": entry.id,
                    "entry_title": entry.title,
                    "current_markdown": entry.markdown,
                    "proposed_title": proposed_title,
                    "proposed_markdown": proposed_markdown,
                    "summary": summary,
                    "suggested_links": suggested_links,
                    "suggested_tags": suggested_tags,
                    "merge_source_entry_ids": merge_source_entry_ids,
                    "requires_confirmation": true,
                }),
                terminate: false,
            })
        })
    }
}

#[derive(Debug, serde::Serialize)]
struct NotebookSearchHit {
    id: String,
    title: String,
    entry_type: String,
    tags: Vec<String>,
    snippet: String,
    score: usize,
}

fn search_notebook_entries(
    entries: &[NotebookEntry],
    query: &str,
    limit: usize,
) -> Vec<NotebookSearchHit> {
    let terms = query
        .split_whitespace()
        .map(|term| term.trim().to_ascii_lowercase())
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();
    if terms.is_empty() {
        return vec![];
    }

    let mut hits = entries
        .iter()
        .filter_map(|entry| {
            let tags = parse_tags(&entry.markdown);
            let haystack = format!(
                "{}\n{}\n{}",
                entry.title,
                tags.join(" "),
                entry.markdown
            )
            .to_ascii_lowercase();
            let score = terms
                .iter()
                .map(|term| haystack.matches(term).count())
                .sum::<usize>();
            if score == 0 {
                return None;
            }
            Some(NotebookSearchHit {
                id: entry.id.clone(),
                title: entry.title.clone(),
                entry_type: format!("{:?}", entry.entry_type).to_ascii_lowercase(),
                tags,
                snippet: notebook_snippet(&entry.markdown, &terms),
                score,
            })
        })
        .collect::<Vec<_>>();

    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.title.cmp(&b.title)));
    hits.truncate(limit);
    hits
}

fn notebook_snippet(markdown: &str, terms: &[String]) -> String {
    let normalized = markdown.split_whitespace().collect::<Vec<_>>().join(" ");
    let lower = normalized.to_ascii_lowercase();
    let start = terms
        .iter()
        .filter_map(|term| lower.find(term))
        .min()
        .unwrap_or(0);
    let start = normalized
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= start)
        .last()
        .unwrap_or(0)
        .saturating_sub(80);
    let snippet = normalized
        .chars()
        .skip(start)
        .take(240)
        .collect::<String>();
    if normalized.chars().count() > snippet.chars().count() {
        format!("{snippet}...")
    } else {
        snippet
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
            dir.path().join("notebook"),
        ));
        let quizzes = Arc::new(QuizStore::new_with_path(dir.path().join("quizzes.json")));
        let entry = notebook
            .create(NotebookEntryInput {
                space_id: None,
                entry_type: NotebookEntryType::Note,
                path: None,
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

    #[tokio::test]
    async fn propose_notebook_edit_returns_preview_without_writing() {
        let dir = tempfile::tempdir().unwrap();
        let notebook = Arc::new(NotebookStore::new_with_path(
            dir.path().join("notebook"),
        ));
        let entry = notebook
            .create(NotebookEntryInput {
                space_id: None,
                entry_type: NotebookEntryType::Note,
                path: None,
                title: "Mask notes".into(),
                markdown: "Original notes.".into(),
                metadata: None,
                source_session_id: None,
                source_message_id: None,
            })
            .unwrap();
        let tool = ProposeNotebookEditTool::new(notebook.clone());

        let result = tool
            .execute(
                json!({
                    "entry_id": entry.id,
                    "proposed_title": "Updated mask notes",
                    "proposed_markdown": "# Updated\n\nBetter notes.",
                    "summary": "Rewrite as structured notes.",
                    "proposal_kind": "links",
                    "suggested_links": [
                        { "text": "mask alignment", "target": "Mask Alignment", "reason": "Connect related concept" }
                    ],
                    "suggested_tags": [
                        { "tag": "semiconductor", "action": "add", "reason": "Topic tag" }
                    ]
                }),
                &make_ctx(),
            )
            .await
            .unwrap();

        assert_eq!(result.details["requires_confirmation"], true);
        assert_eq!(result.details["proposal_kind"], "links");
        assert_eq!(result.details["suggested_links"][0]["target"], "Mask Alignment");
        assert_eq!(result.details["suggested_tags"][0]["tag"], "semiconductor");
        assert_eq!(result.details["proposed_title"], "Updated mask notes");
        assert_eq!(notebook.get(&entry.id).unwrap().markdown, "Original notes.");
    }

    #[tokio::test]
    async fn search_notebook_returns_plain_text_hits() {
        let dir = tempfile::tempdir().unwrap();
        let notebook = Arc::new(NotebookStore::new_with_path(
            dir.path().join("notebook"),
        ));
        let entry = notebook
            .create(NotebookEntryInput {
                space_id: None,
                entry_type: NotebookEntryType::Note,
                path: None,
                title: "Lithography notes".into(),
                markdown: "OPC and mask alignment. #semiconductor".into(),
                metadata: None,
                source_session_id: None,
                source_message_id: None,
            })
            .unwrap();
        let tool = SearchNotebookTool::new(notebook);

        let result = tool
            .execute(json!({ "query": "mask", "limit": 3 }), &make_ctx())
            .await
            .unwrap();

        assert_eq!(result.details["hits"][0]["id"], entry.id);
        match &result.content[0] {
            ContentBlock::Text { text } => assert!(text.contains("mask alignment")),
            _ => panic!("expected text content"),
        }
    }
}
