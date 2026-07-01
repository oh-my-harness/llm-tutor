use std::io::{Cursor, Read};
use std::path::Path;
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Multipart, Path as AxumPath, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::IntoResponse,
    routing::get,
};
use serde::{Deserialize, Serialize};

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
struct ExportNotebookQuery {
    space_id: Option<String>,
    entry_ids: Option<String>,
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
    let import = match parse_import_multipart(&mut multipart).await {
        Ok(import) => import,
        Err(err) => return error_response(StatusCode::BAD_REQUEST, err.to_string()),
    };

    if import.payloads.is_empty() && import.skipped.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "missing file field".into());
    }

    let mut imported = Vec::new();
    let mut skipped = import.skipped;

    for payload in import.payloads {
        match state.store.create(NotebookEntryInput {
            space_id: import.space_id.clone(),
            entry_type: NotebookEntryType::Note,
            title: payload.title,
            markdown: payload.markdown,
            metadata: Some(payload.metadata),
            source_session_id: None,
            source_message_id: None,
        }) {
            Ok(entry) => imported.push(entry),
            Err(err) => skipped.push(ImportSkipped {
                file_name: payload.source_path,
                reason: err.to_string(),
            }),
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

async fn preview_import_entries(
    State(state): State<NotebookState>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let import = match parse_import_multipart(&mut multipart).await {
        Ok(import) => import,
        Err(err) => return error_response(StatusCode::BAD_REQUEST, err.to_string()),
    };

    if import.payloads.is_empty() && import.skipped.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "missing file field".into());
    }

    let existing = state.store.list(import.space_id.as_deref());
    let items = import
        .payloads
        .iter()
        .map(|payload| {
            let duplicate_title_entry = existing
                .iter()
                .find(|entry| normalized_key(&entry.title) == normalized_key(&payload.title));
            ImportPreviewItem {
                source_path: payload.source_path.clone(),
                title: payload.title.clone(),
                markdown_chars: payload.markdown.chars().count(),
                tags: crate::notebook_store::metadata_tags(Some(&payload.metadata)),
                duplicate_title_entry_id: duplicate_title_entry.map(|entry| entry.id.clone()),
                duplicate_title: duplicate_title_entry.map(|entry| entry.title.clone()),
            }
        })
        .collect::<Vec<_>>();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "items": items,
            "skipped": import.skipped,
            "conflict_count": items.iter().filter(|item| item.duplicate_title_entry_id.is_some()).count(),
        })),
    )
        .into_response()
}

async fn export_entry(
    State(state): State<NotebookState>,
    AxumPath(entry_id): AxumPath<String>,
) -> impl IntoResponse {
    let Some(entry) = state.store.get_view(&entry_id) else {
        return error_response(StatusCode::NOT_FOUND, "notebook entry not found".into());
    };
    let markdown = export_markdown(&entry);
    markdown_response(safe_markdown_file_name(&entry.entry.title), markdown)
}

async fn export_entries_zip(
    State(state): State<NotebookState>,
    Query(query): Query<ExportNotebookQuery>,
) -> impl IntoResponse {
    let mut entries = state.store.list_views(query.space_id.as_deref());
    if let Some(ids) = query.entry_ids {
        let requested = ids
            .split(',')
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        entries.retain(|entry| requested.iter().any(|id| id == &entry.entry.id));
    }
    if entries.is_empty() {
        return error_response(
            StatusCode::NOT_FOUND,
            "no notebook entries to export".into(),
        );
    }
    match export_zip(&entries) {
        Ok(bytes) => bytes_response("notebook-export.zip", "application/zip", bytes),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    }
}

async fn export_obsidian_vault_zip(
    State(state): State<NotebookState>,
    Query(query): Query<ExportNotebookQuery>,
) -> impl IntoResponse {
    let mut entries = state.store.list_views(query.space_id.as_deref());
    if let Some(ids) = query.entry_ids {
        let requested = ids
            .split(',')
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        entries.retain(|entry| requested.iter().any(|id| id == &entry.entry.id));
    }
    if entries.is_empty() {
        return error_response(
            StatusCode::NOT_FOUND,
            "no notebook entries to export".into(),
        );
    }
    match export_obsidian_vault(&entries) {
        Ok(bytes) => bytes_response("notebook-vault.zip", "application/zip", bytes),
        Err(err) => error_response(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
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
        .route(
            "/api/notebook/entries/{entry_id}/export.md",
            get(export_entry),
        )
        .route("/api/notebook/export.zip", get(export_entries_zip))
        .route(
            "/api/notebook/export-vault.zip",
            get(export_obsidian_vault_zip),
        )
        .route(
            "/api/notebook/import/preview",
            axum::routing::post(preview_import_entries),
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

#[derive(Debug)]
struct ParsedImport {
    space_id: Option<String>,
    payloads: Vec<ImportPayload>,
    skipped: Vec<ImportSkipped>,
}

#[derive(Debug, Serialize)]
struct ImportSkipped {
    file_name: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct ImportPreviewItem {
    source_path: String,
    title: String,
    markdown_chars: usize,
    tags: Vec<String>,
    duplicate_title_entry_id: Option<String>,
    duplicate_title: Option<String>,
}

async fn parse_import_multipart(multipart: &mut Multipart) -> anyhow::Result<ParsedImport> {
    let mut space_id = None;
    let mut payloads = Vec::new();
    let mut skipped = Vec::new();

    loop {
        let Some(field) = multipart.next_field().await? else {
            break;
        };

        match field.name() {
            Some("space_id") => {
                let value = field.text().await?;
                if !value.trim().is_empty() {
                    space_id = Some(value.trim().to_string());
                }
            }
            Some("file") | Some("files") => {
                let file_name = field.file_name().unwrap_or("notebook.md").to_string();
                let bytes = field.bytes().await?;
                match import_payloads_from_upload(&file_name, bytes.as_ref()) {
                    Ok(items) if items.is_empty() => skipped.push(ImportSkipped {
                        file_name,
                        reason: "no markdown files found".into(),
                    }),
                    Ok(items) => payloads.extend(items),
                    Err(err) => skipped.push(ImportSkipped {
                        file_name,
                        reason: err.to_string(),
                    }),
                }
            }
            _ => {}
        }
    }

    Ok(ParsedImport {
        space_id,
        payloads,
        skipped,
    })
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

fn export_markdown(entry: &crate::notebook_store::NotebookEntryView) -> String {
    let tags = if entry.tags.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::json!(entry.tags)
    };
    let frontmatter = serde_json::json!({
        "id": entry.entry.id,
        "title": entry.entry.title,
        "type": entry.entry.entry_type,
        "tags": tags,
        "source_session_id": entry.entry.source_session_id,
        "source_message_id": entry.entry.source_message_id,
        "created_at": entry.entry.created_at,
        "updated_at": entry.entry.updated_at,
    });
    let yaml = serde_yaml::to_string(&frontmatter).unwrap_or_default();
    format!(
        "---\n{}---\n\n{}",
        yaml.trim_start_matches("---\n"),
        strip_frontmatter(&entry.entry.markdown).trim_start()
    )
}

fn strip_frontmatter(markdown: &str) -> &str {
    let Some(normalized) = markdown
        .strip_prefix("---\r\n")
        .or_else(|| markdown.strip_prefix("---\n"))
    else {
        return markdown;
    };
    let mut consumed = markdown.len() - normalized.len();
    for line in normalized.split_inclusive('\n') {
        consumed += line.len();
        if line.trim_end_matches(['\r', '\n']).trim() == "---" {
            return &markdown[consumed..];
        }
    }
    markdown
}

fn export_zip(entries: &[crate::notebook_store::NotebookEntryView]) -> anyhow::Result<Vec<u8>> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut used_names = Vec::new();
    for entry in entries {
        let file_name = unique_file_name(
            &safe_markdown_file_name(&entry.entry.title),
            &mut used_names,
        );
        writer.start_file(file_name, options)?;
        std::io::Write::write_all(&mut writer, export_markdown(entry).as_bytes())?;
    }
    Ok(writer.finish()?.into_inner())
}

fn export_obsidian_vault(
    entries: &[crate::notebook_store::NotebookEntryView],
) -> anyhow::Result<Vec<u8>> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(cursor);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut used_names = Vec::new();

    writer.start_file(".obsidian/app.json", options)?;
    std::io::Write::write_all(
        &mut writer,
        br#"{"alwaysUpdateLinks":true,"newFileLocation":"root","attachmentFolderPath":"assets"}"#,
    )?;
    writer.start_file(".obsidian/appearance.json", options)?;
    std::io::Write::write_all(&mut writer, br#"{"theme":"obsidian"}"#)?;
    writer.start_file("README.md", options)?;
    std::io::Write::write_all(
        &mut writer,
        b"# Notebook Vault\n\nExported from Tutor Agent. Notes keep their YAML frontmatter and wiki links.\n",
    )?;

    for entry in entries {
        let file_name = unique_file_name(
            &safe_markdown_file_name(&entry.entry.title),
            &mut used_names,
        );
        writer.start_file(file_name, options)?;
        std::io::Write::write_all(&mut writer, export_markdown(entry).as_bytes())?;
    }
    Ok(writer.finish()?.into_inner())
}

fn markdown_response(file_name: String, markdown: String) -> axum::response::Response {
    bytes_response(
        file_name,
        "text/markdown; charset=utf-8",
        markdown.into_bytes(),
    )
}

fn bytes_response(
    file_name: impl AsRef<str>,
    content_type: &str,
    bytes: Vec<u8>,
) -> axum::response::Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type)
            .unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "attachment; filename=\"{}\"",
            file_name.as_ref().replace('"', "")
        ))
        .unwrap_or(HeaderValue::from_static("attachment")),
    );
    (StatusCode::OK, headers, bytes).into_response()
}

fn safe_markdown_file_name(title: &str) -> String {
    format!("{}.md", safe_file_stem(title))
}

fn safe_file_stem(title: &str) -> String {
    let name = title
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '-',
            ch if ch.is_control() => '-',
            ch => ch,
        })
        .collect::<String>()
        .trim()
        .trim_matches('.')
        .to_string();
    if name.is_empty() { "note".into() } else { name }
}

fn unique_file_name(base: &str, used_names: &mut Vec<String>) -> String {
    if !used_names
        .iter()
        .any(|name| name.eq_ignore_ascii_case(base))
    {
        used_names.push(base.to_string());
        return base.to_string();
    }
    let stem = base.strip_suffix(".md").unwrap_or(base);
    for index in 2.. {
        let candidate = format!("{stem}-{index}.md");
        if !used_names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&candidate))
        {
            used_names.push(candidate.clone());
            return candidate;
        }
    }
    unreachable!()
}

fn normalized_key(value: &str) -> String {
    value.trim().to_lowercase()
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
            .clone()
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
        let body = response_json(response).await;
        assert_eq!(body["entries"][0]["tags"][0], "opc");
        assert_eq!(body["entries"][0]["tags"][1], "optics");
        assert_eq!(body["entries"][0]["links"][0]["target"], "Lithography");
        assert!(!memory.recent_events(10).unwrap().is_empty());
    }

    #[tokio::test]
    async fn previews_import_conflicts_and_exports_notebook() {
        let dir = tempfile::tempdir().unwrap();
        let store = Arc::new(NotebookStore::new_with_path(
            dir.path().join("notebook.json"),
        ));
        let memory = Arc::new(MemoryStore::new_with_root(dir.path().join("memory")));
        let existing = store
            .create(NotebookEntryInput {
                space_id: Some("default".into()),
                entry_type: NotebookEntryType::Note,
                title: "Imported OPC".into(),
                markdown: "# Imported OPC\n\nExisting.".into(),
                metadata: None,
                source_session_id: None,
                source_message_id: None,
            })
            .unwrap();
        let app = notebook_router(store, memory);
        let markdown = "---\ntitle: Imported OPC\ntags: [opc]\n---\n# Imported OPC\n\nNew.";
        let body = multipart_body("BOUNDARY", "opc.md", "text/markdown", markdown.as_bytes());

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/api/notebook/import/preview")
                    .header("content-type", "multipart/form-data; boundary=BOUNDARY")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["conflict_count"], 1);
        assert_eq!(
            body["items"][0]["duplicate_title_entry_id"],
            serde_json::json!(existing.id)
        );

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri(format!("/api/notebook/entries/{}/export.md", existing.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let text = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(text.contains("title: Imported OPC"));
        assert!(text.contains("# Imported OPC"));

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/notebook/export.zip?space_id=default")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(archive.len(), 1);
        assert_eq!(archive.by_index(0).unwrap().name(), "Imported OPC.md");

        let response = app
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/api/notebook/export-vault.zip?space_id=default")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert!(archive.by_name(".obsidian/app.json").is_ok());
        assert!(archive.by_name(".obsidian/appearance.json").is_ok());
        assert!(archive.by_name("README.md").is_ok());
        assert!(archive.by_name("Imported OPC.md").is_ok());
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
