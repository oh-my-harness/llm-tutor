use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::{Deserialize, Serialize};

use crate::notebook_store::{NotebookEntry, NotebookStore};
use crate::quiz_store::{QuizQuestion, QuizSession, QuizStore};

#[derive(Clone)]
struct SpaceState {
    notebook: Arc<NotebookStore>,
    quizzes: Arc<QuizStore>,
}

#[derive(Debug, Deserialize)]
struct MentionQuery {
    q: Option<String>,
    limit: Option<usize>,
    space_id: Option<String>,
    #[serde(rename = "type")]
    mention_type: Option<SpaceMentionType>,
}

#[derive(Debug, Deserialize)]
struct ReadItemQuery {
    question_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpaceMentionType {
    NotebookEntry,
    QuizSession,
    QuizQuestion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpaceMention {
    pub id: String,
    #[serde(rename = "type")]
    pub mention_type: SpaceMentionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub question_id: Option<String>,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preview: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
struct SpaceItem {
    mention: SpaceMention,
    content_markdown: String,
    data: serde_json::Value,
}

async fn list_mentions(
    State(state): State<SpaceState>,
    Query(query): Query<MentionQuery>,
) -> impl IntoResponse {
    let query_text = query.q.as_deref().unwrap_or_default().trim();
    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let space_id = query
        .space_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mention_type = query.mention_type;
    let mut mentions = Vec::new();

    if mention_type_matches(mention_type, SpaceMentionType::NotebookEntry) {
        for entry in state.notebook.list(space_id) {
            if !matches_query(
                query_text,
                &[&entry.title, entry_type_label(&entry), &entry.markdown],
            ) {
                continue;
            }
            mentions.push(mention_for_notebook_entry(&entry));
            if mentions.len() >= limit {
                return ok_mentions(mentions);
            }
        }
    }

    for quiz in state.quizzes.list() {
        if mention_type_matches(mention_type, SpaceMentionType::QuizSession)
            && matches_query(
                query_text,
                &[&quiz.title, quiz_topic(&quiz), &quiz_stems(&quiz)],
            )
        {
            mentions.push(mention_for_quiz_session(&quiz));
            if mentions.len() >= limit {
                return ok_mentions(mentions);
            }
        }

        if !mention_type_matches(mention_type, SpaceMentionType::QuizQuestion) {
            continue;
        }
        for question in &quiz.questions {
            if !matches_query(
                query_text,
                &[
                    &quiz.title,
                    &question.stem,
                    &question.explanation,
                    &question.tags.join(" "),
                ],
            ) {
                continue;
            }
            mentions.push(mention_for_quiz_question(&quiz, question));
            if mentions.len() >= limit {
                return ok_mentions(mentions);
            }
        }
    }

    ok_mentions(mentions)
}

fn mention_type_matches(filter: Option<SpaceMentionType>, mention_type: SpaceMentionType) -> bool {
    match filter {
        Some(filter) => filter == mention_type,
        None => true,
    }
}

async fn read_item(
    State(state): State<SpaceState>,
    Path((item_type, target_id)): Path<(String, String)>,
    Query(query): Query<ReadItemQuery>,
) -> impl IntoResponse {
    match item_type.as_str() {
        "notebook_entry" => match state.notebook.get(&target_id) {
            Some(entry) => ok_item(SpaceItem {
                mention: mention_for_notebook_entry(&entry),
                content_markdown: notebook_markdown(&entry),
                data: serde_json::json!({ "notebook_entry": entry }),
            }),
            None => error_response(StatusCode::NOT_FOUND, "notebook entry not found"),
        },
        "quiz_session" => match state.quizzes.get(&target_id) {
            Some(quiz) => ok_item(SpaceItem {
                mention: mention_for_quiz_session(&quiz),
                content_markdown: quiz_session_markdown(&quiz),
                data: serde_json::json!({ "quiz": quiz }),
            }),
            None => error_response(StatusCode::NOT_FOUND, "quiz not found"),
        },
        "quiz_question" => {
            let Some(question_id) = query
                .question_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                return error_response(StatusCode::BAD_REQUEST, "question_id is required");
            };
            let Some(quiz) = state.quizzes.get(&target_id) else {
                return error_response(StatusCode::NOT_FOUND, "quiz not found");
            };
            let Some(question) = quiz
                .questions
                .iter()
                .find(|question| question.id == question_id)
            else {
                return error_response(StatusCode::NOT_FOUND, "quiz question not found");
            };
            ok_item(SpaceItem {
                mention: mention_for_quiz_question(&quiz, question),
                content_markdown: quiz_question_markdown(&quiz, question),
                data: serde_json::json!({ "quiz": quiz, "question": question }),
            })
        }
        _ => error_response(StatusCode::BAD_REQUEST, "unsupported space item type"),
    }
}

pub fn resolve_space_mention_markdown(
    notebook: &NotebookStore,
    quizzes: &QuizStore,
    mention: &SpaceMention,
) -> Option<(String, String)> {
    let target_id = mention
        .target_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .or_else(|| target_from_mention_id(&mention.id));
    match mention.mention_type {
        SpaceMentionType::NotebookEntry => {
            let entry = notebook.get(target_id?)?;
            Some((
                mention_for_notebook_entry(&entry).id,
                notebook_markdown(&entry),
            ))
        }
        SpaceMentionType::QuizSession => {
            let quiz = quizzes.get(target_id?)?;
            Some((
                mention_for_quiz_session(&quiz).id,
                quiz_session_markdown(&quiz),
            ))
        }
        SpaceMentionType::QuizQuestion => {
            let quiz = quizzes.get(target_id?)?;
            let question_id = mention
                .question_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .or_else(|| question_from_mention_id(&mention.id))?;
            let question = quiz
                .questions
                .iter()
                .find(|question| question.id == question_id)?;
            Some((
                mention_for_quiz_question(&quiz, question).id,
                quiz_question_markdown(&quiz, question),
            ))
        }
    }
}

pub fn space_router(notebook: Arc<NotebookStore>, quizzes: Arc<QuizStore>) -> Router {
    let state = SpaceState { notebook, quizzes };
    Router::new()
        .route("/api/space/mentions", get(list_mentions))
        .route("/api/space/items/{item_type}/{target_id}", get(read_item))
        .with_state(state)
}

fn ok_mentions(mentions: Vec<SpaceMention>) -> axum::response::Response {
    (
        StatusCode::OK,
        Json(serde_json::json!({ "mentions": mentions })),
    )
        .into_response()
}

fn ok_item(item: SpaceItem) -> axum::response::Response {
    (StatusCode::OK, Json(serde_json::json!({ "item": item }))).into_response()
}

fn error_response(status: StatusCode, message: &str) -> axum::response::Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

fn mention_for_notebook_entry(entry: &NotebookEntry) -> SpaceMention {
    SpaceMention {
        id: format!("notebook_entry:{}", entry.id),
        mention_type: SpaceMentionType::NotebookEntry,
        target_id: Some(entry.id.clone()),
        question_id: None,
        title: entry.title.clone(),
        preview: first_text_line(&entry.markdown),
        metadata: serde_json::json!({
            "entry_type": entry.entry_type,
            "space_id": entry.space_id,
            "updated_at": entry.updated_at,
            "source_session_id": entry.source_session_id,
            "source_message_id": entry.source_message_id,
        }),
    }
}

fn mention_for_quiz_session(quiz: &QuizSession) -> SpaceMention {
    SpaceMention {
        id: format!("quiz_session:{}", quiz.id),
        mention_type: SpaceMentionType::QuizSession,
        target_id: Some(quiz.id.clone()),
        question_id: None,
        title: quiz.title.clone(),
        preview: Some(format!(
            "{} questions - {}",
            quiz.questions.len(),
            score_label(quiz)
        )),
        metadata: serde_json::json!({
            "status": quiz.status,
            "kb_id": quiz.kb_id,
            "topic": quiz.config.topic,
            "difficulty": quiz.config.difficulty,
            "updated_at": quiz.updated_at,
        }),
    }
}

fn mention_for_quiz_question(quiz: &QuizSession, question: &QuizQuestion) -> SpaceMention {
    SpaceMention {
        id: format!("quiz_question:{}:{}", quiz.id, question.id),
        mention_type: SpaceMentionType::QuizQuestion,
        target_id: Some(quiz.id.clone()),
        question_id: Some(question.id.clone()),
        title: truncate_chars(&question.stem, 120),
        preview: Some(format!("{} - {}", quiz.title, answer_label(question))),
        metadata: serde_json::json!({
            "quiz_id": quiz.id,
            "quiz_title": quiz.title,
            "question_id": question.id,
            "difficulty": question.difficulty,
            "tags": question.tags,
            "updated_at": quiz.updated_at,
        }),
    }
}

fn target_from_mention_id(id: &str) -> Option<&str> {
    let mut parts = id.split(':');
    let _kind = parts.next()?;
    parts.next().filter(|value| !value.trim().is_empty())
}

fn question_from_mention_id(id: &str) -> Option<&str> {
    let mut parts = id.split(':');
    let _kind = parts.next()?;
    let _target = parts.next()?;
    parts.next().filter(|value| !value.trim().is_empty())
}

fn notebook_markdown(entry: &NotebookEntry) -> String {
    format!(
        "# {}\n\nType: {}\n\n{}",
        entry.title,
        entry_type_label(entry),
        entry.markdown
    )
}

fn quiz_session_markdown(quiz: &QuizSession) -> String {
    let mut markdown = format!(
        "# {}\n\nStatus: {:?}\nScore: {}\nKnowledge base: {}\n\n",
        quiz.title,
        quiz.status,
        score_label(quiz),
        quiz.kb_id
    );
    for (index, question) in quiz.questions.iter().enumerate() {
        markdown.push_str(&format!(
            "## Question {}\n\n{}\n\n{}\n\nCorrect answer: {}\n\nExplanation: {}\n\n",
            index + 1,
            question.stem,
            question
                .options
                .iter()
                .map(|option| format!("- {}. {}", option.id, option.text))
                .collect::<Vec<_>>()
                .join("\n"),
            answer_label(question),
            question.explanation
        ));
    }
    markdown
}

fn quiz_question_markdown(quiz: &QuizSession, question: &QuizQuestion) -> String {
    format!(
        "# {}\n\nQuiz: {}\n\n{}\n\nCorrect answer: {}\n\nExplanation: {}\n\nTags: {}",
        question.stem,
        quiz.title,
        question
            .options
            .iter()
            .map(|option| format!("- {}. {}", option.id, option.text))
            .collect::<Vec<_>>()
            .join("\n"),
        answer_label(question),
        question.explanation,
        question.tags.join(", ")
    )
}

fn matches_query(query: &str, values: &[&str]) -> bool {
    if query.is_empty() {
        return true;
    }
    let query = query.to_lowercase();
    values
        .iter()
        .any(|value| value.to_lowercase().contains(&query))
}

fn first_text_line(markdown: &str) -> Option<String> {
    markdown
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| truncate_chars(line, 180))
}

fn truncate_chars(value: &str, limit: usize) -> String {
    let mut text = value.trim().chars().take(limit).collect::<String>();
    if value.trim().chars().count() > limit {
        text.push_str("...");
    }
    text
}

fn entry_type_label(entry: &NotebookEntry) -> &'static str {
    match entry.entry_type {
        crate::notebook_store::NotebookEntryType::Note => "note",
        crate::notebook_store::NotebookEntryType::ResearchReport => "research_report",
        crate::notebook_store::NotebookEntryType::ChatAnswer => "chat_answer",
        crate::notebook_store::NotebookEntryType::SourceSnippet => "source_snippet",
        crate::notebook_store::NotebookEntryType::QuizSummary => "quiz_summary",
        crate::notebook_store::NotebookEntryType::DeepSolveResult => "deep_solve_result",
    }
}

fn quiz_topic(quiz: &QuizSession) -> &str {
    quiz.config.topic.as_deref().unwrap_or_default()
}

fn quiz_stems(quiz: &QuizSession) -> String {
    quiz.questions
        .iter()
        .map(|question| question.stem.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn score_label(quiz: &QuizSession) -> String {
    quiz.score
        .as_ref()
        .map(|score| format!("{}/{}", score.correct, score.total))
        .unwrap_or_else(|| "not answered".to_string())
}

fn answer_label(question: &QuizQuestion) -> String {
    question
        .options
        .iter()
        .find(|option| option.id == question.correct_option_id)
        .map(|option| format!("{}. {}", option.id, option.text))
        .unwrap_or_else(|| question.correct_option_id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request};
    use chrono::Utc;
    use tower::ServiceExt;

    use crate::notebook_store::{NotebookEntryInput, NotebookEntryType};
    use crate::quiz_store::{QuizConfig, QuizDifficulty, QuizOption, QuizQuestionType, QuizStatus};

    #[tokio::test]
    async fn lists_and_reads_space_mentions() {
        let dir = tempfile::tempdir().unwrap();
        let notebook = Arc::new(NotebookStore::new_with_path(dir.path().join("notebook")));
        let quizzes = Arc::new(QuizStore::new_with_path(dir.path().join("quizzes.json")));
        let entry = notebook
            .create(NotebookEntryInput {
                space_id: None,
                entry_type: NotebookEntryType::ResearchReport,
                path: None,
                title: "Lithography notes".into(),
                markdown: "# Lithography\n\nPhotoresist and mask alignment.".into(),
                metadata: None,
                source_session_id: None,
                source_message_id: None,
            })
            .unwrap();
        let quiz = quizzes
            .create(
                "Lithography quiz".into(),
                "__conversation__".into(),
                QuizConfig {
                    topic: Some("lithography".into()),
                    difficulty: QuizDifficulty::Medium,
                    question_count: 1,
                    question_type: QuizQuestionType::SingleChoice,
                },
            )
            .unwrap();
        let question = QuizQuestion {
            id: "q1".into(),
            question_type: QuizQuestionType::SingleChoice,
            stem: "What does photoresist do?".into(),
            options: vec![
                QuizOption {
                    id: "A".into(),
                    text: "Records light exposure".into(),
                },
                QuizOption {
                    id: "B".into(),
                    text: "Cools the wafer".into(),
                },
            ],
            correct_option_id: "A".into(),
            explanation: "Photoresist changes after exposure and development.".into(),
            citations: vec![],
            tags: vec!["photoresist".into()],
            difficulty: QuizDifficulty::Medium,
        };
        quizzes.replace_questions(&quiz.id, vec![question]).unwrap();

        let app = space_router(notebook, quizzes);
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/space/mentions?q=lithography")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        let mentions = body["mentions"].as_array().unwrap();
        assert!(mentions.iter().any(|item| item["type"] == "notebook_entry"));
        assert!(mentions.iter().any(|item| item["type"] == "quiz_session"));

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/space/mentions?q=lithography&type=notebook_entry")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        let mentions = body["mentions"].as_array().unwrap();
        assert!(!mentions.is_empty());
        assert!(mentions.iter().all(|item| item["type"] == "notebook_entry"));

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/space/mentions?q=photoresist&type=quiz_question")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        let mentions = body["mentions"].as_array().unwrap();
        assert!(!mentions.is_empty());
        assert!(mentions.iter().all(|item| item["type"] == "quiz_question"));

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/api/space/items/notebook_entry/{}", entry.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert!(
            body["item"]["content_markdown"]
                .as_str()
                .unwrap()
                .contains("Photoresist")
        );
    }

    #[test]
    fn mention_ids_are_stable() {
        let quiz = QuizSession {
            id: "quiz-1".into(),
            title: "Quiz".into(),
            kb_id: "__conversation__".into(),
            status: QuizStatus::Active,
            config: QuizConfig {
                topic: None,
                difficulty: QuizDifficulty::Easy,
                question_count: 1,
                question_type: QuizQuestionType::SingleChoice,
            },
            questions: vec![],
            answers: vec![],
            score: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        assert_eq!(mention_for_quiz_session(&quiz).id, "quiz_session:quiz-1");
    }

    async fn response_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }
}
