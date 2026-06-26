use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;

const DEFAULT_FILES: &[(&str, &str)] = &[
    ("L2/chat.md", "# Chat memory\n\n"),
    ("L2/quiz.md", "# Quiz memory\n\n"),
    ("L2/notebook.md", "# Notebook memory\n\n"),
    ("L2/research.md", "# Research memory\n\n"),
    ("L3/recent.md", "# Recent learning context\n\n"),
    (
        "L3/profile.md",
        "# Student profile\n\n## Strengths\n\n## Weaknesses\n\n",
    ),
    ("L3/scope.md", "# Learning scope\n\n"),
    ("L3/preferences.md", "# Learning preferences\n\n"),
    (
        "L3/teaching_strategy.md",
        "# Teaching strategy\n\n## Preferred approach\n\n",
    ),
];

const DEFAULT_DIRS: &[&str] = &["L1", "L2", "L3"];

#[derive(Clone)]
pub struct MemoryStore {
    root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryFile {
    pub path: String,
    pub level: String,
    pub name: String,
    pub markdown: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEvent {
    pub id: String,
    pub category: MemoryEventCategory,
    pub action: String,
    pub summary: String,
    pub source_id: Option<String>,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryEventCategory {
    Chat,
    Quiz,
    Notebook,
    Research,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConsolidationPreview {
    pub target_path: String,
    pub proposed_markdown: String,
    pub event_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryAssistResult {
    pub target_path: String,
    pub action: MemoryAssistAction,
    pub report_markdown: String,
    pub proposed_markdown: Option<String>,
    pub changed: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryAssistAction {
    Update,
    Check,
    Dedupe,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryMarker {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemorySourceRef {
    pub index: usize,
    pub target: String,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::new_with_root(default_root().join("memory"))
    }

    pub fn new_with_root(root: PathBuf) -> Self {
        let store = Self { root };
        store
            .ensure_skeleton()
            .expect("failed to create memory directory skeleton");
        store
    }

    pub fn list(&self) -> Result<Vec<MemoryFile>> {
        self.ensure_skeleton()?;
        let mut files = Vec::new();
        for (path, _) in DEFAULT_FILES {
            files.push(self.read(path)?);
        }
        Ok(files)
    }

    pub fn read(&self, path: &str) -> Result<MemoryFile> {
        self.ensure_skeleton()?;
        let path = normalize_memory_path(path)?;
        let full_path = self.root.join(&path);
        let markdown = fs::read_to_string(&full_path)?;
        let level = path
            .parent()
            .and_then(Path::file_name)
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_string();
        Ok(MemoryFile {
            path: path_to_slash(&path),
            level,
            name,
            markdown,
        })
    }

    pub fn write(&self, path: &str, markdown: String) -> Result<MemoryFile> {
        self.ensure_skeleton()?;
        let path = normalize_memory_path(path)?;
        if markdown.trim().is_empty() {
            return Err(anyhow!("memory markdown is empty"));
        }
        fs::write(self.root.join(&path), markdown)?;
        self.read(&path_to_slash(&path))
    }

    pub fn record_event(
        &self,
        category: MemoryEventCategory,
        action: impl Into<String>,
        summary: impl Into<String>,
        source_id: Option<String>,
        payload: serde_json::Value,
    ) -> Result<MemoryEvent> {
        self.ensure_skeleton()?;
        let event = MemoryEvent {
            id: uuid::Uuid::new_v4().to_string(),
            category,
            action: action.into(),
            summary: summary.into().chars().take(500).collect(),
            source_id: source_id.and_then(clean_optional),
            payload,
            created_at: Utc::now(),
        };
        if event.summary.trim().is_empty() {
            return Err(anyhow!("memory event summary is empty"));
        }
        let path = self.root.join(event_file(category));
        let mut line = serde_json::to_string(&event)?;
        line.push('\n');
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        file.write_all(line.as_bytes())?;
        Ok(event)
    }

    pub fn recent_events(&self, limit: usize) -> Result<Vec<MemoryEvent>> {
        self.ensure_skeleton()?;
        let mut events = Vec::new();
        for category in [
            MemoryEventCategory::Chat,
            MemoryEventCategory::Quiz,
            MemoryEventCategory::Notebook,
            MemoryEventCategory::Research,
        ] {
            let path = self.root.join(event_file(category));
            let Ok(text) = fs::read_to_string(path) else {
                continue;
            };
            for line in text.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if let Ok(event) = serde_json::from_str::<MemoryEvent>(line) {
                    events.push(event);
                }
            }
        }
        events.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        events.truncate(limit);
        Ok(events)
    }

    pub fn consolidation_preview(&self) -> Result<MemoryConsolidationPreview> {
        let events = self.recent_events(30)?;
        let current = self.read("L3/recent.md")?.markdown;
        let event_markdown = events
            .iter()
            .rev()
            .map(|event| {
                format!(
                    "- [{}] {}: {} <!--{}-->",
                    event.created_at.format("%Y-%m-%d %H:%M"),
                    event_kind_label(event.category, &event.action),
                    event.summary,
                    event.id
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let proposed_markdown = if event_markdown.is_empty() {
            current
        } else {
            format!(
                "# Recent learning context\n\n## Consolidation preview\n\n{}\n\n---\n\n## Previous content\n\n{}",
                event_markdown,
                current.trim()
            )
        };
        Ok(MemoryConsolidationPreview {
            target_path: "L3/recent.md".into(),
            proposed_markdown,
            event_count: events.len(),
        })
    }

    pub fn assist(
        &self,
        path: &str,
        action: MemoryAssistAction,
        markdown: Option<String>,
    ) -> Result<MemoryAssistResult> {
        self.ensure_skeleton()?;
        let path = normalize_memory_path(path)?;
        let target_path = path_to_slash(&path);
        let current = markdown.unwrap_or_else(|| {
            self.read(&target_path)
                .map(|file| file.markdown)
                .unwrap_or_default()
        });
        match action {
            MemoryAssistAction::Update => self.assist_update(&target_path, &current),
            MemoryAssistAction::Check => Ok(assist_check(&target_path, &current)),
            MemoryAssistAction::Dedupe => Ok(assist_dedupe(&target_path, &current)),
        }
    }

    fn assist_update(&self, target_path: &str, current: &str) -> Result<MemoryAssistResult> {
        let events = recent_events_for_target(self.recent_events(60)?, target_path);
        let event_count = events.len();
        let event_markdown = events
            .iter()
            .rev()
            .map(|event| {
                let source = event
                    .source_id
                    .as_deref()
                    .map(|source| format!(" [^{}]", short_ref_index(source)))
                    .unwrap_or_default();
                format!(
                    "- [{}] {}: {}{} <!--m_{}-->",
                    event.created_at.format("%Y-%m-%d %H:%M"),
                    event_kind_label(event.category, &event.action),
                    event.summary,
                    source,
                    event.id.replace('-', "")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        if event_markdown.is_empty() {
            return Ok(MemoryAssistResult {
                target_path: target_path.into(),
                action: MemoryAssistAction::Update,
                report_markdown: "No recent workspace events match this memory file.".into(),
                proposed_markdown: Some(current.to_string()),
                changed: false,
            });
        }

        let mut proposed = current.trim().to_string();
        if !proposed.is_empty() {
            proposed.push_str("\n\n");
        }
        proposed.push_str("## Agent update draft\n\n");
        proposed.push_str(&event_markdown);

        let refs = events
            .iter()
            .filter_map(|event| event.source_id.as_deref())
            .map(|source| format!("[^{}]: {}", short_ref_index(source), source))
            .collect::<std::collections::BTreeSet<_>>();
        if !refs.is_empty() {
            proposed.push_str("\n\n---\n\n");
            proposed.push_str(&refs.into_iter().collect::<Vec<_>>().join("\n"));
        }

        Ok(MemoryAssistResult {
            target_path: target_path.into(),
            action: MemoryAssistAction::Update,
            report_markdown: format!(
                "Prepared an update draft from {event_count} recent matching events."
            ),
            proposed_markdown: Some(proposed),
            changed: true,
        })
    }

    fn ensure_skeleton(&self) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        for dir in DEFAULT_DIRS {
            fs::create_dir_all(self.root.join(dir))?;
        }
        for (path, default_markdown) in DEFAULT_FILES {
            let full_path = self.root.join(path);
            if !full_path.exists() {
                fs::write(full_path, default_markdown)?;
            }
        }
        Ok(())
    }
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

pub fn parse_memory_markers(markdown: &str) -> Vec<MemoryMarker> {
    markdown
        .split("<!--")
        .skip(1)
        .filter_map(|part| part.split("-->").next())
        .map(str::trim)
        .filter(|marker| marker.starts_with("m_"))
        .map(|id| MemoryMarker { id: id.to_string() })
        .collect()
}

pub fn parse_source_refs(markdown: &str) -> Vec<MemorySourceRef> {
    markdown
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("[^")?;
            let (index, target) = rest.split_once("]:")?;
            Some(MemorySourceRef {
                index: index.parse().ok()?,
                target: target.trim().to_string(),
            })
        })
        .collect()
}

pub fn serialize_memory_marker(id: &str) -> Result<String> {
    let id = id.trim();
    if id.is_empty() || !id.starts_with("m_") || id.contains("-->") {
        return Err(anyhow!("invalid memory marker id"));
    }
    Ok(format!("<!--{id}-->"))
}

pub fn serialize_source_ref(reference: &MemorySourceRef) -> Result<String> {
    if reference.index == 0 || reference.target.trim().is_empty() {
        return Err(anyhow!("invalid memory source reference"));
    }
    Ok(format!(
        "[^{}]: {}",
        reference.index,
        reference.target.trim()
    ))
}

fn event_file(category: MemoryEventCategory) -> &'static str {
    match category {
        MemoryEventCategory::Chat => "L1/chat_events.jsonl",
        MemoryEventCategory::Quiz => "L1/quiz_events.jsonl",
        MemoryEventCategory::Notebook => "L1/notebook_events.jsonl",
        MemoryEventCategory::Research => "L1/research_events.jsonl",
    }
}

fn event_kind_label(category: MemoryEventCategory, action: &str) -> String {
    let category = match category {
        MemoryEventCategory::Chat => "chat",
        MemoryEventCategory::Quiz => "quiz",
        MemoryEventCategory::Notebook => "notebook",
        MemoryEventCategory::Research => "research",
    };
    format!("{category}/{action}")
}

fn recent_events_for_target(events: Vec<MemoryEvent>, target_path: &str) -> Vec<MemoryEvent> {
    let category = if target_path.contains("chat") {
        Some(MemoryEventCategory::Chat)
    } else if target_path.contains("quiz")
        || target_path.contains("profile")
        || target_path.contains("teaching_strategy")
    {
        Some(MemoryEventCategory::Quiz)
    } else if target_path.contains("notebook") || target_path.contains("scope") {
        Some(MemoryEventCategory::Notebook)
    } else if target_path.contains("research") {
        Some(MemoryEventCategory::Research)
    } else {
        None
    };
    events
        .into_iter()
        .filter(|event| category.is_none_or(|category| event.category == category))
        .take(20)
        .collect()
}

fn assist_check(target_path: &str, markdown: &str) -> MemoryAssistResult {
    let markers = parse_memory_markers(markdown);
    let refs = parse_source_refs(markdown);
    let mut report = Vec::new();
    report.push(format!("# Memory check: {target_path}"));
    report.push(String::new());
    report.push(format!("- Markers: {}", markers.len()));
    report.push(format!("- Source refs: {}", refs.len()));

    let mut marker_counts = std::collections::BTreeMap::new();
    for marker in markers {
        *marker_counts.entry(marker.id).or_insert(0usize) += 1;
    }
    let duplicate_markers = marker_counts
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(id, count)| format!("{id} x {count}"))
        .collect::<Vec<_>>();
    if duplicate_markers.is_empty() {
        report.push("- Duplicate markers: none".into());
    } else {
        report.push(format!(
            "- Duplicate markers: {}",
            duplicate_markers.join(", ")
        ));
    }

    let duplicate_lines = duplicate_memory_lines(markdown);
    if duplicate_lines.is_empty() {
        report.push("- Duplicate bullets: none".into());
    } else {
        report.push(format!("- Duplicate bullets: {}", duplicate_lines.len()));
        for line in duplicate_lines.iter().take(8) {
            report.push(format!("  - {line}"));
        }
    }

    MemoryAssistResult {
        target_path: target_path.into(),
        action: MemoryAssistAction::Check,
        report_markdown: report.join("\n"),
        proposed_markdown: None,
        changed: false,
    }
}

fn assist_dedupe(target_path: &str, markdown: &str) -> MemoryAssistResult {
    let mut seen = std::collections::BTreeSet::new();
    let mut removed = 0usize;
    let mut lines = Vec::new();
    for line in markdown.lines() {
        let normalized = normalize_memory_line(line);
        let is_memory_line =
            line.trim_start().starts_with("- ") || line.trim_start().starts_with("[^");
        if is_memory_line && !normalized.is_empty() && !seen.insert(normalized) {
            removed += 1;
            continue;
        }
        lines.push(line);
    }
    let proposed = lines.join("\n");
    MemoryAssistResult {
        target_path: target_path.into(),
        action: MemoryAssistAction::Dedupe,
        report_markdown: if removed == 0 {
            "No duplicate bullets or source refs found.".into()
        } else {
            format!("Removed {removed} duplicate bullet/source-ref lines.")
        },
        proposed_markdown: Some(proposed),
        changed: removed > 0,
    }
}

fn duplicate_memory_lines(markdown: &str) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut duplicates = std::collections::BTreeSet::new();
    for line in markdown.lines() {
        if !line.trim_start().starts_with("- ") {
            continue;
        }
        let normalized = normalize_memory_line(line);
        if !normalized.is_empty() && !seen.insert(normalized.clone()) {
            duplicates.insert(line.trim().to_string());
        }
    }
    duplicates.into_iter().collect()
}

fn normalize_memory_line(line: &str) -> String {
    line.split("<!--")
        .next()
        .unwrap_or(line)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn short_ref_index(source: &str) -> usize {
    let mut hash = 0usize;
    for byte in source.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as usize);
    }
    hash % 997 + 1
}

fn clean_optional(value: String) -> Option<String> {
    let value = value.trim().to_string();
    (!value.is_empty()).then_some(value)
}

fn normalize_memory_path(path: &str) -> Result<PathBuf> {
    let normalized = path.trim().replace('\\', "/");
    if normalized.is_empty()
        || normalized.starts_with('/')
        || normalized.contains("..")
        || !normalized.ends_with(".md")
    {
        return Err(anyhow!("invalid memory path"));
    }
    if !DEFAULT_FILES.iter().any(|(item, _)| *item == normalized) {
        return Err(anyhow!("memory file is not editable"));
    }
    Ok(PathBuf::from(normalized))
}

fn path_to_slash(path: &Path) -> String {
    path.components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn default_root() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".llm-tutor")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_creates_skeleton_and_updates_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        let files = store.list().unwrap();
        assert!(files.iter().any(|file| file.path == "L3/profile.md"));

        let updated = store
            .write(
                "L3/profile.md",
                "# Student profile\n\n- Needs review. <!--m_01-->\n\n[^1]: quiz:q1".into(),
            )
            .unwrap();
        assert_eq!(updated.path, "L3/profile.md");
        assert!(updated.markdown.contains("Needs review"));
    }

    #[test]
    fn memory_parser_extracts_markers_and_refs() {
        let markdown = "- Weak on vectors. [^1] <!--m_01ABC-->\n\n[^1]: quiz:session:q1";
        assert_eq!(
            parse_memory_markers(markdown),
            vec![MemoryMarker {
                id: "m_01ABC".into()
            }]
        );
        assert_eq!(
            parse_source_refs(markdown),
            vec![MemorySourceRef {
                index: 1,
                target: "quiz:session:q1".into()
            }]
        );
        assert_eq!(
            serialize_memory_marker("m_01ABC").unwrap(),
            "<!--m_01ABC-->"
        );
        assert_eq!(
            serialize_source_ref(&MemorySourceRef {
                index: 1,
                target: "quiz:session:q1".into()
            })
            .unwrap(),
            "[^1]: quiz:session:q1"
        );
    }

    #[test]
    fn memory_store_records_events_and_builds_preview() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        let event = store
            .record_event(
                MemoryEventCategory::Quiz,
                "answered",
                "Answered OPC question correctly",
                Some("quiz-1".into()),
                json!({ "question_id": "q1" }),
            )
            .unwrap();
        assert_eq!(event.category, MemoryEventCategory::Quiz);
        let events = store.recent_events(10).unwrap();
        assert_eq!(events.len(), 1);
        let preview = store.consolidation_preview().unwrap();
        assert_eq!(preview.target_path, "L3/recent.md");
        assert_eq!(preview.event_count, 1);
        assert!(preview.proposed_markdown.contains("Answered OPC"));
    }
}
