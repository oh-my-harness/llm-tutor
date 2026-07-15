use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::Deserialize;

use crate::tutor_store::{CreateTutorProfile, TutorStore, TutorStoreError, UpdateTutorProfile};

#[derive(Clone)]
struct TutorsState {
    store: Arc<TutorStore>,
}

#[derive(Default, Deserialize)]
struct ListQuery {
    #[serde(default)]
    include_archived: bool,
}

pub fn tutors_router(store: Arc<TutorStore>) -> Router {
    Router::new()
        .route("/api/tutors", get(list_tutors).post(create_tutor))
        .route(
            "/api/tutors/{id}",
            get(get_tutor).patch(update_tutor).delete(delete_tutor),
        )
        .route("/api/tutors/{id}/reset-profile", post(reset_profile))
        .with_state(TutorsState { store })
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn tutor_routes_create_update_and_archive() {
        let dir = tempfile::tempdir().unwrap();
        let app = tutors_router(Arc::new(TutorStore::new_with_root(dir.path())));

        let create = app
            .clone()
            .oneshot(
                Request::post("/api/tutors")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"name":"Math Tutor","role":"Teach math","goal":"Algebra"}"#,
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
                    .body(Body::from(r#"{"goal":"Geometry"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(update.status(), StatusCode::OK);

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
}
