use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    body::Body,
    extract::{Multipart, Path, State},
    http::{StatusCode, header},
    response::IntoResponse,
    routing::{delete, get, post},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::knowledge_store::{
    KnowledgeBaseView, KnowledgeDocument, KnowledgeStore, normalize_embedding_config,
};

#[derive(Clone)]
struct KnowledgeState {
    store: Arc<KnowledgeStore>,
    jobs: Arc<IngestionJobs>,
}

#[derive(Default)]
struct IngestionJobs {
    items: Mutex<HashMap<String, IngestionJob>>,
}

#[derive(Clone, Serialize)]
struct IngestionJob {
    id: String,
    status: IngestionJobStatus,
    stage: String,
    message: String,
    progress: u8,
    chunks: Option<usize>,
    knowledge_base: Option<KnowledgeBaseView>,
    error: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum IngestionJobStatus {
    Queued,
    Running,
    Done,
    Error,
}

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

async fn list_knowledge_bases(State(state): State<KnowledgeState>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(serde_json::json!({ "knowledge_bases": state.store.list() })),
    )
}

async fn create_knowledge_base(
    State(state): State<KnowledgeState>,
    Json(req): Json<CreateKnowledgeBaseRequest>,
) -> impl IntoResponse {
    match state
        .store
        .create(req.name, normalize_embedding_config(req.embedding))
    {
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
    State(state): State<KnowledgeState>,
    Path(kb): Path<String>,
) -> impl IntoResponse {
    if let Some(item) = state.store.get(&kb) {
        let rag = tutor_rag::LanceDbRag::new(tutor_rag::LanceDbRag::default_root(), item.embedding);
        if let Err(err) = rag.delete_kb(&kb).await {
            return error_response(err);
        }
    }

    match state.store.delete(&kb) {
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
    State(state): State<KnowledgeState>,
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

    if state.store.get(&kb).is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "knowledge base not found" })),
        )
            .into_response();
    }

    let source = req.source.unwrap_or_else(|| "pasted-text".to_string());
    let size_bytes = req.text.len();
    let job = state.jobs.create();
    let job_id = job.id.clone();
    let task_state = state.clone();
    tokio::spawn(async move {
        update_job(&task_state.jobs, &job_id, "parse", "Preparing pasted text", 12, None);
        let result =
            ingest_text_document(&task_state, &job_id, &kb, &source, &req.text, size_bytes).await;
        finish_job(&task_state.jobs, &job_id, result);
    });

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "job": job })),
    )
        .into_response()
}

async fn upload_document(
    State(state): State<KnowledgeState>,
    Path(kb): Path<String>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    if state.store.get(&kb).is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "knowledge base not found" })),
        )
            .into_response();
    }

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
        let normalized_content_type = upload_mime_type(&file_name, content_type.as_deref());
        let job = state.jobs.create();
        let job_id = job.id.clone();
        let task_state = state.clone();
        tokio::spawn(async move {
            let result = ingest_upload_document(
                &task_state,
                &job_id,
                &kb,
                &file_name,
                size_bytes,
                bytes.to_vec(),
                content_type,
                normalized_content_type,
            )
            .await;
            finish_job(&task_state.jobs, &job_id, result);
        });
        return (
            StatusCode::ACCEPTED,
            Json(serde_json::json!({ "job": job })),
        )
            .into_response();
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

fn upload_mime_type(file_name: &str, content_type: Option<&str>) -> Option<String> {
    if is_pdf_upload(file_name, content_type) {
        return Some("application/pdf".to_string());
    }
    content_type.map(str::to_string)
}

async fn ingest_text_document(
    state: &KnowledgeState,
    job_id: &str,
    kb: &str,
    source: &str,
    text: &str,
    size_bytes: usize,
) -> anyhow::Result<(KnowledgeBaseView, usize)> {
    let Some(item) = state.store.get(kb) else {
        anyhow::bail!("knowledge base not found");
    };

    let document_id = uuid::Uuid::new_v4().to_string();
    let index_source = document_index_source(&document_id, source);
    let rag = tutor_rag::LanceDbRag::new(tutor_rag::LanceDbRag::default_root(), item.embedding);
    let jobs = state.jobs.clone();
    let job_id_for_progress = job_id.to_string();
    let chunks = rag
        .ingest_text_with_progress(kb, &index_source, text, move |progress| {
            update_job(
                &jobs,
                &job_id_for_progress,
                &format!("{:?}", progress.stage).to_ascii_lowercase(),
                &progress.message,
                progress.progress,
                progress.chunks,
            );
        })
        .await?;
    update_job(
        &state.jobs,
        job_id,
        "store",
        "Saving document metadata",
        96,
        Some(chunks),
    );
    let content_path = state.store.store_document_text(kb, &document_id, text)?;
    let document = KnowledgeDocument {
        id: document_id,
        name: source.to_string(),
        source: source.to_string(),
        index_source: Some(index_source),
        size_bytes,
        chunks,
        mime_type: Some("text/plain; charset=utf-8".to_string()),
        content_path: Some(content_path),
        file_path: None,
        created_at: Utc::now(),
    };
    Ok((state.store.add_document(kb, document)?, chunks))
}

async fn ingest_upload_document(
    state: &KnowledgeState,
    job_id: &str,
    kb: &str,
    source: &str,
    size_bytes: usize,
    original_bytes: Vec<u8>,
    content_type: Option<String>,
    mime_type: Option<String>,
) -> anyhow::Result<(KnowledgeBaseView, usize)> {
    update_job(state.jobs.as_ref(), job_id, "parse", "Parsing uploaded file", 18, None);
    let text = extract_upload_text(source, content_type.as_deref(), original_bytes.as_slice())?;
    if text.trim().is_empty() {
        anyhow::bail!("document text is empty");
    }

    let (updated, chunks) =
        ingest_text_document(state, job_id, kb, source, &text, size_bytes).await?;
    let document_id = updated
        .documents
        .first()
        .map(|doc| doc.id.clone())
        .ok_or_else(|| anyhow::anyhow!("ingested document is missing"))?;
    let file_path = state
        .store
        .store_document_file(kb, &document_id, source, original_bytes.as_slice())?;
    let updated = state
        .store
        .update_document_file_metadata(kb, &document_id, file_path, mime_type)?;
    Ok((updated, chunks))
}

fn error_response(err: impl std::fmt::Display) -> axum::response::Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": err.to_string() })),
    )
        .into_response()
}

async fn get_document_file(
    State(state): State<KnowledgeState>,
    Path((kb, document_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let Some(item) = state.store.get(&kb) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "knowledge base not found" })),
        )
            .into_response();
    };
    let Some(document) = item.documents.iter().find(|doc| doc.id == document_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "document not found" })),
        )
            .into_response();
    };

    match state.store.document_file(&kb, &document_id) {
        Ok(Some(bytes)) => {
            let content_type = document
                .mime_type
                .as_deref()
                .unwrap_or("application/octet-stream");
            let mut response = Body::from(bytes).into_response();
            let headers = response.headers_mut();
            headers.insert(
                header::CONTENT_TYPE,
                content_type
                    .parse()
                    .unwrap_or(header::HeaderValue::from_static("application/octet-stream")),
            );
            headers.insert(
                header::CONTENT_DISPOSITION,
                format!("inline; filename=\"{}\"", document.name.replace('"', ""))
                    .parse()
                    .unwrap_or(header::HeaderValue::from_static("inline")),
            );
            response
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "document file not found" })),
        )
            .into_response(),
        Err(err) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": err.to_string() })),
        )
            .into_response(),
    }
}

async fn get_document_content(
    State(state): State<KnowledgeState>,
    Path((kb, document_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.store.document_text(&kb, &document_id) {
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
    State(state): State<KnowledgeState>,
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

    let Some(item) = state.store.get(&kb) else {
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

async fn get_ingest_job(
    State(state): State<KnowledgeState>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    match state.jobs.get(&job_id) {
        Some(job) => (StatusCode::OK, Json(serde_json::json!({ "job": job }))).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "ingestion job not found" })),
        )
            .into_response(),
    }
}

async fn delete_document(
    State(state): State<KnowledgeState>,
    Path((kb, document_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let Some(item) = state.store.get(&kb) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "knowledge base not found" })),
        )
            .into_response();
    };
    let Some(document) = item.documents.iter().find(|doc| doc.id == document_id).cloned() else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "document not found" })),
        )
            .into_response();
    };

    let rag = tutor_rag::LanceDbRag::new(tutor_rag::LanceDbRag::default_root(), item.embedding);
    if let Err(err) = rag.delete_source(&kb, document.index_source()).await {
        return error_response(err);
    }

    match state.store.delete_document(&kb, &document_id) {
        Ok(Some(_)) => (
            StatusCode::OK,
            Json(serde_json::json!({ "knowledge_base": state.store.get(&kb).map(KnowledgeBaseView::from) })),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "document not found" })),
        )
            .into_response(),
        Err(err) => error_response(err),
    }
}

async fn reindex_document(
    State(state): State<KnowledgeState>,
    Path((kb, document_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let Some(item) = state.store.get(&kb) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "knowledge base not found" })),
        )
            .into_response();
    };
    let Some(document) = item.documents.iter().find(|doc| doc.id == document_id).cloned() else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "document not found" })),
        )
            .into_response();
    };
    let text = match state.store.document_text(&kb, &document_id) {
        Ok(Some(text)) => text,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "document content not found" })),
            )
                .into_response();
        }
        Err(err) => return error_response(err),
    };

    let job = state.jobs.create();
    let job_id = job.id.clone();
    let task_state = state.clone();
    tokio::spawn(async move {
        let result = async {
            let rag = tutor_rag::LanceDbRag::new(
                tutor_rag::LanceDbRag::default_root(),
                item.embedding,
            );
            update_job(
                &task_state.jobs,
                &job_id,
                "delete",
                "Removing old chunks",
                18,
                None,
            );
            rag.delete_source(&kb, document.index_source()).await?;

            let jobs = task_state.jobs.clone();
            let job_id_for_progress = job_id.clone();
            let chunks = rag
                .ingest_text_with_progress(&kb, document.index_source(), &text, move |progress| {
                    update_job(
                        &jobs,
                        &job_id_for_progress,
                        &format!("{:?}", progress.stage).to_ascii_lowercase(),
                        &progress.message,
                        progress.progress,
                        progress.chunks,
                    );
                })
                .await?;
            update_job(
                &task_state.jobs,
                &job_id,
                "store",
                "Updating document metadata",
                96,
                Some(chunks),
            );
            Ok((task_state.store.update_document_chunks(&kb, &document_id, chunks)?, chunks))
        }
        .await;
        finish_job(&task_state.jobs, &job_id, result);
    });

    (
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "job": job })),
    )
        .into_response()
}

async fn get_document_chunks(
    State(state): State<KnowledgeState>,
    Path((kb, document_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let Some(item) = state.store.get(&kb) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "knowledge base not found" })),
        )
            .into_response();
    };
    let Some(document) = item.documents.iter().find(|doc| doc.id == document_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "document not found" })),
        )
            .into_response();
    };

    let rag = tutor_rag::LanceDbRag::new(tutor_rag::LanceDbRag::default_root(), item.embedding);
    match rag.chunks_for_source(&kb, document.index_source(), 200).await {
        Ok(chunks) => (
            StatusCode::OK,
            Json(serde_json::json!({ "kb": kb, "document_id": document_id, "chunks": chunks })),
        )
            .into_response(),
        Err(err) => error_response(err),
    }
}

impl IngestionJobs {
    fn create(&self) -> IngestionJob {
        let job = IngestionJob {
            id: uuid::Uuid::new_v4().to_string(),
            status: IngestionJobStatus::Queued,
            stage: "queued".into(),
            message: "Queued".into(),
            progress: 0,
            chunks: None,
            knowledge_base: None,
            error: None,
        };
        self.items.lock().unwrap().insert(job.id.clone(), job.clone());
        job
    }

    fn get(&self, id: &str) -> Option<IngestionJob> {
        self.items.lock().unwrap().get(id).cloned()
    }
}

fn update_job(
    jobs: &IngestionJobs,
    id: &str,
    stage: &str,
    message: &str,
    progress: u8,
    chunks: Option<usize>,
) {
    if let Some(job) = jobs.items.lock().unwrap().get_mut(id) {
        job.status = IngestionJobStatus::Running;
        job.stage = stage.to_string();
        job.message = message.to_string();
        job.progress = progress.min(99);
        job.chunks = chunks.or(job.chunks);
    }
}

fn finish_job(
    jobs: &IngestionJobs,
    id: &str,
    result: anyhow::Result<(KnowledgeBaseView, usize)>,
) {
    if let Some(job) = jobs.items.lock().unwrap().get_mut(id) {
        match result {
            Ok((knowledge_base, chunks)) => {
                job.status = IngestionJobStatus::Done;
                job.stage = "done".into();
                job.message = "Done".into();
                job.progress = 100;
                job.chunks = Some(chunks);
                job.knowledge_base = Some(knowledge_base);
                job.error = None;
            }
            Err(err) => {
                job.status = IngestionJobStatus::Error;
                job.stage = "error".into();
                job.message = err.to_string();
                job.error = Some(err.to_string());
            }
        }
    }
}

fn document_index_source(document_id: &str, source: &str) -> String {
    format!("{document_id}::{source}")
}

trait DocumentIndexSource {
    fn index_source(&self) -> &str;
}

impl DocumentIndexSource for KnowledgeDocument {
    fn index_source(&self) -> &str {
        self.index_source.as_deref().unwrap_or(&self.source)
    }
}

pub fn knowledge_router(store: Arc<KnowledgeStore>) -> Router {
    let state = KnowledgeState {
        store,
        jobs: Arc::new(IngestionJobs::default()),
    };
    Router::new()
        .route(
            "/api/knowledge-bases",
            get(list_knowledge_bases).post(create_knowledge_base),
        )
        .route("/api/knowledge-bases/{kb}", delete(delete_knowledge_base))
        .route("/api/ingest-jobs/{job_id}", get(get_ingest_job))
        .route("/api/knowledge-bases/{kb}/documents", post(ingest_document))
        .route(
            "/api/knowledge-bases/{kb}/documents/upload",
            post(upload_document),
        )
        .route(
            "/api/knowledge-bases/{kb}/documents/{document_id}/content",
            get(get_document_content),
        )
        .route(
            "/api/knowledge-bases/{kb}/documents/{document_id}/file",
            get(get_document_file),
        )
        .route(
            "/api/knowledge-bases/{kb}/documents/{document_id}",
            delete(delete_document),
        )
        .route(
            "/api/knowledge-bases/{kb}/documents/{document_id}/reindex",
            post(reindex_document),
        )
        .route(
            "/api/knowledge-bases/{kb}/documents/{document_id}/chunks",
            get(get_document_chunks),
        )
        .route("/api/knowledge-bases/{kb}/search", post(search_knowledge))
        .with_state(state)
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
