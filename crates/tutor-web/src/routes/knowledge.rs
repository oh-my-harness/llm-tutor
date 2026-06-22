use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
};
use chrono::Utc;
use serde::Deserialize;

use crate::knowledge_store::{
    KnowledgeBaseView, KnowledgeDocument, KnowledgeStore, normalize_embedding_config,
};

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

    let source = req.source.as_deref().unwrap_or("pasted-text");
    match ingest_text_document(store.as_ref(), &kb, source, &req.text, req.text.len()).await {
        Ok((updated, chunks)) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "kb": kb,
                "chunks": chunks,
                "knowledge_base": updated,
            })),
        )
            .into_response(),
        Err(err) => error_response(err),
    }
}

async fn upload_document(
    State(store): State<Arc<KnowledgeStore>>,
    Path(kb): Path<String>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() != Some("file") {
            continue;
        }

        let file_name = field.file_name().unwrap_or("uploaded.txt").to_string();
        let content_type = field.content_type().map(str::to_string);
        let bytes = match field.bytes().await {
            Ok(bytes) => bytes,
            Err(err) => return error_response(err),
        };
        let size_bytes = bytes.len();
        let text = match extract_upload_text(&file_name, content_type.as_deref(), bytes.as_ref()) {
            Ok(text) => text,
            Err(err) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({ "error": err.to_string() })),
                )
                    .into_response();
            }
        };

        if text.trim().is_empty() {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "document text is empty" })),
            )
                .into_response();
        }

        return match ingest_text_document(store.as_ref(), &kb, &file_name, &text, size_bytes).await
        {
            Ok((updated, chunks)) => (
                StatusCode::OK,
                Json(serde_json::json!({
                    "kb": kb,
                    "chunks": chunks,
                    "knowledge_base": updated,
                })),
            )
                .into_response(),
            Err(err) => error_response(err),
        };
    }

    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": "missing file field" })),
    )
        .into_response()
}

fn extract_upload_text(
    file_name: &str,
    content_type: Option<&str>,
    bytes: &[u8],
) -> anyhow::Result<String> {
    if is_pdf_upload(file_name, content_type) {
        let text = pdf_extract::extract_text_from_mem(bytes)?;
        if text.trim().is_empty() {
            anyhow::bail!("PDF text is empty or could not be extracted");
        }
        return Ok(text);
    }

    match String::from_utf8(bytes.to_vec()) {
        Ok(text) => Ok(text),
        Err(_) => {
            anyhow::bail!("only UTF-8 text attachments and PDF files are supported for now")
        }
    }
}

fn is_pdf_upload(file_name: &str, content_type: Option<&str>) -> bool {
    file_name
        .rsplit('.')
        .next()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("pdf"))
        || content_type.is_some_and(|value| value.eq_ignore_ascii_case("application/pdf"))
}

async fn ingest_text_document(
    store: &KnowledgeStore,
    kb: &str,
    source: &str,
    text: &str,
    size_bytes: usize,
) -> anyhow::Result<(KnowledgeBaseView, usize)> {
    let Some(item) = store.get(kb) else {
        anyhow::bail!("knowledge base not found");
    };

    let rag = tutor_rag::LanceDbRag::new(tutor_rag::LanceDbRag::default_root(), item.embedding);
    let chunks = rag.ingest_text(kb, source, text).await?;
    let document_id = uuid::Uuid::new_v4().to_string();
    let content_path = store.store_document_text(kb, &document_id, text)?;
    let document = KnowledgeDocument {
        id: document_id,
        name: source.to_string(),
        source: source.to_string(),
        size_bytes,
        chunks,
        content_path: Some(content_path),
        created_at: Utc::now(),
    };
    Ok((store.add_document(kb, document)?, chunks))
}

fn error_response(err: impl std::fmt::Display) -> axum::response::Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": err.to_string() })),
    )
        .into_response()
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
            "/api/knowledge-bases/{kb}/documents/upload",
            post(upload_document),
        )
        .route(
            "/api/knowledge-bases/{kb}/documents/{document_id}/content",
            get(get_document_content),
        )
        .route("/api/knowledge-bases/{kb}/search", post(search_knowledge))
        .with_state(store)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_pdf_upload_by_extension_or_content_type() {
        assert!(is_pdf_upload("lesson.PDF", None));
        assert!(is_pdf_upload("lesson.bin", Some("application/pdf")));
        assert!(!is_pdf_upload("lesson.txt", Some("text/plain")));
    }

    #[test]
    fn extracts_utf8_text_upload() {
        let text = extract_upload_text("lesson.md", Some("text/markdown"), b"# Title").unwrap();
        assert_eq!(text, "# Title");
    }

    #[test]
    fn rejects_non_utf8_non_pdf_upload() {
        let err = extract_upload_text("lesson.bin", None, &[0xff, 0xfe]).unwrap_err();
        assert!(err.to_string().contains("UTF-8 text attachments and PDF files"));
    }
}
