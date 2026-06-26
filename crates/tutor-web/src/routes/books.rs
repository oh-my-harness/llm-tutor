use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::Deserialize;

use crate::book_store::BookStore;

#[derive(Deserialize)]
struct CreateBookRequest {
    title: String,
    description: Option<String>,
}

#[derive(Deserialize)]
struct CreateChapterRequest {
    title: String,
    markdown: String,
    source_report_id: Option<String>,
    source_notebook_entry_id: Option<String>,
    source_session_id: Option<String>,
}

async fn list_books(State(store): State<Arc<BookStore>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({ "books": store.list() })),
    )
}

async fn create_book(
    State(store): State<Arc<BookStore>>,
    Json(req): Json<CreateBookRequest>,
) -> impl IntoResponse {
    match store.create(req.title, req.description) {
        Ok(book) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "book": book })),
        )
            .into_response(),
        Err(err) => error_response(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn create_chapter(
    State(store): State<Arc<BookStore>>,
    Path(book_id): Path<String>,
    Json(req): Json<CreateChapterRequest>,
) -> impl IntoResponse {
    match store.add_chapter(
        &book_id,
        req.title,
        req.markdown,
        req.source_report_id,
        req.source_notebook_entry_id,
        req.source_session_id,
    ) {
        Ok(book) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "book": book })),
        )
            .into_response(),
        Err(err) if err.to_string().contains("not found") => {
            error_response(StatusCode::NOT_FOUND, err.to_string())
        }
        Err(err) => error_response(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

pub fn books_router(store: Arc<BookStore>) -> Router {
    Router::new()
        .route("/api/books", get(list_books).post(create_book))
        .route("/api/books/{book_id}/chapters", post(create_chapter))
        .with_state(store)
}

fn error_response(status: StatusCode, message: String) -> axum::response::Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::{Method, Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn creates_book_and_chapter() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(BookStore::new_with_path(dir.path().join("books.json")));
        let app = books_router(store);
        let response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/books",
                serde_json::json!({ "title": "Research" }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response_json(response).await;
        let book_id = body["book"]["id"].as_str().unwrap();

        let response = app
            .oneshot(json_request(
                Method::POST,
                &format!("/api/books/{book_id}/chapters"),
                serde_json::json!({
                    "title": "Report",
                    "markdown": "# Report",
                    "source_notebook_entry_id": "notebook-1",
                    "source_session_id": "session-1"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response_json(response).await;
        assert_eq!(body["book"]["chapters"][0]["title"], "Report");
    }

    fn json_request(method: Method, uri: &str, value: serde_json::Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(value.to_string()))
            .unwrap()
    }

    async fn response_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }
}
