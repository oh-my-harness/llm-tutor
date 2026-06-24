use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::Deserialize;
use tutor_rag::KnowledgeRetriever;

use crate::knowledge_store::KnowledgeStore;
use crate::quiz_store::{
    QuizCitation, QuizConfig, QuizDifficulty, QuizOption, QuizQuestion, QuizQuestionType,
    QuizSession, QuizStore,
};

#[derive(Clone)]
struct QuizState {
    store: Arc<QuizStore>,
    knowledge: Arc<KnowledgeStore>,
    rag_root: PathBuf,
}

#[derive(Debug, Deserialize)]
struct CreateQuizRequest {
    title: Option<String>,
    kb_id: String,
    topic: Option<String>,
    difficulty: Option<QuizDifficulty>,
    question_count: Option<usize>,
    llm: Option<CreateLlmConfig>,
}

#[derive(Debug, Deserialize)]
struct SubmitAnswerRequest {
    question_id: String,
    selected_option_id: String,
}

#[derive(Debug, Deserialize)]
struct CreateLlmConfig {
    provider: String,
    model: String,
    api_key: Option<String>,
    base_url: Option<String>,
    chat_path: Option<String>,
}

async fn list_quizzes(State(state): State<QuizState>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({ "quizzes": state.store.list() })),
    )
}

async fn create_quiz(
    State(state): State<QuizState>,
    Json(req): Json<CreateQuizRequest>,
) -> impl IntoResponse {
    let config = QuizConfig {
        topic: req.topic,
        difficulty: req.difficulty.unwrap_or(QuizDifficulty::Medium),
        question_count: req.question_count.unwrap_or(5).clamp(1, 10),
        question_type: QuizQuestionType::SingleChoice,
    };

    let quiz = match state
        .store
        .create(req.title.unwrap_or_default(), req.kb_id, config)
    {
        Ok(quiz) => quiz,
        Err(err) => return error_response(StatusCode::BAD_REQUEST, err),
    };

    match generate_questions(&state, quiz, req.llm).await {
        Ok(quiz) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "quiz": quiz })),
        )
            .into_response(),
        Err(err) => error_response(StatusCode::BAD_REQUEST, err),
    }
}

async fn get_quiz(State(state): State<QuizState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.store.get(&id) {
        Some(quiz) => (StatusCode::OK, Json(serde_json::json!({ "quiz": quiz }))).into_response(),
        None => error_response(StatusCode::NOT_FOUND, "quiz not found"),
    }
}

async fn submit_answer(
    State(state): State<QuizState>,
    Path(id): Path<String>,
    Json(req): Json<SubmitAnswerRequest>,
) -> impl IntoResponse {
    match state
        .store
        .submit_answer(&id, &req.question_id, &req.selected_option_id)
    {
        Ok(quiz) => (StatusCode::OK, Json(serde_json::json!({ "quiz": quiz }))).into_response(),
        Err(err) => error_response(StatusCode::BAD_REQUEST, err),
    }
}

async fn finish_quiz(State(state): State<QuizState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.store.finish(&id) {
        Ok(quiz) => (StatusCode::OK, Json(serde_json::json!({ "quiz": quiz }))).into_response(),
        Err(err) => error_response(StatusCode::BAD_REQUEST, err),
    }
}

async fn delete_quiz(State(state): State<QuizState>, Path(id): Path<String>) -> impl IntoResponse {
    if state.store.delete(&id) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        error_response(StatusCode::NOT_FOUND, "quiz not found")
    }
}

pub fn quiz_router(store: Arc<QuizStore>, knowledge: Arc<KnowledgeStore>) -> Router {
    quiz_router_with_rag_root(store, knowledge, tutor_rag::LanceDbRag::default_root())
}

fn quiz_router_with_rag_root(
    store: Arc<QuizStore>,
    knowledge: Arc<KnowledgeStore>,
    rag_root: impl Into<PathBuf>,
) -> Router {
    let state = QuizState {
        store,
        knowledge,
        rag_root: rag_root.into(),
    };
    Router::new()
        .route("/api/quizzes", get(list_quizzes).post(create_quiz))
        .route("/api/quizzes/{id}", get(get_quiz).delete(delete_quiz))
        .route("/api/quizzes/{id}/answers", post(submit_answer))
        .route("/api/quizzes/{id}/finish", post(finish_quiz))
        .with_state(state)
}

async fn generate_questions(
    state: &QuizState,
    quiz: QuizSession,
    llm: Option<CreateLlmConfig>,
) -> anyhow::Result<QuizSession> {
    let Some(kb) = state.knowledge.get(&quiz.kb_id) else {
        anyhow::bail!("knowledge base not found");
    };
    if kb.documents.is_empty() {
        anyhow::bail!("knowledge base has no documents");
    }

    let query = quiz
        .config
        .topic
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(&kb.name);
    let rag = tutor_rag::LanceDbRag::new(state.rag_root.clone(), kb.embedding);
    let hits = rag.search(Some(&quiz.kb_id), query, 12).await?;
    if hits.is_empty() {
        anyhow::bail!("no source chunks found for quiz generation");
    }
    let questions = if let Some(llm) = llm.and_then(llm_config_from_request) {
        let sources = hits
            .iter()
            .map(|hit| tutor_agent::quiz::QuizSourceChunk {
                source: hit.source.clone(),
                text: hit.text.clone(),
                score: hit.score,
            })
            .collect::<Vec<_>>();
        let generated = tutor_agent::quiz::generate_quiz_questions(
            &llm,
            &tutor_agent::quiz::QuizGenerationConfig {
                topic: quiz.config.topic.clone(),
                difficulty: format!("{:?}", quiz.config.difficulty).to_ascii_lowercase(),
                question_count: quiz.config.question_count,
            },
            &sources,
        )
        .await?;
        questions_from_generated(&quiz.config, &hits, generated)
    } else {
        questions_from_hits(&quiz.config, &hits)
    };
    state.store.replace_questions(&quiz.id, questions)
}

fn llm_config_from_request(config: CreateLlmConfig) -> Option<tutor_agent::LlmConfig> {
    let api_key = config.api_key?.trim().to_string();
    if api_key.is_empty() || config.model.trim().is_empty() {
        return None;
    }
    let provider = match config.provider.trim().to_ascii_lowercase().as_str() {
        "anthropic" | "claude" => tutor_agent::LlmProviderKind::Anthropic,
        "deepseek" => tutor_agent::LlmProviderKind::DeepSeek,
        "openai" | "openai-compatible" => tutor_agent::LlmProviderKind::OpenAI,
        _ => return None,
    };
    Some(tutor_agent::LlmConfig::from_parts(
        provider,
        config.model.trim().to_string(),
        api_key,
        config.base_url.filter(|value| !value.trim().is_empty()),
        config.chat_path.filter(|value| !value.trim().is_empty()),
        None,
    ))
}

fn questions_from_generated(
    config: &QuizConfig,
    hits: &[tutor_rag::SearchHit],
    generated: Vec<tutor_agent::quiz::GeneratedQuizQuestion>,
) -> Vec<QuizQuestion> {
    generated
        .into_iter()
        .enumerate()
        .map(|(index, question)| {
            let citations = question
                .citation_indices
                .iter()
                .filter_map(|source_index| hits.get(*source_index))
                .map(|hit| QuizCitation {
                    source: hit.source.clone(),
                    text: hit.text.clone(),
                    score: hit.score,
                })
                .collect::<Vec<_>>();
            let option_ids = ["A", "B", "C", "D", "E", "F"];
            QuizQuestion {
                id: format!("q{}", index + 1),
                question_type: QuizQuestionType::SingleChoice,
                stem: question.stem,
                options: question
                    .options
                    .into_iter()
                    .enumerate()
                    .map(|(option_index, text)| QuizOption {
                        id: option_ids
                            .get(option_index)
                            .copied()
                            .unwrap_or("Z")
                            .to_string(),
                        text,
                    })
                    .collect(),
                correct_option_id: option_ids
                    .get(question.correct_option_index)
                    .copied()
                    .unwrap_or("A")
                    .to_string(),
                explanation: question.explanation,
                citations,
                tags: question.tags,
                difficulty: config.difficulty.clone(),
            }
        })
        .collect()
}

fn questions_from_hits(config: &QuizConfig, hits: &[tutor_rag::SearchHit]) -> Vec<QuizQuestion> {
    let count = config.question_count.clamp(1, 10).min(hits.len().max(1));
    let topic = config
        .topic
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("the selected material");
    (0..count)
        .filter_map(|index| {
            let hit = hits.get(index % hits.len())?;
            let snippet = compact_text(&hit.text, 220);
            let distractor = hits
                .iter()
                .find(|candidate| candidate.id != hit.id)
                .map(|candidate| compact_text(&candidate.text, 140))
                .unwrap_or_else(|| "A statement that is not supported by the cited source.".into());
            Some(QuizQuestion {
                id: format!("q{}", index + 1),
                question_type: QuizQuestionType::SingleChoice,
                stem: format!("According to the cited material, which statement is best supported about {topic}?"),
                options: vec![
                    QuizOption {
                        id: "A".into(),
                        text: snippet.clone(),
                    },
                    QuizOption {
                        id: "B".into(),
                        text: "The cited material says this topic is unrelated to the knowledge base.".into(),
                    },
                    QuizOption {
                        id: "C".into(),
                        text: distractor,
                    },
                    QuizOption {
                        id: "D".into(),
                        text: "The answer cannot be inferred from any retrieved source.".into(),
                    },
                ],
                correct_option_id: "A".into(),
                explanation: format!("Option A is grounded in the retrieved source chunk from {}.", hit.source),
                citations: vec![QuizCitation {
                    source: hit.source.clone(),
                    text: hit.text.clone(),
                    score: hit.score,
                }],
                tags: vec![topic.to_string(), hit.source.clone()],
                difficulty: config.difficulty.clone(),
            })
        })
        .collect()
}

fn compact_text(text: &str, max_chars: usize) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= max_chars {
        return normalized;
    }
    let mut out = normalized.chars().take(max_chars).collect::<String>();
    out.push_str("...");
    out
}

fn error_response(err_status: StatusCode, err: impl std::fmt::Display) -> axum::response::Response {
    (
        err_status,
        Json(serde_json::json!({ "error": err.to_string() })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use chrono::Utc;
    use tower::ServiceExt;

    #[tokio::test]
    async fn creates_and_answers_quiz() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(QuizStore::new_with_path(dir.path().join("quizzes.json")));
        let knowledge = KnowledgeStore::new_with_path(dir.path().join("knowledge-bases.json"));
        let embedding = tutor_rag::EmbeddingConfig {
            provider: "local-test".into(),
            model: "hash".into(),
            api_key: "test-key".into(),
            base_url: None,
            embeddings_path: None,
            dimensions: Some(32),
            send_dimensions: false,
        };
        let kb = knowledge.create("Quiz KB", embedding.clone()).unwrap();
        knowledge
            .add_document(
                &kb.id,
                crate::knowledge_store::KnowledgeDocument {
                    id: "doc-1".into(),
                    name: "source.md".into(),
                    source: "source.md".into(),
                    index_source: None,
                    size_bytes: 64,
                    chunks: 1,
                    mime_type: Some("text/markdown".into()),
                    content_path: None,
                    file_path: None,
                    created_at: Utc::now(),
                },
            )
            .unwrap();
        let rag_root = dir.path().join("rag");
        tutor_rag::LanceDbRag::new(&rag_root, embedding)
            .ingest_text(
                &kb.id,
                "source.md",
                "OPC corrects lithography mask patterns before wafer exposure.",
            )
            .await
            .unwrap();
        let app = quiz_router_with_rag_root(store, knowledge, rag_root);

        let create = serde_json::json!({
            "kb_id": kb.id,
            "topic": "OPC",
            "question_count": 1
        });
        let response = app
            .clone()
            .oneshot(json_request(Method::POST, "/api/quizzes", create))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = body_json(response).await;
        let quiz_id = body["quiz"]["id"].as_str().unwrap();
        let question_id = body["quiz"]["questions"][0]["id"].as_str().unwrap();

        let answer = serde_json::json!({
            "question_id": question_id,
            "selected_option_id": "A"
        });
        let response = app
            .oneshot(json_request(
                Method::POST,
                &format!("/api/quizzes/{quiz_id}/answers"),
                answer,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = body_json(response).await;
        assert_eq!(body["quiz"]["score"]["correct"], 1);
    }

    fn json_request(method: Method, uri: &str, body: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    async fn body_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }
}
