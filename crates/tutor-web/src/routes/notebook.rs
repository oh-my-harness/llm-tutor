use std::io::{Cursor, Read};
use std::path::Path;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Multipart, Path as AxumPath, Query, State},
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
        Json(serde_json::json!({ "entries": state.store.list_views(space_id) })),
    )
}

async fn get_entry(
    State(state): State<NotebookState>,
    AxumPath(entry_id): AxumPath<String>,
) -> impl IntoResponse {
    match state.store.get_view(&entry_id) {
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
    AxumPath(entry_id): AxumPath<String>,
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
    AxumPath(entry_id): AxumPath<String>,
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

async fn import_entries(
    State(state): State<NotebookState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut space_id = None;
    let mut imported = Vec::new();
    let mut skipped = Vec::new();

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(field)) => field,
            Ok(None) => break,
            Err(err) => return error_response(StatusCode::BAD_REQUEST, err.to_string()),
        };

        match field.name() {
            Some("space_id") => match field.text().await {
                Ok(value) => {
                    if !value.trim().is_empty() {
                        space_id = Some(value.trim().to_string());
                    }
                }
                Err(err) => return error_response(StatusCode::BAD_REQUEST, err.to_string()),
            },
            Some("file") | Some("files") => {
                let file_name = field.file_name().unwrap_or("notebook.md").to_string();
                let bytes = match field.bytes().await {
                    Ok(bytes) => bytes,
                    Err(err) => return error_response(StatusCode::BAD_REQUEST, err.to_string()),
                };
                match import_payloads_from_upload(&file_name, bytes.as_ref()) {
                    Ok(payloads) => {
                        if payloads.is_empty() {
                            skipped.push(serde_json::json!({
                                "file_name": file_name,
                                "reason": "no markdown files found",
                            }));
                            continue;
                        }
                        for payload in payloads {
                            match state.store.create(NotebookEntryInput {
                                space_id: space_id.clone(),
                                entry_type: NotebookEntryType::Note,
                                title: payload.title,
                                markdown: payload.markdown,
                                metadata: Some(payload.metadata),
                                source_session_id: None,
                                source_message_id: None,
                            }) {
                                Ok(entry) => imported.push(entry),
                                Err(err) => skipped.push(serde_json::json!({
                                    "file_name": payload.source_path,
                                    "reason": err.to_string(),
                                })),
                            }
                        }
                    }
                    Err(err) => skipped.push(serde_json::json!({
                        "file_name": file_name,
                        "reason": err.to_string(),
                    })),
                }
            }
            _ => {}
        }
    }

    if imported.is_empty() && skipped.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "missing file field".into());
    }

    if !imported.is_empty() {
        let _ = state.memory.record_event(
            MemoryEventCategory::Notebook,
            "imported",
            format!("Imported {} notebook entries", imported.len()),
            None,
            serde_json::json!({
                "entry_ids": imported.iter().map(|entry| entry.id.as_str()).collect::<Vec<_>>(),
                "skipped_count": skipped.len(),
            }),
        );
    }

    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "entries": imported,
            "skipped": skipped,
        })),
    )
        .into_response()
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
        .route("/api/notebook/import", axum::routing::post(import_entries))
        .with_state(state)
}

fn error_response(status: StatusCode, message: String) -> axum::response::Response {
    (status, Json(serde_json::json!({ "error": message }))).into_response()
}

#[derive(Debug)]
struct ImportPayload {
    source_path: String,
    title: String,
    markdown: String,
    metadata: serde_json::Value,
}

fn import_payloads_from_upload(
    file_name: &str,
    bytes: &[u8],
) -> anyhow::Result<Vec<ImportPayload>> {
    if file_name.to_lowercase().ends_with(".zip") {
        return import_payloads_from_zip(file_name, bytes);
    }
    if !file_name.to_lowercase().ends_with(".md")
        && !file_name.to_lowercase().ends_with(".markdown")
    {
        anyhow::bail!("unsupported notebook import file type");
    }
    let markdown = decode_utf8(bytes)?;
    Ok(vec![payload_from_markdown(file_name, markdown)?])
}

fn import_payloads_from_zip(file_name: &str, bytes: &[u8]) -> anyhow::Result<Vec<ImportPayload>> {
    let reader = Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader)?;
    let mut payloads = Vec::new();
    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        if file.is_dir() {
            continue;
        }
        let name = file.name().replace('\\', "/");
        if !name.to_lowercase().ends_with(".md") && !name.to_lowercase().ends_with(".markdown") {
            continue;
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        let markdown = decode_utf8(&bytes)?;
        payloads.push(payload_from_markdown(&name, markdown)?);
    }
    payloads.sort_by(|a, b| a.source_path.cmp(&b.source_path));
    if payloads.is_empty() {
        anyhow::bail!("{file_name} contains no markdown files");
    }
    Ok(payloads)
}

fn payload_from_markdown(source_path: &str, markdown: String) -> anyhow::Result<ImportPayload> {
    if markdown.trim().is_empty() {
        anyhow::bail!("markdown file is empty");
    }
    let frontmatter = parse_frontmatter(&markdown);
    let title = frontmatter
        .as_ref()
        .and_then(|value| value.get("title"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| title_from_path(source_path));
    let mut metadata = serde_json::json!({
        "import": {
            "source_path": source_path,
            "format": "markdown",
        },
        "frontmatter": frontmatter.unwrap_or_else(|| serde_json::json!({})),
    });
    if let Some(tags) = metadata["frontmatter"].get("tags").cloned() {
        metadata["tags"] = tags;
    }
    if let Some(tag) = metadata["frontmatter"].get("tag").cloned() {
        metadata["tag"] = tag;
    }
    Ok(ImportPayload {
        source_path: source_path.to_string(),
        title,
        markdown,
        metadata,
    })
}

fn parse_frontmatter(markdown: &str) -> Option<serde_json::Value> {
    let normalized = markdown
        .strip_prefix("---\r\n")
        .or_else(|| markdown.strip_prefix("---\n"))?;
    let frontmatter_start = markdown.len() - normalized.len();
    let mut offset = frontmatter_start;
    for line in normalized.split_inclusive('\n') {
        let line_text = line.trim_end_matches(['\r', '\n']);
        if line_text.trim() == "---" {
            let yaml = &markdown[frontmatter_start..offset];
            return serde_yaml::from_str::<serde_yaml::Value>(yaml)
                .ok()
                .and_then(|value| serde_json::to_value(value).ok());
        }
        offset += line.len();
    }
    None
}

fn title_from_path(source_path: &str) -> String {
    let normalized = source_path.replace('\\', "/");
    let file_name = normalized.rsplit('/').next().unwrap_or(source_path);
    Path::new(file_name)
        .file_stem()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Imported note")
        .to_string()
}

fn decode_utf8(bytes: &[u8]) -> anyhow::Result<String> {
    String::from_utf8(bytes.to_vec()).map_err(|_| anyhow::anyhow!("file is not valid UTF-8"))
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

    #[tokio::test]
    async fn imports_markdown_entries_from_multipart() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(NotebookStore::new_with_path(
            dir.path().join("notebook.json"),
        ));
        let memory = Arc::new(MemoryStore::new_with_root(dir.path().join("memory")));
        let app = notebook_router(store, memory.clone());
        let markdown = "---\ntitle: Imported OPC\ntags:\n  - optics\n  - opc\n---\n# Imported OPC\n\nSee [[Lithography]].\n";
        let body = multipart_body("BOUNDARY", "opc.md", "text/markdown", markdown.as_bytes());

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/notebook/import")
                    .header("content-type", "multipart/form-data; boundary=BOUNDARY")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
        let body = response_json(response).await;
        assert_eq!(body["entries"].as_array().unwrap().len(), 1);
        assert_eq!(body["entries"][0]["title"], "Imported OPC");
        assert_eq!(body["skipped"].as_array().unwrap().len(), 0);

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/notebook/entries")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = response_json(response).await;
        assert_eq!(body["entries"][0]["tags"][0], "opc");
        assert_eq!(body["entries"][0]["tags"][1], "optics");
        assert_eq!(body["entries"][0]["links"][0]["target"], "Lithography");
        assert!(!memory.recent_events(10).unwrap().is_empty());
    }

    #[test]
    fn parses_markdown_frontmatter_and_zip_imports() {
        let markdown = "---\r\ntitle: Process Notes\ntags: lithography opc\r\n---\r\n# Body";
        let payload = payload_from_markdown("folder/process.md", markdown.into()).unwrap();
        assert_eq!(payload.title, "Process Notes");
        assert_eq!(payload.metadata["frontmatter"]["tags"], "lithography opc");

        let mut zip_bytes = Cursor::new(Vec::new());
        {
            let mut writer = zip::ZipWriter::new(&mut zip_bytes);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            writer.start_file("notes/a.md", options).unwrap();
            std::io::Write::write_all(&mut writer, b"# A").unwrap();
            writer.start_file("assets/image.png", options).unwrap();
            std::io::Write::write_all(&mut writer, b"not markdown").unwrap();
            writer.finish().unwrap();
        }
        let payloads = import_payloads_from_upload("vault.zip", zip_bytes.get_ref()).unwrap();
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].title, "a");
        assert_eq!(payloads[0].source_path, "notes/a.md");
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

    fn multipart_body(
        boundary: &str,
        file_name: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> Vec<u8> {
        let mut body = Vec::new();
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\n")
                .as_bytes(),
        );
        body.extend_from_slice(format!("Content-Type: {content_type}\r\n\r\n").as_bytes());
        body.extend_from_slice(bytes);
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        body
    }
}
