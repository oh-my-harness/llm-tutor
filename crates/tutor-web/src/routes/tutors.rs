use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::Deserialize;

use crate::settings_store::SettingsStore;
use crate::tutor_memory_store::{
    CreateTutorMemoryEntry, TutorMemoryError, TutorMemoryStore, UpdateTutorMemoryEntry,
};
use crate::tutor_store::{CreateTutorProfile, TutorStore, TutorStoreError, UpdateTutorProfile};

#[derive(Clone)]
struct TutorsState {
    store: Arc<TutorStore>,
    settings: Arc<SettingsStore>,
    memory: Arc<TutorMemoryStore>,
}

#[derive(Default, Deserialize)]
struct ListQuery {
    #[serde(default)]
    include_archived: bool,
}

#[derive(Default, Deserialize)]
struct MemoryListQuery {
    #[serde(default)]
    include_resolved: bool,
}

#[derive(Default, Deserialize)]
struct ResolveMemoryRequest {
    #[serde(default)]
    resolution_note: Option<String>,
}

pub fn tutors_router(
    store: Arc<TutorStore>,
    settings: Arc<SettingsStore>,
    memory: Arc<TutorMemoryStore>,
) -> Router {
    Router::new()
        .route("/api/tutors", get(list_tutors).post(create_tutor))
        .route(
            "/api/tutors/{id}",
            get(get_tutor).patch(update_tutor).delete(delete_tutor),
        )
        .route("/api/tutors/{id}/reset-profile", post(reset_profile))
        .route(
            "/api/tutors/{id}/memory",
            get(list_tutor_memory).post(create_tutor_memory),
        )
        .route(
            "/api/tutors/{id}/memory/{entry_id}",
            get(get_tutor_memory)
                .patch(update_tutor_memory)
                .delete(delete_tutor_memory),
        )
        .route(
            "/api/tutors/{id}/memory/{entry_id}/resolve",
            post(resolve_tutor_memory),
        )
        .route("/api/tutors/{id}/reset-memory", post(reset_tutor_memory))
        .with_state(TutorsState {
            store,
            settings,
            memory,
        })
}

async fn list_tutors(
    State(state): State<TutorsState>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    Json(serde_json::json!({
        "tutors": state.store.list(query.include_archived),
    }))
}

async fn create_tutor(
    State(state): State<TutorsState>,
    Json(input): Json<CreateTutorProfile>,
) -> impl IntoResponse {
    if let Some(config_id) = input.default_model_config_id.as_deref()
        && !state.settings.has_llm_config(config_id)
    {
        return store_error(TutorStoreError::Validation(
            "default model configuration does not exist".into(),
        ));
    }
    match state.store.create(input) {
        Ok(tutor) => (StatusCode::CREATED, Json(serde_json::json!(tutor))).into_response(),
        Err(error) => store_error(error),
    }
}

async fn get_tutor(State(state): State<TutorsState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.store.get(&id) {
        Some(tutor) => (StatusCode::OK, Json(serde_json::json!(tutor))).into_response(),
        None => store_error(TutorStoreError::NotFound),
    }
}

async fn update_tutor(
    State(state): State<TutorsState>,
    Path(id): Path<String>,
    Json(input): Json<UpdateTutorProfile>,
) -> impl IntoResponse {
    if let Some(Some(config_id)) = input.default_model_config_id.as_ref()
        && !state.settings.has_llm_config(config_id)
    {
        return store_error(TutorStoreError::Validation(
            "default model configuration does not exist".into(),
        ));
    }
    match state.store.update(&id, input) {
        Ok(tutor) => (StatusCode::OK, Json(serde_json::json!(tutor))).into_response(),
        Err(error) => store_error(error),
    }
}

async fn delete_tutor(
    State(state): State<TutorsState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.store.archive(&id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => store_error(error),
    }
}

async fn reset_profile(
    State(state): State<TutorsState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.store.reset_general_tutor(&id) {
        Ok(tutor) => (StatusCode::OK, Json(serde_json::json!(tutor))).into_response(),
        Err(error) => store_error(error),
    }
}

async fn list_tutor_memory(
    State(state): State<TutorsState>,
    Path(id): Path<String>,
    Query(query): Query<MemoryListQuery>,
) -> impl IntoResponse {
    if state.store.get(&id).is_none() {
        return store_error(TutorStoreError::NotFound);
    }
    match state.memory.list(&id, query.include_resolved) {
        Ok(entries) => Json(serde_json::json!({ "entries": entries })).into_response(),
        Err(error) => memory_error(error),
    }
}

async fn create_tutor_memory(
    State(state): State<TutorsState>,
    Path(id): Path<String>,
    Json(mut input): Json<CreateTutorMemoryEntry>,
) -> impl IntoResponse {
    if state.store.get(&id).is_none() {
        return store_error(TutorStoreError::NotFound);
    }
    input.source_session_id = None;
    input.source_message_id = None;
    match state.memory.create(&id, input) {
        Ok(entry) => (StatusCode::CREATED, Json(serde_json::json!(entry))).into_response(),
        Err(error) => memory_error(error),
    }
}

async fn get_tutor_memory(
    State(state): State<TutorsState>,
    Path((id, entry_id)): Path<(String, String)>,
) -> impl IntoResponse {
    if state.store.get(&id).is_none() {
        return store_error(TutorStoreError::NotFound);
    }
    match state.memory.get(&id, &entry_id) {
        Ok(entry) => Json(serde_json::json!(entry)).into_response(),
        Err(error) => memory_error(error),
    }
}

async fn update_tutor_memory(
    State(state): State<TutorsState>,
    Path((id, entry_id)): Path<(String, String)>,
    Json(input): Json<UpdateTutorMemoryEntry>,
) -> impl IntoResponse {
    if state.store.get(&id).is_none() {
        return store_error(TutorStoreError::NotFound);
    }
    match state.memory.update(&id, &entry_id, input) {
        Ok(entry) => Json(serde_json::json!(entry)).into_response(),
        Err(error) => memory_error(error),
    }
}

async fn resolve_tutor_memory(
    State(state): State<TutorsState>,
    Path((id, entry_id)): Path<(String, String)>,
    Json(input): Json<ResolveMemoryRequest>,
) -> impl IntoResponse {
    if state.store.get(&id).is_none() {
        return store_error(TutorStoreError::NotFound);
    }
    match state.memory.resolve(&id, &entry_id, input.resolution_note) {
        Ok(entry) => Json(serde_json::json!(entry)).into_response(),
        Err(error) => memory_error(error),
    }
}

async fn delete_tutor_memory(
    State(state): State<TutorsState>,
    Path((id, entry_id)): Path<(String, String)>,
) -> impl IntoResponse {
    if state.store.get(&id).is_none() {
        return store_error(TutorStoreError::NotFound);
    }
    match state.memory.delete(&id, &entry_id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => memory_error(error),
    }
}

async fn reset_tutor_memory(
    State(state): State<TutorsState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if state.store.get(&id).is_none() {
        return store_error(TutorStoreError::NotFound);
    }
    match state.memory.reset(&id) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(error) => memory_error(error),
    }
}

fn store_error(error: TutorStoreError) -> axum::response::Response {
    let status = match error {
        TutorStoreError::NotFound => StatusCode::NOT_FOUND,
        TutorStoreError::BuiltInTutor | TutorStoreError::Validation(_) => StatusCode::BAD_REQUEST,
        TutorStoreError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (
        status,
        Json(serde_json::json!({ "error": error.to_string() })),
    )
        .into_response()
}

fn memory_error(error: TutorMemoryError) -> axum::response::Response {
    let status = match error {
        TutorMemoryError::NotFound => StatusCode::NOT_FOUND,
        TutorMemoryError::Validation(_) => StatusCode::BAD_REQUEST,
        TutorMemoryError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (
        status,
        Json(serde_json::json!({ "error": error.to_string() })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn tutor_routes_create_update_and_archive() {
        let dir = tempfile::tempdir().unwrap();
        let app = tutors_router(
            Arc::new(TutorStore::new_with_root(dir.path())),
            Arc::new(SettingsStore::new_with_path(
                dir.path().join("settings.json"),
            )),
            Arc::new(TutorMemoryStore::new_with_root(dir.path())),
        );

        let create = app
            .clone()
            .oneshot(
                Request::post("/api/tutors")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r##"{"name":"Math Tutor","soul_markdown":"# Identity\n\nTeach math"}"##,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create.status(), StatusCode::CREATED);
        let body = axum::body::to_bytes(create.into_body(), usize::MAX)
            .await
            .unwrap();
        let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let id = created["id"].as_str().unwrap();

        let update = app
            .clone()
            .oneshot(
                Request::patch(format!("/api/tutors/{id}"))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r##"{"soul_markdown":"# Identity\n\nTeach geometry"}"##,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(update.status(), StatusCode::OK);

        let dangling_model = app
            .clone()
            .oneshot(
                Request::post("/api/tutors")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r##"{"name":"Invalid","soul_markdown":"# Identity\n\nTeach","default_model_config_id":"missing"}"##,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(dangling_model.status(), StatusCode::BAD_REQUEST);

        let delete = app
            .oneshot(
                Request::delete(format!("/api/tutors/{id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(delete.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn tutor_memory_routes_manage_only_existing_tutors() {
        let dir = tempfile::tempdir().unwrap();
        let app = tutors_router(
            Arc::new(TutorStore::new_with_root(dir.path())),
            Arc::new(SettingsStore::new_with_path(
                dir.path().join("settings.json"),
            )),
            Arc::new(TutorMemoryStore::new_with_root(dir.path())),
        );

        let create = app
            .clone()
            .oneshot(
                Request::post("/api/tutors/general-tutor/memory")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"kind":"commitment","text":"Prepare the next lesson"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create.status(), StatusCode::CREATED);
        let body = axum::body::to_bytes(create.into_body(), usize::MAX)
            .await
            .unwrap();
        let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let entry_id = created["id"].as_str().unwrap();

        let resolve = app
            .clone()
            .oneshot(
                Request::post(format!(
                    "/api/tutors/general-tutor/memory/{entry_id}/resolve"
                ))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"resolution_note":"Completed"}"#))
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resolve.status(), StatusCode::OK);

        let list = app
            .clone()
            .oneshot(
                Request::get("/api/tutors/general-tutor/memory?include_resolved=true")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = axum::body::to_bytes(list.into_body(), usize::MAX)
            .await
            .unwrap();
        let listed: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(listed["entries"][0]["status"], "resolved");

        let missing = app
            .oneshot(
                Request::get("/api/tutors/missing/memory")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
    }
}
