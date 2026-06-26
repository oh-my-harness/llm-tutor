use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Book {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub chapters: Vec<BookChapter>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookChapter {
    pub id: String,
    pub title: String,
    pub markdown: String,
    #[serde(default)]
    pub source_report_id: Option<String>,
    #[serde(default)]
    pub source_notebook_entry_id: Option<String>,
    #[serde(default)]
    pub source_session_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct BookStore {
    path: PathBuf,
    items: Mutex<Vec<Book>>,
}

impl BookStore {
    pub fn new() -> Self {
        Self::new_with_path(default_root().join("books.json"))
    }

    pub fn new_with_path(path: PathBuf) -> Self {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("failed to create book store directory");
        }
        let items = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<Book>>(&text).ok())
            .unwrap_or_default();
        Self {
            path,
            items: Mutex::new(items),
        }
    }

    pub fn list(&self) -> Vec<Book> {
        let mut items = self.items.lock().unwrap().clone();
        items.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        items
    }

    pub fn create(&self, title: String, description: Option<String>) -> Result<Book> {
        let title = normalize_title(&title);
        let now = Utc::now();
        let book = Book {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            description: description.filter(|value| !value.trim().is_empty()),
            chapters: Vec::new(),
            created_at: now,
            updated_at: now,
        };
        let mut items = self.items.lock().unwrap();
        items.push(book.clone());
        self.save_locked(&items)?;
        Ok(book)
    }

    pub fn add_chapter(
        &self,
        book_id: &str,
        title: String,
        markdown: String,
        source_report_id: Option<String>,
        source_notebook_entry_id: Option<String>,
        source_session_id: Option<String>,
    ) -> Result<Book> {
        if markdown.trim().is_empty() {
            return Err(anyhow!("chapter markdown is empty"));
        }
        let mut items = self.items.lock().unwrap();
        let Some(book) = items.iter_mut().find(|item| item.id == book_id) else {
            return Err(anyhow!("book not found"));
        };
        let now = Utc::now();
        book.chapters.push(BookChapter {
            id: uuid::Uuid::new_v4().to_string(),
            title: normalize_title(&title),
            markdown,
            source_report_id: source_report_id.filter(|value| !value.trim().is_empty()),
            source_notebook_entry_id: source_notebook_entry_id
                .filter(|value| !value.trim().is_empty()),
            source_session_id: source_session_id.filter(|value| !value.trim().is_empty()),
            created_at: now,
            updated_at: now,
        });
        book.updated_at = now;
        let updated = book.clone();
        self.save_locked(&items)?;
        Ok(updated)
    }

    fn save_locked(&self, items: &[Book]) -> Result<()> {
        fs::write(&self.path, serde_json::to_string_pretty(items)?)?;
        Ok(())
    }
}

impl Default for BookStore {
    fn default() -> Self {
        Self::new()
    }
}

fn default_root() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".llm-tutor")
}

fn normalize_title(title: &str) -> String {
    let title = title.trim();
    if title.is_empty() {
        "Untitled".to_string()
    } else {
        title.chars().take(120).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn book_store_creates_book_and_chapter() {
        let dir = tempfile::tempdir().unwrap();
        let store = BookStore::new_with_path(dir.path().join("books.json"));
        let book = store.create("Research Notes".into(), None).unwrap();
        let book = store
            .add_chapter(
                &book.id,
                "Chapter 1".into(),
                "# Report".into(),
                None,
                Some("notebook-1".into()),
                Some("session-1".into()),
            )
            .unwrap();
        assert_eq!(book.chapters.len(), 1);
        assert_eq!(
            store.list()[0].chapters[0].source_notebook_entry_id,
            Some("notebook-1".into())
        );
        assert_eq!(
            store.list()[0].chapters[0].source_session_id,
            Some("session-1".into())
        );
    }
}
