use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

const DEFAULT_FILES: &[(&str, &str)] = &[
    ("L2/chat.md", "# Chat memory\n\n"),
    ("L2/quiz.md", "# Quiz memory\n\n"),
    ("L2/notebook.md", "# Notebook memory\n\n"),
    ("L2/knowledge.md", "# Knowledge memory\n\n"),
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
const MEMORY_UPDATE_CHUNK_SIZE: usize = 10;
const MAX_MEMORY_FACT_TEXT_CHARS: usize = 500;

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
pub struct MemoryUndoResult {
    pub file: MemoryFile,
    pub restored_from: String,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedMemorySource {
    pub reference: String,
    pub event: MemoryEvent,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryEventCategory {
    Chat,
    Quiz,
    Notebook,
    Knowledge,
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
    #[serde(default)]
    pub facts: Vec<MemoryFact>,
    pub edits: Vec<MemoryTextEdit>,
    pub trace: Option<MemoryAssistTrace>,
    pub changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryAssistTrace {
    pub input_json: String,
    pub output_json: String,
    #[serde(default)]
    pub chunks: Vec<MemoryAssistTraceChunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryAssistTraceChunk {
    pub index: usize,
    pub total: usize,
    #[serde(rename = "citeableRefs")]
    pub citeable_refs: Vec<String>,
    pub status: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryEntry {
    pub line_number: usize,
    pub section: Option<String>,
    pub text: String,
    pub marker: String,
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConsolidationMode {
    Update,
    Audit,
    Dedup,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsolidationJob {
    pub mode: ConsolidationMode,
    pub layer: String,
    pub key: String,
    pub language: String,
    pub today: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsolidationTarget {
    pub title: String,
    #[serde(rename = "existingMarkdown")]
    pub existing_markdown: String,
    #[serde(rename = "allowedSections")]
    pub allowed_sections: Vec<String>,
    pub focus: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsolidationChunk {
    pub index: usize,
    pub total: usize,
    #[serde(rename = "citeableRefs")]
    pub citeable_refs: Vec<String>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConsolidationInput {
    pub job: ConsolidationJob,
    pub target: ConsolidationTarget,
    pub chunk: ConsolidationChunk,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryFact {
    pub text: String,
    pub section: String,
    pub refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MemoryTextEditOp {
    Replace,
    Delete,
    Insert,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryTextEdit {
    pub op: MemoryTextEditOp,
    pub start_line: usize,
    pub end_line: Option<usize>,
    pub text: Option<String>,
    #[serde(default)]
    pub refs: Vec<String>,
    pub reason: Option<String>,
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
        let markdown = normalize_memory_markdown(&markdown)?;
        if markdown.trim().is_empty() {
            return Err(anyhow!("memory markdown is empty"));
        }
        let full_path = self.root.join(&path);
        if full_path.exists() {
            self.write_undo_snapshot(&path, &fs::read_to_string(&full_path)?)?;
        }
        fs::write(full_path, markdown)?;
        self.read(&path_to_slash(&path))
    }

    pub fn undo_latest_write(&self, path: &str) -> Result<MemoryUndoResult> {
        self.ensure_skeleton()?;
        let path = normalize_memory_path(path)?;
        let undo_path = self.undo_path(&path);
        if !undo_path.exists() {
            return Err(anyhow!("no memory undo snapshot exists for this file"));
        }
        let markdown = fs::read_to_string(&undo_path)?;
        fs::write(self.root.join(&path), markdown)?;
        fs::remove_file(&undo_path)?;
        let restored_from = path_to_slash(&path);
        Ok(MemoryUndoResult {
            file: self.read(&restored_from)?,
            restored_from,
        })
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
            MemoryEventCategory::Knowledge,
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

    pub fn resolve_source_ref(&self, reference: &str) -> Result<ResolvedMemorySource> {
        self.ensure_skeleton()?;
        let reference = reference.trim();
        let (surface, id) = reference
            .split_once(':')
            .ok_or_else(|| anyhow!("memory source ref must look like surface:id"))?;
        let category = category_for_surface(surface)
            .ok_or_else(|| anyhow!("unsupported memory source surface `{surface}`"))?;
        let path = self.root.join(event_file(category));
        let text = fs::read_to_string(path)?;
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let event = serde_json::from_str::<MemoryEvent>(line)?;
            if event.id == id || event.source_id.as_deref() == Some(id) {
                return Ok(ResolvedMemorySource {
                    reference: reference.to_string(),
                    event,
                });
            }
        }
        Err(anyhow!("memory source ref `{reference}` was not found"))
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

    pub fn consolidation_input(
        &self,
        target_path: &str,
        action: MemoryAssistAction,
        markdown: Option<String>,
    ) -> Result<ConsolidationInput> {
        self.ensure_skeleton()?;
        let path = normalize_memory_path(target_path)?;
        let target_path = path_to_slash(&path);
        let current = markdown.unwrap_or_else(|| {
            self.read(&target_path)
                .map(|file| file.markdown)
                .unwrap_or_default()
        });
        let (citeable_refs, text) = match action {
            MemoryAssistAction::Check => self.audit_source_chunk(&target_path, &current)?,
            MemoryAssistAction::Dedupe => dedupe_source_chunk(&current),
            MemoryAssistAction::Update if target_path.starts_with("L3/") => {
                self.l3_source_chunk()?
            }
            MemoryAssistAction::Update => {
                let events = recent_events_for_target(self.recent_events(60)?, &target_path);
                (
                    events.iter().map(event_citeable_ref).collect::<Vec<_>>(),
                    render_event_chunk(&events),
                )
            }
        };
        Ok(ConsolidationInput {
            job: ConsolidationJob {
                mode: match action {
                    MemoryAssistAction::Update => ConsolidationMode::Update,
                    MemoryAssistAction::Check => ConsolidationMode::Audit,
                    MemoryAssistAction::Dedupe => ConsolidationMode::Dedup,
                },
                layer: target_path
                    .split('/')
                    .next()
                    .unwrap_or_default()
                    .to_string(),
                key: target_path
                    .trim_end_matches(".md")
                    .split('/')
                    .next_back()
                    .unwrap_or_default()
                    .to_string(),
                language: "zh".into(),
                today: Utc::now().date_naive().to_string(),
            },
            target: target_catalog(&target_path, current),
            chunk: ConsolidationChunk {
                index: 1,
                total: 1,
                citeable_refs,
                text,
            },
        })
    }

    pub fn consolidation_inputs(
        &self,
        target_path: &str,
        action: MemoryAssistAction,
        markdown: Option<String>,
    ) -> Result<Vec<ConsolidationInput>> {
        self.ensure_skeleton()?;
        let path = normalize_memory_path(target_path)?;
        let target_path = path_to_slash(&path);
        let current = markdown.unwrap_or_else(|| {
            self.read(&target_path)
                .map(|file| file.markdown)
                .unwrap_or_default()
        });
        let chunks = match action {
            MemoryAssistAction::Check => vec![self.audit_source_chunk(&target_path, &current)?],
            MemoryAssistAction::Dedupe => vec![dedupe_source_chunk(&current)],
            MemoryAssistAction::Update if target_path.starts_with("L3/") => {
                self.l3_source_chunks()?
            }
            MemoryAssistAction::Update => {
                let events = recent_events_for_target(self.recent_events(60)?, &target_path);
                event_source_chunks(&events, MEMORY_UPDATE_CHUNK_SIZE)
            }
        };
        let chunks = if chunks.is_empty() {
            vec![(Vec::new(), String::new())]
        } else {
            chunks
        };
        let total = chunks.len();
        let job = ConsolidationJob {
            mode: match action {
                MemoryAssistAction::Update => ConsolidationMode::Update,
                MemoryAssistAction::Check => ConsolidationMode::Audit,
                MemoryAssistAction::Dedupe => ConsolidationMode::Dedup,
            },
            layer: target_path
                .split('/')
                .next()
                .unwrap_or_default()
                .to_string(),
            key: target_path
                .trim_end_matches(".md")
                .split('/')
                .next_back()
                .unwrap_or_default()
                .to_string(),
            language: "zh".into(),
            today: Utc::now().date_naive().to_string(),
        };
        let target = target_catalog(&target_path, current);
        Ok(chunks
            .into_iter()
            .enumerate()
            .map(|(index, (citeable_refs, text))| ConsolidationInput {
                job: job.clone(),
                target: target.clone(),
                chunk: ConsolidationChunk {
                    index: index + 1,
                    total,
                    citeable_refs,
                    text,
                },
            })
            .collect())
    }

    fn l3_source_chunk(&self) -> Result<(Vec<String>, String)> {
        let mut refs = Vec::new();
        let mut entities = Vec::new();
        for (surface, path) in [
            ("chat", "L2/chat.md"),
            ("quiz", "L2/quiz.md"),
            ("notebook", "L2/notebook.md"),
            ("knowledge", "L2/knowledge.md"),
            ("research", "L2/research.md"),
        ] {
            let markdown = self.read(path)?.markdown;
            if !has_memory_content(&markdown) {
                continue;
            }
            refs.push(surface.to_string());
            entities.push(format!(
                "@entity {surface}\ntitle: {path}\ncontent:\n{}",
                markdown.trim()
            ));
        }
        let text = if refs.is_empty() {
            String::new()
        } else {
            let rendered_refs = refs
                .iter()
                .map(|reference| format!("- {reference}"))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "# Chunk-local citeable refs\n{rendered_refs}\n\n{}",
                entities.join("\n\n")
            )
        };
        Ok((refs, text))
    }

    fn l3_source_chunks(&self) -> Result<Vec<(Vec<String>, String)>> {
        let mut chunks = Vec::new();
        for (surface, path) in [
            ("chat", "L2/chat.md"),
            ("quiz", "L2/quiz.md"),
            ("notebook", "L2/notebook.md"),
            ("knowledge", "L2/knowledge.md"),
            ("research", "L2/research.md"),
        ] {
            let markdown = self.read(path)?.markdown;
            if !has_memory_content(&markdown) {
                continue;
            }
            let text = format!(
                "# Chunk-local citeable refs\n- {surface}\n\n@entity {surface}\ntitle: {path}\ncontent:\n{}",
                markdown.trim()
            );
            chunks.push((vec![surface.to_string()], text));
        }
        Ok(chunks)
    }

    fn audit_source_chunk(
        &self,
        target_path: &str,
        markdown: &str,
    ) -> Result<(Vec<String>, String)> {
        let definitions = parse_source_refs(markdown)
            .into_iter()
            .map(|reference| (reference.index, reference.target))
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut citeable_refs = std::collections::BTreeSet::new();
        let mut evidence_blocks = Vec::new();
        for (line_number, line) in markdown.lines().enumerate() {
            let source_targets = footnote_indices_in_line(line)
                .into_iter()
                .filter_map(|index| definitions.get(&index).cloned())
                .collect::<Vec<_>>();
            if source_targets.is_empty() {
                continue;
            }
            for target in source_targets {
                citeable_refs.insert(target.clone());
                evidence_blocks.push(render_audit_evidence(
                    self,
                    target_path,
                    line_number + 1,
                    line,
                    &target,
                ));
            }
        }
        let citeable_refs = citeable_refs.into_iter().collect::<Vec<_>>();
        let text = if evidence_blocks.is_empty() {
            format!("# Line-numbered view\n{}", line_numbered_markdown(markdown))
        } else {
            format!(
                "# Line-numbered view\n{}\n\n# Evidence\n{}",
                line_numbered_markdown(markdown),
                evidence_blocks.join("\n\n")
            )
        };
        Ok((citeable_refs, text))
    }

    pub fn append_memory_facts(
        &self,
        target_path: &str,
        current: &str,
        facts: &[MemoryFact],
        citeable_refs: &[String],
        allowed_sections: &[String],
    ) -> Result<String> {
        let target_path = path_to_slash(&normalize_memory_path(target_path)?);
        if facts.is_empty() {
            return Ok(current.to_string());
        }
        validate_memory_facts(&target_path, facts, citeable_refs, allowed_sections)?;

        let mut entries = parse_memory_entries(current);
        for fact in facts {
            let normalized = normalize_memory_fact_text(&fact.text);
            if let Some(entry) = entries
                .iter_mut()
                .find(|entry| normalize_memory_fact_text(&entry.text) == normalized)
            {
                merge_source_refs(&mut entry.source_refs, &fact.refs);
                continue;
            }
            entries.push(MemoryEntry {
                line_number: 0,
                section: Some(fact.section.trim().to_string()),
                text: fact.text.trim().to_string(),
                marker: format!("m_{}", uuid::Uuid::new_v4().simple()),
                source_refs: fact.refs.clone(),
            });
        }
        let title = memory_title(current)
            .unwrap_or_else(|| target_catalog(&target_path, String::new()).title);
        normalize_memory_markdown(&serialize_memory_entries(&title, &entries)?)
    }

    pub fn apply_text_edits(&self, current: &str, edits: &[MemoryTextEdit]) -> Result<String> {
        apply_text_edits(current, edits)
    }

    pub fn validate_text_edits(
        &self,
        current: &str,
        edits: &[MemoryTextEdit],
        citeable_refs: &[String],
    ) -> Result<()> {
        validate_text_edits(current, edits, citeable_refs)
    }

    pub fn validate_text_edits_for_action(
        &self,
        action: MemoryAssistAction,
        current: &str,
        edits: &[MemoryTextEdit],
        citeable_refs: &[String],
    ) -> Result<()> {
        if action == MemoryAssistAction::Dedupe
            && edits.iter().any(|edit| edit.op == MemoryTextEditOp::Insert)
        {
            return Err(anyhow!("memory dedupe edit must not insert new facts"));
        }
        validate_text_edits(current, edits, citeable_refs)
    }

    fn assist_update(&self, target_path: &str, current: &str) -> Result<MemoryAssistResult> {
        let input = self.consolidation_input(
            target_path,
            MemoryAssistAction::Update,
            Some(current.to_string()),
        )?;
        if input.chunk.citeable_refs.is_empty() {
            return Ok(MemoryAssistResult {
                target_path: target_path.into(),
                action: MemoryAssistAction::Update,
                report_markdown: "No recent workspace events match this memory file.".into(),
                proposed_markdown: Some(current.to_string()),
                facts: Vec::new(),
                edits: Vec::new(),
                trace: None,
                changed: false,
            });
        }
        let facts = facts_from_input_chunk(&input);
        let proposed = self.append_memory_facts(
            target_path,
            current,
            &facts,
            &input.chunk.citeable_refs,
            &input.target.allowed_sections,
        )?;

        Ok(MemoryAssistResult {
            target_path: target_path.into(),
            action: MemoryAssistAction::Update,
            report_markdown: format!(
                "Prepared an update draft from {} normalized source events.",
                input.chunk.citeable_refs.len()
            ),
            proposed_markdown: Some(proposed),
            facts,
            edits: Vec::new(),
            trace: None,
            changed: true,
        })
    }

    fn ensure_skeleton(&self) -> Result<()> {
        fs::create_dir_all(&self.root)?;
        for dir in DEFAULT_DIRS {
            fs::create_dir_all(self.root.join(dir))?;
        }
        fs::create_dir_all(self.root.join(".undo"))?;
        for (path, default_markdown) in DEFAULT_FILES {
            let full_path = self.root.join(path);
            if !full_path.exists() {
                fs::write(full_path, default_markdown)?;
            }
        }
        Ok(())
    }

    fn write_undo_snapshot(&self, path: &Path, markdown: &str) -> Result<()> {
        let undo_path = self.undo_path(path);
        if let Some(parent) = undo_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(undo_path, markdown)?;
        Ok(())
    }

    fn undo_path(&self, path: &Path) -> PathBuf {
        self.root.join(".undo").join(path).with_extension("md.bak")
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
    markdown.lines().filter_map(parse_source_ref_line).collect()
}

pub fn parse_memory_entries(markdown: &str) -> Vec<MemoryEntry> {
    let definitions = parse_source_refs(markdown)
        .into_iter()
        .map(|reference| (reference.index, reference.target))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut section = None::<String>;
    let mut entries = Vec::new();
    for (index, line) in markdown.lines().enumerate() {
        if let Some(heading) = memory_section_heading(line) {
            section = Some(heading);
            continue;
        }
        if let Some(entry) = parse_memory_entry_line(index + 1, line, section.clone(), &definitions)
        {
            entries.push(entry);
        }
    }
    entries
}

pub fn serialize_memory_entries(title: &str, entries: &[MemoryEntry]) -> Result<String> {
    let title = title.trim();
    if title.is_empty() {
        return Err(anyhow!("memory title is empty"));
    }
    let mut refs_by_target = std::collections::BTreeMap::<String, usize>::new();
    let mut next_ref = 1usize;
    let mut lines = vec![format!("# {title}")];
    let mut last_section = None::<String>;
    for entry in entries {
        if entry.text.trim().is_empty() {
            return Err(anyhow!("memory entry text is empty"));
        }
        let section = entry
            .section
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        if lines.len() == 1 {
            lines.push(String::new());
            if let Some(section) = &section {
                lines.push(format!("## {section}"));
                lines.push(String::new());
            }
            last_section = section;
        } else if section != last_section {
            lines.push(String::new());
            if let Some(section) = &section {
                lines.push(format!("## {section}"));
                lines.push(String::new());
            }
            last_section = section;
        }
        let marker = serialize_memory_marker(&entry.marker)?;
        let refs = entry
            .source_refs
            .iter()
            .filter_map(|target| {
                let target = target.trim();
                if target.is_empty() {
                    return None;
                }
                let index = if let Some(index) = refs_by_target.get(target) {
                    *index
                } else {
                    let index = next_ref;
                    next_ref += 1;
                    refs_by_target.insert(target.to_string(), index);
                    index
                };
                Some(format!("[^{index}]"))
            })
            .collect::<Vec<_>>()
            .join(" ");
        let refs = if refs.is_empty() {
            String::new()
        } else {
            format!(" {refs}")
        };
        lines.push(format!("- {}{} {}", entry.text.trim(), refs, marker));
    }
    if !refs_by_target.is_empty() {
        lines.push(String::new());
        lines.push("---".into());
        lines.push(String::new());
        let refs = refs_by_target
            .iter()
            .map(|(target, index)| {
                serialize_source_ref(&MemorySourceRef {
                    index: *index,
                    target: target.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        lines.extend(refs);
    }
    Ok(lines.join("\n"))
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

pub fn normalize_memory_markdown(markdown: &str) -> Result<String> {
    let mut definitions = std::collections::BTreeMap::<usize, String>::new();
    let mut body_lines = Vec::new();
    for line in markdown.lines() {
        if let Some(reference) = parse_source_ref_line(line) {
            definitions
                .entry(reference.index)
                .or_insert(reference.target);
        } else {
            body_lines.push(line.to_string());
        }
    }

    let mut old_to_new = std::collections::BTreeMap::<usize, usize>::new();
    let mut target_to_new = std::collections::BTreeMap::<String, usize>::new();
    let mut normalized_refs = Vec::<MemorySourceRef>::new();
    for line in &body_lines {
        for old_index in footnote_indices_in_line(line) {
            let Some(target) = definitions.get(&old_index).cloned() else {
                continue;
            };
            if let Some(new_index) = target_to_new.get(&target).copied() {
                old_to_new.insert(old_index, new_index);
                continue;
            }
            let new_index = normalized_refs.len() + 1;
            target_to_new.insert(target.clone(), new_index);
            old_to_new.insert(old_index, new_index);
            normalized_refs.push(MemorySourceRef {
                index: new_index,
                target,
            });
        }
    }

    let mut normalized_body = body_lines
        .iter()
        .map(|line| replace_footnote_indices(line, &old_to_new))
        .collect::<Vec<_>>();
    while normalized_body
        .last()
        .is_some_and(|line| line.trim().is_empty())
    {
        normalized_body.pop();
    }
    let mut result = normalized_body.join("\n");
    if !normalized_refs.is_empty() {
        if !result.trim().is_empty() {
            result.push_str("\n\n");
        }
        result.push_str("---\n\n");
        let refs = normalized_refs
            .iter()
            .map(serialize_source_ref)
            .collect::<Result<Vec<_>>>()?;
        result.push_str(&refs.join("\n"));
    }
    Ok(result)
}

fn parse_source_ref_line(line: &str) -> Option<MemorySourceRef> {
    let line = line.trim();
    let rest = line.strip_prefix("[^")?;
    let (index, target) = rest.split_once("]:")?;
    Some(MemorySourceRef {
        index: index.parse().ok()?,
        target: target.trim().to_string(),
    })
}

fn parse_memory_entry_line(
    line_number: usize,
    line: &str,
    section: Option<String>,
    definitions: &std::collections::BTreeMap<usize, String>,
) -> Option<MemoryEntry> {
    let trimmed = line.trim();
    let bullet = trimmed.strip_prefix("- ")?;
    let marker = marker_in_line(bullet)?;
    let source_refs = footnote_indices_in_line(bullet)
        .into_iter()
        .filter_map(|index| definitions.get(&index).cloned())
        .collect::<Vec<_>>();
    let text = strip_entry_markup(bullet).trim().to_string();
    if text.is_empty() {
        return None;
    }
    Some(MemoryEntry {
        line_number,
        section,
        text,
        marker,
        source_refs,
    })
}

fn memory_title(markdown: &str) -> Option<String> {
    markdown.lines().find_map(|line| {
        let line = line.trim();
        line.strip_prefix("# ")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn memory_section_heading(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let heading = trimmed
        .strip_prefix("## ")
        .or_else(|| trimmed.strip_prefix("### "))?
        .trim();
    (!heading.is_empty()).then(|| heading.to_string())
}

fn marker_in_line(line: &str) -> Option<String> {
    let start = line.find("<!--")?;
    let rest = &line[start + 4..];
    let end = rest.find("-->")?;
    let marker = rest[..end].trim();
    marker.starts_with("m_").then(|| marker.to_string())
}

fn strip_entry_markup(line: &str) -> String {
    let mut output = String::new();
    let mut rest = line;
    loop {
        let footnote_pos = rest.find("[^");
        let marker_pos = rest.find("<!--");
        let Some(next_pos) = (match (footnote_pos, marker_pos) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        }) else {
            output.push_str(rest);
            break;
        };
        output.push_str(&rest[..next_pos]);
        rest = &rest[next_pos..];
        if rest.starts_with("[^") {
            if let Some(end) = rest.find(']') {
                rest = &rest[end + 1..];
            } else {
                output.push_str(rest);
                break;
            }
        } else if rest.starts_with("<!--") {
            if let Some(end) = rest.find("-->") {
                rest = &rest[end + 3..];
            } else {
                output.push_str(rest);
                break;
            }
        }
    }
    output.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn footnote_indices_in_line(line: &str) -> Vec<usize> {
    let mut indices = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find("[^") {
        rest = &rest[start + 2..];
        let Some(end) = rest.find(']') else {
            break;
        };
        if let Ok(index) = rest[..end].parse::<usize>() {
            indices.push(index);
        }
        rest = &rest[end + 1..];
    }
    indices
}

fn replace_footnote_indices(
    line: &str,
    old_to_new: &std::collections::BTreeMap<usize, usize>,
) -> String {
    let mut output = String::new();
    let mut rest = line;
    loop {
        let Some(start) = rest.find("[^") else {
            output.push_str(rest);
            break;
        };
        output.push_str(&rest[..start]);
        output.push_str("[^");
        rest = &rest[start + 2..];
        let Some(end) = rest.find(']') else {
            output.push_str(rest);
            break;
        };
        if let Ok(old_index) = rest[..end].parse::<usize>() {
            if let Some(new_index) = old_to_new.get(&old_index) {
                output.push_str(&new_index.to_string());
            } else {
                output.push_str(&rest[..end]);
            }
        } else {
            output.push_str(&rest[..end]);
        }
        output.push(']');
        rest = &rest[end + 1..];
    }
    output
}

fn apply_text_edits(current: &str, edits: &[MemoryTextEdit]) -> Result<String> {
    if edits.is_empty() {
        return Ok(current.to_string());
    }
    let mut lines = current.lines().map(str::to_string).collect::<Vec<_>>();
    let original_len = lines.len();
    let mut sorted = edits.to_vec();
    sorted.sort_by(|a, b| b.start_line.cmp(&a.start_line));
    for edit in sorted {
        validate_text_edit(&edit, original_len)?;
        match edit.op {
            MemoryTextEditOp::Delete => {
                let start = edit.start_line - 1;
                let end = edit.end_line.unwrap_or(edit.start_line);
                lines.drain(start..end);
            }
            MemoryTextEditOp::Replace => {
                let start = edit.start_line - 1;
                let end = edit.end_line.unwrap_or(edit.start_line);
                let replacement = edit
                    .text
                    .unwrap_or_default()
                    .lines()
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                lines.splice(start..end, replacement);
            }
            MemoryTextEditOp::Insert => {
                let index = edit.start_line - 1;
                let insertion = edit
                    .text
                    .unwrap_or_default()
                    .lines()
                    .map(str::to_string)
                    .collect::<Vec<_>>();
                lines.splice(index..index, insertion);
            }
        }
    }
    Ok(lines.join("\n"))
}

fn validate_text_edits(
    current: &str,
    edits: &[MemoryTextEdit],
    citeable_refs: &[String],
) -> Result<()> {
    let original_len = current.lines().count();
    let lines = current.lines().collect::<Vec<_>>();
    let allowed_refs = allowed_edit_refs(current, citeable_refs);
    for edit in edits {
        validate_text_edit(edit, original_len)?;
        validate_edit_visible_lines(edit, &lines)?;
        if edit.reason.as_deref().unwrap_or_default().trim().is_empty() {
            return Err(anyhow!("memory edit requires a reason"));
        }
        let refs_required = matches!(
            edit.op,
            MemoryTextEditOp::Replace | MemoryTextEditOp::Insert
        ) && !allowed_refs.is_empty();
        if refs_required && edit.refs.is_empty() {
            return Err(anyhow!(
                "memory replace/insert edit must cite evidence refs"
            ));
        }
        for reference in &edit.refs {
            if !allowed_refs.contains(reference) {
                return Err(anyhow!(
                    "memory edit cites unknown source ref `{reference}`"
                ));
            }
        }
    }
    Ok(())
}

fn validate_edit_visible_lines(edit: &MemoryTextEdit, lines: &[&str]) -> Result<()> {
    if edit.op == MemoryTextEditOp::Insert {
        return Ok(());
    }
    let end_line = edit.end_line.unwrap_or(edit.start_line);
    for line_number in edit.start_line..=end_line {
        let line = lines.get(line_number - 1).copied().unwrap_or_default();
        if is_protected_memory_line(line) {
            return Err(anyhow!(
                "memory edit cannot target protected line {line_number}"
            ));
        }
    }
    Ok(())
}

fn is_protected_memory_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.is_empty() || trimmed == "---" || trimmed.starts_with('#') || trimmed.starts_with("[^")
}

fn allowed_edit_refs(
    current: &str,
    citeable_refs: &[String],
) -> std::collections::BTreeSet<String> {
    let mut refs = citeable_refs
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    for reference in parse_source_refs(current) {
        refs.insert(reference.target);
    }
    refs
}

fn validate_text_edit(edit: &MemoryTextEdit, original_len: usize) -> Result<()> {
    if edit.start_line == 0 {
        return Err(anyhow!("memory edit start_line must be >= 1"));
    }
    let end_line = edit.end_line.unwrap_or(edit.start_line);
    match edit.op {
        MemoryTextEditOp::Replace | MemoryTextEditOp::Delete => {
            if end_line < edit.start_line {
                return Err(anyhow!("memory edit has invalid line range"));
            }
            if end_line > original_len {
                return Err(anyhow!("memory edit line range is out of bounds"));
            }
            if matches!(edit.op, MemoryTextEditOp::Replace)
                && edit.text.as_deref().unwrap_or_default().trim().is_empty()
            {
                return Err(anyhow!("memory replace edit requires text"));
            }
        }
        MemoryTextEditOp::Insert => {
            if edit.start_line > original_len + 1 {
                return Err(anyhow!("memory insert edit line is out of bounds"));
            }
            if edit.text.as_deref().unwrap_or_default().trim().is_empty() {
                return Err(anyhow!("memory insert edit requires text"));
            }
        }
    }
    Ok(())
}

fn event_file(category: MemoryEventCategory) -> &'static str {
    match category {
        MemoryEventCategory::Chat => "L1/chat_events.jsonl",
        MemoryEventCategory::Quiz => "L1/quiz_events.jsonl",
        MemoryEventCategory::Notebook => "L1/notebook_events.jsonl",
        MemoryEventCategory::Knowledge => "L1/knowledge_events.jsonl",
        MemoryEventCategory::Research => "L1/research_events.jsonl",
    }
}

fn event_kind_label(category: MemoryEventCategory, action: &str) -> String {
    let category = match category {
        MemoryEventCategory::Chat => "chat",
        MemoryEventCategory::Quiz => "quiz",
        MemoryEventCategory::Notebook => "notebook",
        MemoryEventCategory::Knowledge => "knowledge",
        MemoryEventCategory::Research => "research",
    };
    format!("{category}/{action}")
}

fn event_surface(category: MemoryEventCategory) -> &'static str {
    match category {
        MemoryEventCategory::Chat => "chat",
        MemoryEventCategory::Quiz => "quiz",
        MemoryEventCategory::Notebook => "notebook",
        MemoryEventCategory::Knowledge => "knowledge",
        MemoryEventCategory::Research => "research",
    }
}

fn category_for_surface(surface: &str) -> Option<MemoryEventCategory> {
    match surface {
        "chat" => Some(MemoryEventCategory::Chat),
        "quiz" => Some(MemoryEventCategory::Quiz),
        "notebook" => Some(MemoryEventCategory::Notebook),
        "knowledge" => Some(MemoryEventCategory::Knowledge),
        "research" => Some(MemoryEventCategory::Research),
        _ => None,
    }
}

fn event_citeable_ref(event: &MemoryEvent) -> String {
    let id = event.source_id.as_deref().unwrap_or(&event.id);
    format!("{}:{}", event_surface(event.category), id)
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
    } else if target_path.contains("knowledge") {
        Some(MemoryEventCategory::Knowledge)
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

fn target_catalog(target_path: &str, existing_markdown: String) -> ConsolidationTarget {
    let (title, focus, sections) = match target_path {
        "L2/chat.md" => (
            "Chat memory",
            "Stable misconceptions, demonstrated mastery, and recurring topics.",
            vec!["Misconceptions", "Mastery", "Topics"],
        ),
        "L2/quiz.md" => (
            "Quiz memory",
            "Error patterns, strong topics, weak topics, and question types.",
            vec!["Error patterns", "Strong topics", "Weak topics"],
        ),
        "L2/notebook.md" => (
            "Notebook memory",
            "Recurring note themes, preferred formats, and open questions.",
            vec!["Themes", "Formats", "Open questions"],
        ),
        "L2/knowledge.md" => (
            "Knowledge memory",
            "Document interests, frequent queries, and knowledge gaps.",
            vec!["Interests", "Frequent queries", "Gaps"],
        ),
        "L2/research.md" => (
            "Research memory",
            "Research topics, preferred report shape, and unresolved questions.",
            vec!["Topics", "Report preferences", "Open questions"],
        ),
        "L3/recent.md" => (
            "Recent learning context",
            "Rolling timeline of recent learning activity.",
            vec!["This week", "Earlier"],
        ),
        "L3/profile.md" => (
            "Student profile",
            "Durable learner identity, learning style, strengths, and weaknesses.",
            vec!["Identity", "Learning style", "Strengths", "Weaknesses"],
        ),
        "L3/scope.md" => (
            "Learning scope",
            "Concepts the learner has engaged with and confidence labels.",
            vec!["Familiar", "Practicing", "Unsure"],
        ),
        "L3/preferences.md" => (
            "Learning preferences",
            "Explicit user-stated long-term preferences.",
            vec!["Preferences"],
        ),
        "L3/teaching_strategy.md" => (
            "Teaching strategy",
            "How the tutor should adapt examples, difficulty, hints, and reviews.",
            vec!["Explanation style", "Practice strategy", "Review strategy"],
        ),
        _ => ("Memory", "Durable learner memory.", vec!["Notes"]),
    };
    ConsolidationTarget {
        title: title.into(),
        existing_markdown,
        allowed_sections: sections.into_iter().map(str::to_string).collect(),
        focus: focus.into(),
    }
}

fn render_event_chunk(events: &[MemoryEvent]) -> String {
    if events.is_empty() {
        return String::new();
    }
    let refs = events
        .iter()
        .map(event_citeable_ref)
        .map(|reference| format!("- {reference}"))
        .collect::<Vec<_>>()
        .join("\n");
    let entities = events
        .iter()
        .map(|event| {
            let reference = event_citeable_ref(event);
            format!(
                "@entity {reference}\ntitle: {}\nts: {}\ncontent:\n{}\nmetadata:\n{}",
                event_kind_label(event.category, &event.action),
                event.created_at.to_rfc3339(),
                event.summary.trim(),
                event.payload
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    format!("# Chunk-local citeable refs\n{refs}\n\n{entities}")
}

fn event_source_chunks(events: &[MemoryEvent], chunk_size: usize) -> Vec<(Vec<String>, String)> {
    if events.is_empty() || chunk_size == 0 {
        return Vec::new();
    }
    events
        .chunks(chunk_size)
        .map(|chunk| {
            (
                chunk.iter().map(event_citeable_ref).collect::<Vec<_>>(),
                render_event_chunk(chunk),
            )
        })
        .collect()
}

fn dedupe_source_chunk(markdown: &str) -> (Vec<String>, String) {
    let citeable_refs = parse_source_refs(markdown)
        .into_iter()
        .map(|reference| reference.target)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let refs = if citeable_refs.is_empty() {
        String::new()
    } else {
        format!(
            "\n\n# Existing source refs\n{}",
            citeable_refs
                .iter()
                .map(|reference| format!("- {reference}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    };
    (
        citeable_refs,
        format!(
            "# Line-numbered view\n{}{}",
            line_numbered_markdown(markdown),
            refs
        ),
    )
}

fn line_numbered_markdown(markdown: &str) -> String {
    markdown
        .lines()
        .enumerate()
        .map(|(index, line)| format!("{:>4}: {}", index + 1, line))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_audit_evidence(
    store: &MemoryStore,
    target_path: &str,
    line_number: usize,
    line: &str,
    reference: &str,
) -> String {
    let source = if reference.contains(':') {
        match store.resolve_source_ref(reference) {
            Ok(source) => format!(
                "source_status: found\nsource_kind: L1 event\ntitle: {}\nts: {}\ncontent:\n{}\nmetadata:\n{}",
                event_kind_label(source.event.category, &source.event.action),
                source.event.created_at.to_rfc3339(),
                source.event.summary.trim(),
                source.event.payload
            ),
            Err(err) => format!("source_status: missing\nerror: {err}"),
        }
    } else if target_path.starts_with("L3/") {
        match l2_path_for_surface(reference).and_then(|path| store.read(path).ok()) {
            Some(file) if has_memory_content(&file.markdown) => format!(
                "source_status: found\nsource_kind: L2 surface memory\nsource_path: {}\ncontent:\n{}",
                file.path,
                file.markdown.trim()
            ),
            Some(file) => format!(
                "source_status: empty\nsource_kind: L2 surface memory\nsource_path: {}",
                file.path
            ),
            None => format!("source_status: missing\nerror: unsupported surface `{reference}`"),
        }
    } else {
        "source_status: unsupported\nerror: L2 memory refs must point to L1 events".into()
    };
    format!(
        "## Evidence for line {line_number}\nline: {}\nref: {reference}\n{source}",
        line.trim()
    )
}

fn l2_path_for_surface(surface: &str) -> Option<&'static str> {
    match surface {
        "chat" => Some("L2/chat.md"),
        "quiz" => Some("L2/quiz.md"),
        "notebook" => Some("L2/notebook.md"),
        "knowledge" => Some("L2/knowledge.md"),
        "research" => Some("L2/research.md"),
        _ => None,
    }
}

fn facts_from_input_chunk(input: &ConsolidationInput) -> Vec<MemoryFact> {
    if input.chunk.citeable_refs.is_empty() {
        return Vec::new();
    }
    let section = input
        .target
        .allowed_sections
        .first()
        .cloned()
        .unwrap_or_else(|| "Notes".into());
    input
        .chunk
        .text
        .split("@entity ")
        .skip(1)
        .filter_map(|block| {
            let reference = block.lines().next()?.trim();
            if !input
                .chunk
                .citeable_refs
                .iter()
                .any(|item| item == reference)
            {
                return None;
            }
            let content = block
                .split("content:\n")
                .nth(1)?
                .split("\nmetadata:")
                .next()?;
            let text = content.trim();
            (!text.is_empty()).then(|| MemoryFact {
                text: text.chars().take(240).collect(),
                section: section.clone(),
                refs: vec![reference.to_string()],
            })
        })
        .collect()
}

fn validate_memory_facts(
    target_path: &str,
    facts: &[MemoryFact],
    citeable_refs: &[String],
    allowed_sections: &[String],
) -> Result<()> {
    let citeable_refs = citeable_refs
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let allowed_sections = allowed_sections
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    for fact in facts {
        if fact.text.trim().is_empty() {
            return Err(anyhow!("memory fact text is empty"));
        }
        if fact.text.chars().count() > MAX_MEMORY_FACT_TEXT_CHARS {
            return Err(anyhow!("memory fact text is too long"));
        }
        if !allowed_sections.is_empty() && !allowed_sections.contains(fact.section.trim()) {
            return Err(anyhow!(
                "memory fact uses unsupported section `{}`",
                fact.section
            ));
        }
        if fact.refs.is_empty() {
            return Err(anyhow!("memory fact must cite at least one source"));
        }
        for reference in &fact.refs {
            if target_path.starts_with("L3/") {
                if reference.contains(':') || category_for_surface(reference).is_none() {
                    return Err(anyhow!(
                        "L3 memory fact must cite a bare allowed surface, got `{reference}`"
                    ));
                }
            }
            if !citeable_refs.contains(reference.as_str()) {
                return Err(anyhow!(
                    "memory fact cites unknown source ref `{reference}`"
                ));
            }
        }
    }
    Ok(())
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
        facts: Vec::new(),
        edits: Vec::new(),
        trace: None,
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
        facts: Vec::new(),
        edits: Vec::new(),
        trace: None,
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

fn normalize_memory_fact_text(text: &str) -> String {
    text.split("<!--")
        .next()
        .unwrap_or(text)
        .split("[^")
        .next()
        .unwrap_or(text)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_end_matches(['.', '。'])
        .to_ascii_lowercase()
}

fn merge_source_refs(target: &mut Vec<String>, refs: &[String]) {
    for reference in refs {
        if !target.iter().any(|item| item == reference) {
            target.push(reference.clone());
        }
    }
}

fn has_memory_content(markdown: &str) -> bool {
    markdown
        .lines()
        .any(|line| line.trim_start().starts_with("- "))
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
    use serde_json::json;

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
    fn normalize_memory_markdown_dedupes_and_removes_unused_refs() {
        let markdown = "# Quiz memory\n\n- First fact. [^2]\n- Second fact. [^3]\n- Unknown fact. [^9]\n\n[^1]: chat:unused\n[^2]: quiz:q1\n[^3]: quiz:q1\n[^4]: quiz:unused";

        let normalized = normalize_memory_markdown(markdown).unwrap();

        assert!(normalized.contains("- First fact. [^1]"));
        assert!(normalized.contains("- Second fact. [^1]"));
        assert!(normalized.contains("- Unknown fact. [^9]"));
        assert!(normalized.contains("[^1]: quiz:q1"));
        assert!(!normalized.contains("chat:unused"));
        assert!(!normalized.contains("quiz:unused"));
        assert!(!normalized.contains("[^2]:"));
    }

    #[test]
    fn memory_entries_round_trip_with_shared_source_refs() {
        let markdown = "# Quiz memory\n\n- First fact. [^2] <!--m_1-->\n- Second fact. [^3] <!--m_2-->\n\n---\n\n[^2]: quiz:q1\n[^3]: quiz:q1\n[^4]: quiz:unused";

        let entries = parse_memory_entries(markdown);
        let serialized = serialize_memory_entries("Quiz memory", &entries).unwrap();
        let reparsed = parse_memory_entries(&serialized);

        assert_eq!(entries, reparsed);
        assert!(serialized.contains("- First fact. [^1] <!--m_1-->"));
        assert!(serialized.contains("- Second fact. [^1] <!--m_2-->"));
        assert!(serialized.contains("[^1]: quiz:q1"));
        assert!(!serialized.contains("quiz:unused"));
        assert!(!serialized.contains("[^2]:"));
    }

    #[test]
    fn memory_entry_serializer_removes_refs_for_deleted_entries() {
        let markdown = "# Chat memory\n\n- Keep this. [^1] <!--m_keep-->\n- Delete this. [^2] <!--m_drop-->\n\n---\n\n[^1]: chat:keep\n[^2]: chat:drop";
        let entries = parse_memory_entries(markdown)
            .into_iter()
            .filter(|entry| entry.marker != "m_drop")
            .collect::<Vec<_>>();

        let serialized = serialize_memory_entries("Chat memory", &entries).unwrap();

        assert!(serialized.contains("chat:keep"));
        assert!(!serialized.contains("chat:drop"));
        assert_eq!(parse_memory_entries(&serialized).len(), 1);
    }

    #[test]
    fn memory_entry_serializer_preserves_sections() {
        let markdown = "# Quiz memory\n\n## Weak topics\n\n- Needs OPC review. [^1] <!--m_1-->\n\n## Strong topics\n\n- Understands basic lithography. [^2] <!--m_2-->\n\n---\n\n[^1]: quiz:q1\n[^2]: quiz:q2";

        let entries = parse_memory_entries(markdown);
        let serialized = serialize_memory_entries("Quiz memory", &entries).unwrap();

        assert_eq!(entries[0].section.as_deref(), Some("Weak topics"));
        assert_eq!(entries[1].section.as_deref(), Some("Strong topics"));
        assert!(serialized.contains("## Weak topics\n\n- Needs OPC review."));
        assert!(serialized.contains("## Strong topics\n\n- Understands basic lithography."));
        assert_eq!(
            parse_memory_entries(&serialized)
                .iter()
                .map(|entry| entry.section.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("Weak topics"), Some("Strong topics")]
        );
    }

    #[test]
    fn memory_store_write_normalizes_source_refs() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        let updated = store
            .write(
                "L2/quiz.md",
                "# Quiz memory\n\n- Same source. [^7]\n\n[^7]: quiz:q1\n[^8]: quiz:unused".into(),
            )
            .unwrap();

        assert!(updated.markdown.contains("- Same source. [^1]"));
        assert!(updated.markdown.contains("[^1]: quiz:q1"));
        assert!(!updated.markdown.contains("quiz:unused"));
    }

    #[test]
    fn memory_store_can_undo_latest_write_once() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        store
            .write("L2/chat.md", "# Chat memory\n\n- Original.".into())
            .unwrap();
        store
            .write("L2/chat.md", "# Chat memory\n\n- Changed.".into())
            .unwrap();

        let restored = store.undo_latest_write("L2/chat.md").unwrap();

        assert!(restored.file.markdown.contains("Original"));
        assert!(!restored.file.markdown.contains("Changed"));
        let err = store.undo_latest_write("L2/chat.md").unwrap_err();
        assert!(err.to_string().contains("no memory undo snapshot"));
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

    #[test]
    fn memory_store_lists_knowledge_surface_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        let files = store.list().unwrap();

        assert!(files.iter().any(|file| file.path == "L2/knowledge.md"));
        assert!(dir.path().join("memory/L2/knowledge.md").exists());
    }

    #[test]
    fn memory_store_resolves_source_refs_to_l1_events() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        store
            .record_event(
                MemoryEventCategory::Quiz,
                "answered",
                "Answered OPC question correctly",
                Some("quiz-1".into()),
                json!({ "question_id": "q1" }),
            )
            .unwrap();

        let source = store.resolve_source_ref("quiz:quiz-1").unwrap();

        assert_eq!(source.reference, "quiz:quiz-1");
        assert_eq!(source.event.summary, "Answered OPC question correctly");
    }

    #[test]
    fn consolidation_input_normalizes_events_with_citeable_refs() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        store
            .record_event(
                MemoryEventCategory::Quiz,
                "answered",
                "Missed an OPC distractor question",
                Some("quiz-attempt-1".into()),
                json!({ "question_id": "q1" }),
            )
            .unwrap();

        let input = store
            .consolidation_input("L2/quiz.md", MemoryAssistAction::Update, None)
            .unwrap();

        assert_eq!(input.job.layer, "L2");
        assert_eq!(input.job.key, "quiz");
        assert_eq!(input.chunk.citeable_refs, vec!["quiz:quiz-attempt-1"]);
        assert!(input.chunk.text.contains("@entity quiz:quiz-attempt-1"));
        assert!(
            input
                .target
                .allowed_sections
                .contains(&"Weak topics".into())
        );
    }

    #[test]
    fn check_consolidation_input_annotates_l2_memory_with_l1_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        store
            .record_event(
                MemoryEventCategory::Chat,
                "answered",
                "User asked for visual explanations of OPC.",
                Some("session-1".into()),
                json!({ "session": "session-1" }),
            )
            .unwrap();
        let markdown = "# Chat memory\n\n- Learner asks for visual OPC explanations. [^1] <!--m_1-->\n\n---\n\n[^1]: chat:session-1";

        let input = store
            .consolidation_input(
                "L2/chat.md",
                MemoryAssistAction::Check,
                Some(markdown.into()),
            )
            .unwrap();

        assert_eq!(input.job.mode, ConsolidationMode::Audit);
        assert_eq!(input.chunk.citeable_refs, vec!["chat:session-1"]);
        assert!(input.chunk.text.contains("# Line-numbered view"));
        assert!(input.chunk.text.contains("## Evidence for line 3"));
        assert!(input.chunk.text.contains("source_kind: L1 event"));
        assert!(
            input
                .chunk
                .text
                .contains("User asked for visual explanations")
        );
    }

    #[test]
    fn check_consolidation_input_annotates_l3_memory_with_l2_surface_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        store
            .write(
                "L2/chat.md",
                "# Chat memory\n\n- Learner asks for more diagrams. <!--m_chat-->".into(),
            )
            .unwrap();
        let markdown = "# Student profile\n\n- Learner may benefit from visual explanations. [^1] <!--m_1-->\n\n---\n\n[^1]: chat";

        let input = store
            .consolidation_input(
                "L3/profile.md",
                MemoryAssistAction::Check,
                Some(markdown.into()),
            )
            .unwrap();

        assert_eq!(input.job.mode, ConsolidationMode::Audit);
        assert_eq!(input.chunk.citeable_refs, vec!["chat"]);
        assert!(input.chunk.text.contains("source_kind: L2 surface memory"));
        assert!(input.chunk.text.contains("source_path: L2/chat.md"));
        assert!(input.chunk.text.contains("Learner asks for more diagrams"));
    }

    #[test]
    fn dedupe_consolidation_input_uses_line_numbered_memory() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        let markdown = "# Quiz memory\n\n## Weak topics\n\n- Same fact. [^1] <!--m_1-->\n- Same fact. [^2] <!--m_2-->\n\n---\n\n[^1]: quiz:q1\n[^2]: quiz:q2";

        let input = store
            .consolidation_input(
                "L2/quiz.md",
                MemoryAssistAction::Dedupe,
                Some(markdown.into()),
            )
            .unwrap();

        assert_eq!(input.job.mode, ConsolidationMode::Dedup);
        assert_eq!(input.chunk.citeable_refs, vec!["quiz:q1", "quiz:q2"]);
        assert!(input.chunk.text.contains("# Line-numbered view"));
        assert!(
            input
                .chunk
                .text
                .contains("   5: - Same fact. [^1] <!--m_1-->")
        );
        assert!(input.chunk.text.contains("# Existing source refs"));
    }

    #[test]
    fn consolidation_inputs_keep_dedupe_as_single_numbered_chunk() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        let markdown = "# Chat memory\n\n- Duplicate. [^1] <!--m_1-->\n- Duplicate. [^1] <!--m_2-->\n\n[^1]: chat:s1";

        let inputs = store
            .consolidation_inputs(
                "L2/chat.md",
                MemoryAssistAction::Dedupe,
                Some(markdown.into()),
            )
            .unwrap();

        assert_eq!(inputs.len(), 1);
        assert_eq!(inputs[0].chunk.index, 1);
        assert_eq!(inputs[0].chunk.total, 1);
        assert_eq!(inputs[0].chunk.citeable_refs, vec!["chat:s1"]);
        assert!(
            inputs[0]
                .chunk
                .text
                .contains("   4: - Duplicate. [^1] <!--m_2-->")
        );
    }

    #[test]
    fn l3_consolidation_input_uses_l2_memory_surfaces() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        store
            .write(
                "L2/chat.md",
                "# Chat memory\n\n- Learner often asks for visual explanations. <!--m_chat-->"
                    .into(),
            )
            .unwrap();
        store
            .write(
                "L2/quiz.md",
                "# Quiz memory\n\n- Learner missed OPC distractors. <!--m_quiz-->".into(),
            )
            .unwrap();

        let input = store
            .consolidation_input("L3/profile.md", MemoryAssistAction::Update, None)
            .unwrap();

        assert_eq!(input.job.layer, "L3");
        assert_eq!(input.chunk.citeable_refs, vec!["chat", "quiz"]);
        assert!(input.chunk.text.contains("@entity chat"));
        assert!(input.chunk.text.contains("@entity quiz"));
        assert!(!input.chunk.text.contains("chat:"));
    }

    #[test]
    fn consolidation_inputs_chunk_l2_update_events() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        for index in 0..12 {
            store
                .record_event(
                    MemoryEventCategory::Chat,
                    "answered",
                    format!("Chat learning event {index}"),
                    Some(format!("session-{index}")),
                    json!({ "index": index }),
                )
                .unwrap();
        }

        let inputs = store
            .consolidation_inputs("L2/chat.md", MemoryAssistAction::Update, None)
            .unwrap();

        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].chunk.index, 1);
        assert_eq!(inputs[0].chunk.total, 2);
        assert_eq!(inputs[0].chunk.citeable_refs.len(), 10);
        assert_eq!(inputs[1].chunk.index, 2);
        assert_eq!(inputs[1].chunk.total, 2);
        assert_eq!(inputs[1].chunk.citeable_refs.len(), 2);
        assert!(
            inputs
                .iter()
                .all(|input| input.chunk.text.contains("# Chunk-local citeable refs"))
        );
    }

    #[test]
    fn consolidation_inputs_chunk_l3_update_by_surface() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        store
            .write(
                "L2/chat.md",
                "# Chat memory\n\n- Learner often asks for visual explanations. <!--m_chat-->"
                    .into(),
            )
            .unwrap();
        store
            .write(
                "L2/quiz.md",
                "# Quiz memory\n\n- Learner missed OPC distractors. <!--m_quiz-->".into(),
            )
            .unwrap();

        let inputs = store
            .consolidation_inputs("L3/profile.md", MemoryAssistAction::Update, None)
            .unwrap();

        assert_eq!(inputs.len(), 2);
        assert_eq!(inputs[0].chunk.total, 2);
        assert_eq!(inputs[0].chunk.citeable_refs, vec!["chat"]);
        assert_eq!(inputs[1].chunk.citeable_refs, vec!["quiz"]);
        assert!(inputs[0].chunk.text.contains("@entity chat"));
        assert!(inputs[1].chunk.text.contains("@entity quiz"));
    }

    #[test]
    fn knowledge_consolidation_input_uses_knowledge_events_and_l3_surface_refs() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        store
            .record_event(
                MemoryEventCategory::Knowledge,
                "search",
                "Searched semiconductor notes for photoresist.",
                Some("kb-1".into()),
                json!({ "query": "photoresist" }),
            )
            .unwrap();

        let l2_input = store
            .consolidation_input("L2/knowledge.md", MemoryAssistAction::Update, None)
            .unwrap();

        assert_eq!(l2_input.job.key, "knowledge");
        assert_eq!(l2_input.chunk.citeable_refs, vec!["knowledge:kb-1"]);
        assert!(l2_input.chunk.text.contains("@entity knowledge:kb-1"));
        assert!(
            l2_input
                .target
                .allowed_sections
                .contains(&"Frequent queries".into())
        );

        store
            .write(
                "L2/knowledge.md",
                "# Knowledge memory\n\n- Learner searches semiconductor documents. <!--m_kb-->"
                    .into(),
            )
            .unwrap();
        let l3_input = store
            .consolidation_input("L3/scope.md", MemoryAssistAction::Update, None)
            .unwrap();

        assert!(l3_input.chunk.citeable_refs.contains(&"knowledge".into()));
        assert!(l3_input.chunk.text.contains("@entity knowledge"));
    }

    #[test]
    fn append_memory_facts_rejects_unciteable_refs() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        let err = store
            .append_memory_facts(
                "L2/chat.md",
                "# Chat memory\n\n",
                &[MemoryFact {
                    text: "Learner needs more visual examples.".into(),
                    section: "Topics".into(),
                    refs: vec!["chat:missing".into()],
                }],
                &["chat:existing".into()],
                &["Topics".into()],
            )
            .unwrap_err();

        assert!(err.to_string().contains("unknown source ref"));
    }

    #[test]
    fn append_l3_memory_facts_rejects_l1_style_refs() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        let err = store
            .append_memory_facts(
                "L3/profile.md",
                "# Student profile\n\n",
                &[MemoryFact {
                    text: "Learner benefits from visual examples.".into(),
                    section: "Learning style".into(),
                    refs: vec!["chat:session-1".into()],
                }],
                &["chat:session-1".into()],
                &["Learning style".into()],
            )
            .unwrap_err();

        assert!(err.to_string().contains("bare allowed surface"));
    }

    #[test]
    fn append_memory_facts_rejects_unsupported_sections() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        let err = store
            .append_memory_facts(
                "L3/profile.md",
                "# Student profile\n\n",
                &[MemoryFact {
                    text: "Learner needs more visual examples.".into(),
                    section: "Made up".into(),
                    refs: vec!["chat:existing".into()],
                }],
                &["chat:existing".into()],
                &["Learning style".into()],
            )
            .unwrap_err();

        assert!(err.to_string().contains("unsupported section"));
    }

    #[test]
    fn append_memory_facts_rejects_overlong_text() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        let err = store
            .append_memory_facts(
                "L2/quiz.md",
                "# Quiz memory\n\n",
                &[MemoryFact {
                    text: "x".repeat(MAX_MEMORY_FACT_TEXT_CHARS + 1),
                    section: "Weak topics".into(),
                    refs: vec!["quiz:q1".into()],
                }],
                &["quiz:q1".into()],
                &["Weak topics".into()],
            )
            .unwrap_err();

        assert!(err.to_string().contains("too long"));
    }

    #[test]
    fn append_memory_facts_uses_section_serializer_and_shared_refs() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        let current = "# Quiz memory\n\n## Weak topics\n\n- Existing fact. [^1] <!--m_existing-->\n\n---\n\n[^1]: quiz:q1";

        let proposed = store
            .append_memory_facts(
                "L2/quiz.md",
                current,
                &[
                    MemoryFact {
                        text: "Learner still confuses OPC distractors.".into(),
                        section: "Weak topics".into(),
                        refs: vec!["quiz:q1".into()],
                    },
                    MemoryFact {
                        text: "Learner answers lithography basics correctly.".into(),
                        section: "Strong topics".into(),
                        refs: vec!["quiz:q2".into()],
                    },
                ],
                &["quiz:q1".into(), "quiz:q2".into()],
                &["Weak topics".into(), "Strong topics".into()],
            )
            .unwrap();

        assert!(!proposed.contains("Agent update draft"));
        assert!(proposed.contains("## Weak topics"));
        assert!(proposed.contains("## Strong topics"));
        assert!(proposed.contains("Learner still confuses OPC distractors. [^1]"));
        assert!(proposed.contains("Learner answers lithography basics correctly. [^2]"));
        assert_eq!(proposed.matches("[^1]: quiz:q1").count(), 1);
        assert_eq!(proposed.matches("[^2]: quiz:q2").count(), 1);
    }

    #[test]
    fn append_memory_facts_merges_existing_duplicate_refs() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        let current = "# Quiz memory\n\n## Weak topics\n\n- Learner should review OPC distractors. [^1] <!--m_existing-->\n\n---\n\n[^1]: quiz:q1";

        let proposed = store
            .append_memory_facts(
                "L2/quiz.md",
                current,
                &[MemoryFact {
                    text: "Learner should review OPC distractors.".into(),
                    section: "Weak topics".into(),
                    refs: vec!["quiz:q2".into()],
                }],
                &["quiz:q1".into(), "quiz:q2".into()],
                &["Weak topics".into()],
            )
            .unwrap();

        assert_eq!(
            proposed
                .matches("Learner should review OPC distractors")
                .count(),
            1
        );
        assert!(proposed.contains("Learner should review OPC distractors. [^1] [^2]"));
        assert!(proposed.contains("[^1]: quiz:q1"));
        assert!(proposed.contains("[^2]: quiz:q2"));
        assert!(proposed.contains("<!--m_existing-->"));
    }

    #[test]
    fn append_memory_facts_merges_duplicate_facts_in_same_batch() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));

        let proposed = store
            .append_memory_facts(
                "L2/chat.md",
                "# Chat memory\n\n",
                &[
                    MemoryFact {
                        text: "Learner prefers visual examples.".into(),
                        section: "Topics".into(),
                        refs: vec!["chat:s1".into()],
                    },
                    MemoryFact {
                        text: "Learner prefers visual examples".into(),
                        section: "Topics".into(),
                        refs: vec!["chat:s2".into()],
                    },
                ],
                &["chat:s1".into(), "chat:s2".into()],
                &["Topics".into()],
            )
            .unwrap();

        assert_eq!(
            proposed.matches("Learner prefers visual examples").count(),
            1
        );
        assert!(proposed.contains("[^1]: chat:s1"));
        assert!(proposed.contains("[^2]: chat:s2"));
    }

    #[test]
    fn apply_text_edits_deletes_and_replaces_by_original_line_numbers() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        let current =
            "# Quiz memory\n\n- Same fact. <!--m_1-->\n- Same fact. <!--m_2-->\n- Keep this.";

        let proposed = store
            .apply_text_edits(
                current,
                &[
                    MemoryTextEdit {
                        op: MemoryTextEditOp::Replace,
                        start_line: 5,
                        end_line: Some(5),
                        text: Some("- Keep this useful fact.".into()),
                        refs: vec!["quiz:q1".into()],
                        reason: Some("clearer wording".into()),
                    },
                    MemoryTextEdit {
                        op: MemoryTextEditOp::Delete,
                        start_line: 4,
                        end_line: Some(4),
                        text: None,
                        refs: vec![],
                        reason: None,
                    },
                ],
            )
            .unwrap();

        assert!(proposed.contains("- Same fact. <!--m_1-->"));
        assert!(!proposed.contains("<!--m_2-->"));
        assert!(proposed.contains("- Keep this useful fact."));
    }

    #[test]
    fn validate_text_edits_rejects_unknown_refs() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        let current = "# Chat memory\n\n- Existing fact. [^1] <!--m_1-->\n\n[^1]: chat:known";

        let err = store
            .validate_text_edits(
                current,
                &[MemoryTextEdit {
                    op: MemoryTextEditOp::Delete,
                    start_line: 3,
                    end_line: Some(3),
                    text: None,
                    refs: vec!["chat:unknown".into()],
                    reason: Some("unsupported".into()),
                }],
                &["chat:known".into()],
            )
            .unwrap_err();

        assert!(err.to_string().contains("unknown source ref"));
    }

    #[test]
    fn validate_text_edits_requires_refs_for_replace_when_evidence_exists() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        let current = "# Chat memory\n\n- Existing fact. [^1] <!--m_1-->\n\n[^1]: chat:known";

        let err = store
            .validate_text_edits(
                current,
                &[MemoryTextEdit {
                    op: MemoryTextEditOp::Replace,
                    start_line: 3,
                    end_line: Some(3),
                    text: Some("- Better fact. [^1] <!--m_1-->".into()),
                    refs: vec![],
                    reason: Some("needs evidence".into()),
                }],
                &["chat:known".into()],
            )
            .unwrap_err();

        assert!(err.to_string().contains("must cite evidence refs"));
    }

    #[test]
    fn validate_text_edits_requires_reason() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));

        let err = store
            .validate_text_edits(
                "# Chat memory\n\n- Existing fact. <!--m_1-->",
                &[MemoryTextEdit {
                    op: MemoryTextEditOp::Delete,
                    start_line: 3,
                    end_line: Some(3),
                    text: None,
                    refs: vec![],
                    reason: None,
                }],
                &[],
            )
            .unwrap_err();

        assert!(err.to_string().contains("requires a reason"));
    }

    #[test]
    fn validate_text_edits_rejects_protected_lines() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        let current = "# Chat memory\n\n## Topics\n\n- Editable fact. [^1] <!--m_1-->\n\n---\n\n[^1]: chat:known";

        for (line, label) in [
            (1, "title"),
            (2, "blank"),
            (3, "section"),
            (7, "separator"),
            (8, "ref"),
        ] {
            let err = store
                .validate_text_edits(
                    current,
                    &[MemoryTextEdit {
                        op: MemoryTextEditOp::Delete,
                        start_line: line,
                        end_line: Some(line),
                        text: None,
                        refs: vec![],
                        reason: Some(format!("delete {label}")),
                    }],
                    &["chat:known".into()],
                )
                .unwrap_err();

            assert!(
                err.to_string().contains("protected line"),
                "expected protected-line error for {label}, got {err}"
            );
        }
    }

    #[test]
    fn validate_dedupe_edits_rejects_insert() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        let current = "# Quiz memory\n\n- Existing fact. [^1] <!--m_1-->\n\n[^1]: quiz:q1";

        let err = store
            .validate_text_edits_for_action(
                MemoryAssistAction::Dedupe,
                current,
                &[MemoryTextEdit {
                    op: MemoryTextEditOp::Insert,
                    start_line: 4,
                    end_line: None,
                    text: Some("- New fact. [^1] <!--m_new-->".into()),
                    refs: vec!["quiz:q1".into()],
                    reason: Some("not allowed in dedupe".into()),
                }],
                &["quiz:q1".into()],
            )
            .unwrap_err();

        assert!(err.to_string().contains("must not insert"));
    }

    #[test]
    fn apply_text_edits_rejects_out_of_bounds_line() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::new_with_root(dir.path().join("memory"));
        let err = store
            .apply_text_edits(
                "# Profile\n",
                &[MemoryTextEdit {
                    op: MemoryTextEditOp::Delete,
                    start_line: 5,
                    end_line: Some(5),
                    text: None,
                    refs: vec![],
                    reason: None,
                }],
            )
            .unwrap_err();

        assert!(err.to_string().contains("out of bounds"));
    }
}
