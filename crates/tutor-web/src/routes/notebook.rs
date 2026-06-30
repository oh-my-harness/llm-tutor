use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use serde::Deserialize;

use crate::memory_store::{MemoryEventCategory, MemoryStore};
use crate::notebook_store::{
    NotebookEntryInput, NotebookEntryType, NotebookEntryUpdate, NotebookStore,
};

#[derive(Clone)]
struct NotebookState {
    store: Arc<NotebookStore>,
    memory: Arc<MemoryStore>,
}

#[derive(Deserialize)]
struct ListNotebookQuery {
    space_id: Option<String>,
}

#[derive(Deserialize)]
struct CreateNotebookEntryRequest {
    space_id: Option<String>,
    entry_type: Option<NotebookEntryType>,
    title: String,
    markdown: String,
    metadata: Option<serde_json::Value>,
    source_session_id: Option<String>,
    source_message_id: Option<String>,
}

#[derive(Deserialize)]
struct UpdateNotebookEntryRequest {
    title: Option<String>,
    markdown: Option<String>,
    metadata: Option<serde_json::Value>,
    source_session_id: Option<String>,
    source_message_id: Option<String>,
}

async fn list_entries(
    State(state): State<NotebookState>,
    Query(query): Query<ListNotebookQuery>,
) -> impl IntoResponse {
    let space_id = query.space_id.as_deref();
    (
        StatusCode::OK,
        Json(serde_json::json!({ "entries": state.store.list(space_id) })),
    )
}

async fn get_entry(
    State(state): State<NotebookState>,
    Path(entry_id): Path<String>,
) -> impl IntoResponse {
    match state.store.get(&entry_id) {
        Some(entry) => {
            (StatusCode::OK, Json(serde_json::json!({ "entry": entry }))).into_response()
        }
        None => error_response(StatusCode::NOT_FOUND, "notebook entry not found".into()),
    }
}

async fn create_entry(
    State(state): State<NotebookState>,
    Json(req): Json<CreateNotebookEntryRequest>,
) -> impl IntoResponse {
    match state.store.create(NotebookEntryInput {
        space_id: req.space_id,
        entry_type: req.entry_type.unwrap_or(NotebookEntryType::Note),
        title: req.title,
        markdown: req.markdown,
        metadata: req.metadata,
        source_session_id: req.source_session_id,
        source_message_id: req.source_message_id,
    }) {
        Ok(entry) => {
            let _ = state.memory.record_event(
                MemoryEventCategory::Notebook,
                "created",
                format!("Created notebook entry: {}", entry.title),
                Some(entry.id.clone()),
                serde_json::json!({
                    "entry_type": entry.entry_type,
                    "space_id": entry.space_id,
                    "source_session_id": entry.source_session_id,
                }),
            );
            (
                StatusCode::CREATED,
                Json(serde_json::json!({ "entry": entry })),
            )
                .into_response()
        }
        Err(err) => error_response(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn update_entry(
    State(state): State<NotebookState>,
    Path(entry_id): Path<String>,
    Json(req): Json<UpdateNotebookEntryRequest>,
) -> impl IntoResponse {
    match state.store.update(
        &entry_id,
        NotebookEntryUpdate {
            title: req.title,
            markdown: req.markdown,
            metadata: req.metadata,
            source_session_id: req.source_session_id,
            source_message_id: req.source_message_id,
        },
    ) {
        Ok(entry) => {
            let _ = state.memory.record_event(
                MemoryEventCategory::Notebook,
                "updated",
                format!("Updated notebook entry: {}", entry.title),
                Some(entry.id.clone()),
                serde_json::json!({
                    "entry_type": entry.entry_type,
                    "space_id": entry.space_id,
                    "metadata": entry.metadata,
                }),
            );
            (StatusCode::OK, Json(serde_json::json!({ "entry": entry }))).into_response()
        }
        Err(err) if err.to_string().contains("not found") => {
            error_response(StatusCode::NOT_FOUND, err.to_string())
        }
        Err(err) => error_response(StatusCode::BAD_REQUEST, err.to_string()),
    }
}

async fn delete_entry(
    State(state): State<NotebookState>,
    Path(entry_id): Path<String>,
) -> impl IntoResponse {
    let existing = state.store.get(&entry_id);
    if state.store.delete(&entry_id) {
        let _ = state.memory.record_event(
            MemoryEventCategory::Notebook,
            "deleted",
            format!(
                "Deleted notebook entry: {}",
                existing
                    .as_ref()
                    .map(|entry| entry.title.as_str())
                    .unwrap_or(&entry_id)
            ),
            Some(entry_id),
            serde_json::json!({}),
        );
        StatusCode::NO_CONTENT.into_response()
    } else {
        error_response(StatusCode::NOT_FOUND, "notebook entry not found".into())
    }
}

pub fn notebook_router(store: Arc<NotebookStore>, memory: Arc<MemoryStore>) -> Router {
    let state = NotebookState { store, memory };
    Router::new()
        .route(
            "/api/notebook/entries",
            get(list_entries).post(create_entry),
        )
        .route(
            "/api/notebook/entries/{entry_id}",
            get(get_entry).patch(update_entry).delete(delete_entry),
        )
        .with_state(state)
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
    async fn creates_lists_and_deletes_notebook_entry() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(NotebookStore::new_with_path(
            dir.path().join("notebook.json"),
        ));
        let memory = Arc::new(MemoryStore::new_with_root(dir.path().join("memory")));
        let app = notebook_router(store, memory.clone());
        let response = app
            .clone()
            .oneshot(json_request(
                Method::POST,
                "/api/notebook/entries",
                serde_json::json!({
                    "title": "Report",
                    "entry_type": "research_report",
                    "markdown": "# Report"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response_json(response).await;
        let entry_id = body["entry"]["id"].as_str().unwrap();

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/notebook/entries")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["entries"].as_array().unwrap().len(), 1);

        let response = app
            .clone()
            .oneshot(json_request(
                Method::PATCH,
                &format!("/api/notebook/entries/{entry_id}"),
                serde_json::json!({
                    "title": "Updated report",
                    "markdown": "# Updated report"
                }),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["entry"]["title"], "Updated report");
        assert_eq!(body["entry"]["markdown"], "# Updated report");

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::DELETE)
                    .uri(format!("/api/notebook/entries/{entry_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(!memory.recent_events(10).unwrap().is_empty());
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
