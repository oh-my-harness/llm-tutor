use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::Deserialize;

use crate::quiz_store::{QuizConfig, QuizDifficulty, QuizQuestionType, QuizStore};

#[derive(Clone)]
struct QuizState {
    store: Arc<QuizStore>,
}

#[derive(Debug, Deserialize)]
struct CreateQuizRequest {
    title: Option<String>,
    kb_id: String,
    topic: Option<String>,
    difficulty: Option<QuizDifficulty>,
    question_count: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct SubmitAnswerRequest {
    question_id: String,
    selected_option_id: String,
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

    match state
        .store
        .create(req.title.unwrap_or_default(), req.kb_id, config)
    {
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

pub fn quiz_router(store: Arc<QuizStore>) -> Router {
    let state = QuizState { store };
    Router::new()
        .route("/api/quizzes", get(list_quizzes).post(create_quiz))
        .route("/api/quizzes/{id}", get(get_quiz).delete(delete_quiz))
        .route("/api/quizzes/{id}/answers", post(submit_answer))
        .route("/api/quizzes/{id}/finish", post(finish_quiz))
        .with_state(state)
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
    use tower::ServiceExt;

    #[tokio::test]
    async fn creates_and_answers_quiz() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(QuizStore::new_with_path(dir.path().join("quizzes.json")));
        let app = quiz_router(store);

        let create = serde_json::json!({
            "kb_id": "kb-1",
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
