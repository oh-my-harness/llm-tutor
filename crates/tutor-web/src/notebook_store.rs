use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotebookEntryType {
    Note,
    ResearchReport,
    ChatAnswer,
    SourceSnippet,
    QuizSummary,
    DeepSolveResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookEntry {
    pub id: String,
    pub space_id: String,
    pub entry_type: NotebookEntryType,
    pub title: String,
    pub markdown: String,
    pub metadata: Option<serde_json::Value>,
    pub source_session_id: Option<String>,
    pub source_message_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotebookLink {
    pub raw: String,
    pub target: String,
    pub alias: Option<String>,
    pub target_id: Option<String>,
    pub target_title: Option<String>,
    pub resolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotebookBacklink {
    pub source_entry_id: String,
    pub source_title: String,
    pub raw: String,
    pub alias: Option<String>,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotebookEntryView {
    #[serde(flatten)]
    pub entry: NotebookEntry,
    pub tags: Vec<String>,
    pub links: Vec<NotebookLink>,
    pub backlinks: Vec<NotebookBacklink>,
}

pub struct NotebookStore {
    path: PathBuf,
    items: Mutex<Vec<NotebookEntry>>,
}

impl NotebookStore {
    pub fn new() -> Self {
        Self::new_with_path(default_root().join("notebook_entries.json"))
    }

    pub fn new_with_path(path: PathBuf) -> Self {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create notebook store directory");
        }
        let items = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<NotebookEntry>>(&text).ok())
            .unwrap_or_default();
        Self {
            path,
            items: Mutex::new(items),
        }
    }

    pub fn list(&self, space_id: Option<&str>) -> Vec<NotebookEntry> {
        let mut items: Vec<_> = self
            .items
            .lock()
            .unwrap()
            .iter()
            .filter(|item| space_id.is_none_or(|space_id| item.space_id == space_id))
            .cloned()
            .collect();
        items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        items
    }

    pub fn get(&self, id: &str) -> Option<NotebookEntry> {
        self.items
            .lock()
            .unwrap()
            .iter()
            .find(|item| item.id == id)
            .cloned()
    }

    pub fn list_views(&self, space_id: Option<&str>) -> Vec<NotebookEntryView> {
        let entries = self.list(space_id);
        entry_views(&entries)
    }

    pub fn get_view(&self, id: &str) -> Option<NotebookEntryView> {
        let entries = self.list(None);
        let entry = entries.iter().find(|item| item.id == id)?.clone();
        Some(entry_view(entry, &entries))
    }

    pub fn create(&self, input: NotebookEntryInput) -> Result<NotebookEntry> {
        if input.markdown.trim().is_empty() {
            return Err(anyhow!("notebook markdown is empty"));
        }
        let now = Utc::now();
        let entry = NotebookEntry {
            id: uuid::Uuid::new_v4().to_string(),
            space_id: normalize_space_id(input.space_id),
            entry_type: input.entry_type,
            title: normalize_title(&input.title),
            markdown: input.markdown,
            metadata: input.metadata,
            source_session_id: clean_optional(input.source_session_id),
            source_message_id: clean_optional(input.source_message_id),
            created_at: now,
            updated_at: now,
        };
        let mut items = self.items.lock().unwrap();
        items.push(entry.clone());
        self.save_locked(&items)?;
        Ok(entry)
    }

    pub fn update(&self, id: &str, input: NotebookEntryUpdate) -> Result<NotebookEntry> {
        let mut items = self.items.lock().unwrap();
        let Some(entry) = items.iter_mut().find(|item| item.id == id) else {
            return Err(anyhow!("notebook entry not found"));
        };
        if let Some(title) = input.title {
            entry.title = normalize_title(&title);
        }
        if let Some(markdown) = input.markdown {
            if markdown.trim().is_empty() {
                return Err(anyhow!("notebook markdown is empty"));
            }
            entry.markdown = markdown;
        }
        if let Some(metadata) = input.metadata {
            entry.metadata = Some(metadata);
        }
        if let Some(source_session_id) = input.source_session_id {
            entry.source_session_id = clean_optional(Some(source_session_id));
        }
        if let Some(source_message_id) = input.source_message_id {
            entry.source_message_id = clean_optional(Some(source_message_id));
        }
        entry.updated_at = Utc::now();
        let updated = entry.clone();
        self.save_locked(&items)?;
        Ok(updated)
    }

    pub fn delete(&self, id: &str) -> bool {
        let mut items = self.items.lock().unwrap();
        let before = items.len();
        items.retain(|item| item.id != id);
        let deleted = items.len() != before;
        if deleted {
            let _ = self.save_locked(&items);
        }
        deleted
    }

    fn save_locked(&self, items: &[NotebookEntry]) -> Result<()> {
        fs::write(&self.path, serde_json::to_string_pretty(items)?)?;
        Ok(())
    }
}

pub fn entry_views(entries: &[NotebookEntry]) -> Vec<NotebookEntryView> {
    entries
        .iter()
        .cloned()
        .map(|entry| entry_view(entry, entries))
        .collect()
}

pub fn entry_view(entry: NotebookEntry, entries: &[NotebookEntry]) -> NotebookEntryView {
    let tags = parse_tags(&entry.markdown);
    let links = parse_links(&entry.markdown)
        .into_iter()
        .map(|link| resolve_link(link, entries))
        .collect::<Vec<_>>();
    let backlinks = entries
        .iter()
        .filter(|source| source.id != entry.id)
        .flat_map(|source| backlinks_from_source(source, &entry, entries))
        .collect::<Vec<_>>();
    NotebookEntryView {
        entry,
        tags,
        links,
        backlinks,
    }
}

pub fn parse_links(markdown: &str) -> Vec<NotebookLink> {
    let mut links = Vec::new();
    let mut rest = markdown;
    while let Some(start) = rest.find("[[") {
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("]]") else {
            break;
        };
        let raw_inner = after_start[..end].trim();
        if !raw_inner.is_empty() {
            let (target, alias) = parse_link_inner(raw_inner);
            if !target.is_empty() {
                links.push(NotebookLink {
                    raw: format!("[[{raw_inner}]]"),
                    target,
                    alias,
                    target_id: None,
                    target_title: None,
                    resolved: false,
                });
            }
        }
        rest = &after_start[end + 2..];
    }
    dedupe_links(links)
}

pub fn parse_tags(markdown: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let chars = markdown.char_indices().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        let (byte_index, ch) = chars[index];
        if ch != '#' || is_escaped_hash(markdown, byte_index) {
            index += 1;
            continue;
        }
        if byte_index > 0 {
            let previous = markdown[..byte_index].chars().next_back();
            if previous.is_some_and(|value| value.is_alphanumeric() || value == '_') {
                index += 1;
                continue;
            }
        }
        let mut end = byte_index + ch.len_utf8();
        let mut next_index = index + 1;
        while next_index < chars.len() {
            let (next_byte, next_ch) = chars[next_index];
            if next_ch.is_alphanumeric() || matches!(next_ch, '_' | '-' | '/') {
                end = next_byte + next_ch.len_utf8();
                next_index += 1;
            } else {
                break;
            }
        }
        if end > byte_index + 1 {
            tags.push(markdown[byte_index + 1..end].to_string());
        }
        index = next_index.max(index + 1);
    }
    tags.sort_by_key(|tag| tag.to_lowercase());
    tags.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    tags
}

fn parse_link_inner(raw: &str) -> (String, Option<String>) {
    let mut parts = raw.splitn(2, '|');
    let target = parts.next().unwrap_or_default().trim().to_string();
    let alias = parts
        .next()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    (target, alias)
}

fn resolve_link(mut link: NotebookLink, entries: &[NotebookEntry]) -> NotebookLink {
    let target_key = normalize_lookup_key(&link.target);
    let resolved = entries
        .iter()
        .find(|entry| entry.id == link.target || normalize_lookup_key(&entry.title) == target_key);
    if let Some(entry) = resolved {
        link.target_id = Some(entry.id.clone());
        link.target_title = Some(entry.title.clone());
        link.resolved = true;
    }
    link
}

fn backlinks_from_source(
    source: &NotebookEntry,
    target: &NotebookEntry,
    entries: &[NotebookEntry],
) -> Vec<NotebookBacklink> {
    parse_links_with_positions(&source.markdown)
        .into_iter()
        .filter_map(|(link, start, end)| {
            let resolved = resolve_link(link, entries);
            if resolved.target_id.as_deref() != Some(target.id.as_str()) {
                return None;
            }
            Some(NotebookBacklink {
                source_entry_id: source.id.clone(),
                source_title: source.title.clone(),
                raw: resolved.raw,
                alias: resolved.alias,
                snippet: snippet_around(&source.markdown, start, end),
            })
        })
        .collect()
}

fn parse_links_with_positions(markdown: &str) -> Vec<(NotebookLink, usize, usize)> {
    let mut links = Vec::new();
    let mut offset = 0usize;
    let mut rest = markdown;
    while let Some(start) = rest.find("[[") {
        let absolute_start = offset + start;
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("]]") else {
            break;
        };
        let absolute_end = absolute_start + 2 + end + 2;
        let raw_inner = after_start[..end].trim();
        if !raw_inner.is_empty() {
            let (target, alias) = parse_link_inner(raw_inner);
            if !target.is_empty() {
                links.push((
                    NotebookLink {
                        raw: format!("[[{raw_inner}]]"),
                        target,
                        alias,
                        target_id: None,
                        target_title: None,
                        resolved: false,
                    },
                    absolute_start,
                    absolute_end,
                ));
            }
        }
        offset = absolute_end;
        rest = &markdown[offset..];
    }
    links
}

fn dedupe_links(links: Vec<NotebookLink>) -> Vec<NotebookLink> {
    let mut deduped = Vec::new();
    for link in links {
        let seen = deduped.iter().any(|existing: &NotebookLink| {
            normalize_lookup_key(&existing.target) == normalize_lookup_key(&link.target)
                && existing.alias == link.alias
        });
        if !seen {
            deduped.push(link);
        }
    }
    deduped
}

fn normalize_lookup_key(value: &str) -> String {
    value.trim().trim_matches('#').trim().to_lowercase()
}

fn snippet_around(markdown: &str, start: usize, end: usize) -> String {
    let left = markdown[..start].chars().rev().take(48).collect::<String>();
    let left = left.chars().rev().collect::<String>();
    let right = markdown[end..].chars().take(72).collect::<String>();
    format!("{left}{}{right}", &markdown[start..end])
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_escaped_hash(markdown: &str, byte_index: usize) -> bool {
    markdown[..byte_index]
        .chars()
        .rev()
        .take_while(|ch| *ch == '\\')
        .count()
        % 2
        == 1
}

impl Default for NotebookStore {
    fn default() -> Self {
        Self::new()
    }
}

pub struct NotebookEntryInput {
    pub space_id: Option<String>,
    pub entry_type: NotebookEntryType,
    pub title: String,
    pub markdown: String,
    pub metadata: Option<serde_json::Value>,
    pub source_session_id: Option<String>,
    pub source_message_id: Option<String>,
}

pub struct NotebookEntryUpdate {
    pub title: Option<String>,
    pub markdown: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub source_session_id: Option<String>,
    pub source_message_id: Option<String>,
}

fn default_root() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".llm-tutor")
}

fn normalize_space_id(space_id: Option<String>) -> String {
    space_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "default".to_string())
}

fn normalize_title(title: &str) -> String {
    let title = title.trim();
    if title.is_empty() {
        "Untitled note".to_string()
    } else {
        title.chars().take(120).collect()
    }
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notebook_store_creates_updates_and_deletes_entry() {
        let dir = tempfile::tempdir().unwrap();
        let store = NotebookStore::new_with_path(dir.path().join("notebook.json"));
        let entry = store
            .create(NotebookEntryInput {
                space_id: None,
                entry_type: NotebookEntryType::ResearchReport,
                title: " Report ".into(),
                markdown: "# Report".into(),
                metadata: None,
                source_session_id: Some("session-1".into()),
                source_message_id: None,
            })
            .unwrap();

        assert_eq!(entry.space_id, "default");
        assert_eq!(store.list(Some("default")).len(), 1);

        let updated = store
            .update(
                &entry.id,
                NotebookEntryUpdate {
                    title: Some("Updated".into()),
                    markdown: Some("# Updated".into()),
                    metadata: None,
                    source_session_id: None,
                    source_message_id: None,
                },
            )
            .unwrap();
        assert_eq!(updated.title, "Updated");
        assert!(store.delete(&entry.id));
        assert!(store.list(Some("default")).is_empty());
    }

    #[test]
    fn parses_wiki_links_and_tags() {
        let links = parse_links("See [[Lithography]] and [[note-1|OPC notes]].");
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].target, "Lithography");
        assert_eq!(links[1].target, "note-1");
        assert_eq!(links[1].alias.as_deref(), Some("OPC notes"));

        let tags = parse_tags("Study #lithography and #weak-point. Ignore email#a and \\#escaped.");
        assert_eq!(tags, vec!["lithography", "weak-point"]);
    }

    #[test]
    fn entry_view_resolves_links_and_backlinks() {
        let target = NotebookEntry {
            id: "target-1".into(),
            space_id: "default".into(),
            entry_type: NotebookEntryType::Note,
            title: "Lithography".into(),
            markdown: "# Lithography\n\n#process".into(),
            metadata: None,
            source_session_id: None,
            source_message_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let source = NotebookEntry {
            id: "source-1".into(),
            space_id: "default".into(),
            entry_type: NotebookEntryType::Note,
            title: "OPC".into(),
            markdown: "OPC is related to [[Lithography|litho]].".into(),
            metadata: None,
            source_session_id: None,
            source_message_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let entries = vec![target.clone(), source.clone()];
        let target_view = entry_view(target, &entries);
        let source_view = entry_view(source, &entries);

        assert_eq!(target_view.tags, vec!["process"]);
        assert_eq!(target_view.backlinks.len(), 1);
        assert_eq!(target_view.backlinks[0].source_title, "OPC");
        assert!(
            target_view.backlinks[0]
                .snippet
                .contains("[[Lithography|litho]]")
        );
        assert_eq!(source_view.links.len(), 1);
        assert!(source_view.links[0].resolved);
        assert_eq!(source_view.links[0].target_id.as_deref(), Some("target-1"));
    }
}
