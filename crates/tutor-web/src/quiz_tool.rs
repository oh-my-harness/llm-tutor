use std::path::PathBuf;
use std::sync::Arc;

use futures::future::BoxFuture;
use llm_harness_types::{ContentBlock, Tool, ToolContext, ToolError, ToolResult};
use serde_json::json;

use crate::knowledge_store::KnowledgeStore;
use crate::memory_store::MemoryStore;
use crate::notebook_store::NotebookStore;
use crate::quiz_store::{QuizDifficulty, QuizStore};
use crate::routes::quiz::{CreateLlmConfig, CreateQuizRequest, QuizState, create_quiz_for_request};

static CREATE_QUIZ_SCHEMA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();

pub(crate) struct CreateQuizTool {
    store: Arc<QuizStore>,
    knowledge: Arc<KnowledgeStore>,
    notebook: Arc<NotebookStore>,
    memory: Arc<MemoryStore>,
    rag_root: PathBuf,
    default_kb_id: Option<String>,
    llm: Option<CreateLlmConfig>,
}

impl CreateQuizTool {
    pub(crate) fn new(
        store: Arc<QuizStore>,
        knowledge: Arc<KnowledgeStore>,
        notebook: Arc<NotebookStore>,
        memory: Arc<MemoryStore>,
        rag_root: PathBuf,
        default_kb_id: Option<String>,
        llm: Option<CreateLlmConfig>,
    ) -> Self {
        Self {
            store,
            knowledge,
            notebook,
            memory,
            rag_root,
            default_kb_id,
            llm,
        }
    }
}

impl Tool for CreateQuizTool {
    fn name(&self) -> &str {
        "create_quiz"
    }

    fn description(&self) -> &str {
        "Create an interactive Quiz only after the user explicitly asks to generate one or confirms a quiz plan. Use source_text for conversation, attachments, or read_space_item results; use kb_id for a knowledge base; use notebook_entry_id for a specific note. Do not call this just to discuss quiz scope."
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        CREATE_QUIZ_SCHEMA.get_or_init(|| {
            json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "Optional quiz title."
                    },
                    "topic": {
                        "type": "string",
                        "description": "Quiz focus or learning objective."
                    },
                    "difficulty": {
                        "type": "string",
                        "enum": ["easy", "medium", "hard"],
                        "description": "Target difficulty."
                    },
                    "question_count": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 10,
                        "description": "Number of questions to generate."
                    },
                    "kb_id": {
                        "type": "string",
                        "description": "Knowledge base id. Omit to use the associated knowledge base if one is selected."
                    },
                    "notebook_entry_id": {
                        "type": "string",
                        "description": "Notebook entry id to use as source material."
                    },
                    "source_text": {
                        "type": "string",
                        "description": "Explicit source material from the conversation, attachments, or Space references. Do not put mere instructions here."
                    },
                    "source_label": {
                        "type": "string",
                        "description": "Human-readable label for source_text."
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
            let request = CreateQuizRequest {
                title: optional_string(&args, "title"),
                kb_id: optional_string(&args, "kb_id").or_else(|| self.default_kb_id.clone()),
                notebook_entry_id: optional_string(&args, "notebook_entry_id"),
                source_text: optional_string(&args, "source_text"),
                source_label: optional_string(&args, "source_label"),
                topic: optional_string(&args, "topic"),
                difficulty: optional_string(&args, "difficulty")
                    .as_deref()
                    .map(parse_difficulty)
                    .transpose()?,
                question_count: args["question_count"].as_u64().map(|value| value as usize),
                llm: self.llm.clone(),
            };
            let state = QuizState {
                store: self.store.clone(),
                knowledge: self.knowledge.clone(),
                notebook: self.notebook.clone(),
                memory: self.memory.clone(),
                rag_root: self.rag_root.clone(),
            };
            let quiz = create_quiz_for_request(&state, request)
                .await
                .map_err(|err| ToolError::Execution(err.to_string()))?;
            Ok(ToolResult {
                content: vec![ContentBlock::Text {
                    text: format!(
                        "Created Quiz \"{}\" with {} questions. The product UI will render the interactive quiz card.",
                        quiz.title,
                        quiz.questions.len()
                    ),
                }],
                details: json!({
                    "quiz_id": quiz.id,
                    "title": quiz.title,
                    "question_count": quiz.questions.len(),
                    "quiz": quiz,
                }),
                terminate: false,
            })
        })
    }
}

fn optional_string(args: &serde_json::Value, key: &str) -> Option<String> {
    args[key]
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn parse_difficulty(value: &str) -> Result<QuizDifficulty, ToolError> {
    match value.trim().to_ascii_lowercase().as_str() {
        "easy" => Ok(QuizDifficulty::Easy),
        "medium" => Ok(QuizDifficulty::Medium),
        "hard" => Ok(QuizDifficulty::Hard),
        other => Err(ToolError::InvalidArguments(format!(
            "unsupported difficulty `{other}`"
        ))),
    }
}
