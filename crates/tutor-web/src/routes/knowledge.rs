use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use chrono::Utc;
use serde::Deserialize;

use crate::knowledge_store::{KnowledgeDocument, KnowledgeStore, normalize_embedding_config};

#[derive(Deserialize)]
struct CreateKnowledgeBaseRequest {
    name: String,
    embedding: tutor_rag::EmbeddingConfig,
}

#[derive(Deserialize)]
struct IngestDocumentRequest {
    source: Option<String>,
    text: String,
}

#[derive(Deserialize)]
struct SearchRequest {
    query: String,
    top_k: Option<usize>,
}

async fn list_knowledge_bases(State(store): State<Arc<KnowledgeStore>>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({ "knowledge_bases": store.list() })),
    )
}

async fn create_knowledge_base(
    State(store): State<Arc<KnowledgeStore>>,
    Json(req): Json<CreateKnowledgeBaseRequest>,
) -> impl IntoResponse {
    match store.create(req.name, normalize_embedding_config(req.embedding)) {
        Ok(item) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "knowledge_base": item })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

async fn delete_knowledge_base(
    State(store): State<Arc<KnowledgeStore>>,
    Path(kb): Path<String>,
) -> impl IntoResponse {
    match store.delete(&kb) {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "knowledge base not found" })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

async fn ingest_document(
    State(store): State<Arc<KnowledgeStore>>,
    Path(kb): Path<String>,
    Json(req): Json<IngestDocumentRequest>,
) -> impl IntoResponse {
    if req.text.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "document text is empty" })),
        )
            .into_response();
    }

    let Some(item) = store.get(&kb) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "knowledge base not found" })),
        )
            .into_response();
    };

    let source = req.source.as_deref().unwrap_or("pasted-text");
    let rag = tutor_rag::LanceDbRag::new(tutor_rag::LanceDbRag::default_root(), item.embedding);
    match rag.ingest_text(&kb, source, &req.text).await {
        Ok(chunks) => {
            let document_id = uuid::Uuid::new_v4().to_string();
            let content_path = match store.store_document_text(&kb, &document_id, &req.text) {
                Ok(path) => path,
                Err(err) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({ "error": err.to_string() })),
                    )
                        .into_response();
                }
            };
            let document = KnowledgeDocument {
                id: document_id,
                name: source.to_string(),
                source: source.to_string(),
                size_bytes: req.text.len(),
                chunks,
                content_path: Some(content_path),
                created_at: Utc::now(),
            };
            match store.add_document(&kb, document) {
                Ok(updated) => (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "kb": kb,
                        "chunks": chunks,
                        "knowledge_base": updated,
                    })),
                )
                    .into_response(),
                Err(err) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": err.to_string() })),
                )
                    .into_response(),
            }
        }
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

async fn get_document_content(
    State(store): State<Arc<KnowledgeStore>>,
    Path((kb, document_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match store.document_text(&kb, &document_id) {
        Ok(Some(text)) => (
            StatusCode::OK,
            Json(serde_json::json!({ "kb": kb, "document_id": document_id, "text": text })),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "document content not found" })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

async fn search_knowledge(
    State(store): State<Arc<KnowledgeStore>>,
    Path(kb): Path<String>,
    Json(req): Json<SearchRequest>,
) -> impl IntoResponse {
    if req.query.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "query is empty" })),
        )
            .into_response();
    }

    let Some(item) = store.get(&kb) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "knowledge base not found" })),
        )
            .into_response();
    };

    let rag = tutor_rag::LanceDbRag::new(tutor_rag::LanceDbRag::default_root(), item.embedding);
    match tutor_rag::KnowledgeRetriever::search(&rag, Some(&kb), &req.query, req.top_k.unwrap_or(5))
        .await
    {
        Ok(hits) => (
            StatusCode::OK,
            Json(serde_json::json!({ "kb": kb, "hits": hits })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

pub fn knowledge_router(store: Arc<KnowledgeStore>) -> Router {
    Router::new()
        .route(
            "/api/knowledge-bases",
            get(list_knowledge_bases).post(create_knowledge_base),
        )
        .route("/api/knowledge-bases/{kb}", delete(delete_knowledge_base))
        .route("/api/knowledge-bases/{kb}/documents", post(ingest_document))
        .route(
            "/api/knowledge-bases/{kb}/documents/{document_id}/content",
            get(get_document_content),
        )
        .route("/api/knowledge-bases/{kb}/search", post(search_knowledge))
        .with_state(store)
}
