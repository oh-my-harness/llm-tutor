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
}
