use std::{fs, path::PathBuf, sync::Mutex};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const GENERAL_TUTOR_ID: &str = "general-tutor";
pub const MAX_SOUL_CHARS: usize = 16_000;

const SUPPORTED_CAPABILITIES: &[&str] = &["chat", "code_exec", "quiz", "research", "organize"];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TutorResourcePermissions {
    #[serde(default)]
    pub knowledge_base_ids: Vec<String>,
    #[serde(default = "default_true")]
    pub notebook: bool,
    #[serde(default = "default_true")]
    pub space: bool,
}

impl Default for TutorResourcePermissions {
    fn default() -> Self {
        Self {
            knowledge_base_ids: Vec::new(),
            notebook: true,
            space: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TutorProfile {
    pub id: String,
    pub name: String,
    pub soul_markdown: String,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub default_model_config_id: Option<String>,
    pub default_capability: String,
    pub allowed_capabilities: Vec<String>,
    #[serde(default = "default_true")]
    pub learner_memory_access: bool,
    #[serde(default)]
    pub resource_permissions: TutorResourcePermissions,
    #[serde(default = "default_true")]
    pub autonomous_memory: bool,
    #[serde(default)]
    pub built_in: bool,
    #[serde(default)]
    pub archived: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CreateTutorProfile {
    pub name: String,
    pub soul_markdown: String,
    #[serde(default)]
    pub avatar: Option<String>,
    #[serde(default)]
    pub default_model_config_id: Option<String>,
    #[serde(default = "default_capability")]
    pub default_capability: String,
    #[serde(default = "default_allowed_capabilities")]
    pub allowed_capabilities: Vec<String>,
    #[serde(default = "default_true")]
    pub learner_memory_access: bool,
    #[serde(default)]
    pub resource_permissions: TutorResourcePermissions,
    #[serde(default = "default_true")]
    pub autonomous_memory: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct UpdateTutorProfile {
    pub name: Option<String>,
    pub soul_markdown: Option<String>,
    pub avatar: Option<Option<String>>,
    pub default_model_config_id: Option<Option<String>>,
    pub default_capability: Option<String>,
    pub allowed_capabilities: Option<Vec<String>>,
    pub learner_memory_access: Option<bool>,
    pub resource_permissions: Option<TutorResourcePermissions>,
    pub autonomous_memory: Option<bool>,
    pub archived: Option<bool>,
}

#[derive(Debug, Error)]
pub enum TutorStoreError {
    #[error("tutor not found")]
    NotFound,
    #[error("built-in tutor cannot be deleted")]
    BuiltInTutor,
    #[error("{0}")]
    Validation(String),
    #[error(transparent)]
    Storage(#[from] anyhow::Error),
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct TutorFile {
    #[serde(default = "schema_version")]
    schema_version: u32,
    #[serde(default)]
    tutors: Vec<TutorProfile>,
}

pub struct TutorStore {
    root: PathBuf,
    path: PathBuf,
    value: Mutex<TutorFile>,
}

impl TutorStore {
    pub fn new_with_root(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        fs::create_dir_all(&root).expect("failed to create tutor store directory");
        let path = root.join("tutors.json");
        let (mut value, migrated) = fs::read_to_string(&path)
            .ok()
            .and_then(|text| load_file(&text).ok())
            .unwrap_or_default();
        let retired_capabilities_migrated = migrate_retired_capabilities(&mut value);
        if !value.tutors.iter().any(|item| item.id == GENERAL_TUTOR_ID) {
            value.tutors.push(general_tutor());
            save_file(&path, &value).expect("failed to seed General Tutor");
        } else if migrated || retired_capabilities_migrated {
            save_file(&path, &value).expect("failed to migrate tutor store");
        }
        Self {
            root,
            path,
            value: Mutex::new(value),
        }
    }

    pub fn list(&self, include_archived: bool) -> Vec<TutorProfile> {
        let mut items = self
            .value
            .lock()
            .unwrap()
            .tutors
            .iter()
            .filter(|item| include_archived || !item.archived)
            .cloned()
            .collect::<Vec<_>>();
        items.sort_by(|left, right| {
            right
                .built_in
                .cmp(&left.built_in)
                .then_with(|| left.created_at.cmp(&right.created_at))
        });
        items
    }

    pub fn get(&self, id: &str) -> Option<TutorProfile> {
        self.value
            .lock()
            .unwrap()
            .tutors
            .iter()
            .find(|item| item.id == id)
            .cloned()
    }

    pub fn get_available(&self, id: &str) -> Option<TutorProfile> {
        self.get(id).filter(|item| !item.archived)
    }

    pub fn create(&self, input: CreateTutorProfile) -> Result<TutorProfile, TutorStoreError> {
        let now = Utc::now();
        let profile = TutorProfile {
            id: uuid::Uuid::new_v4().to_string(),
            name: clean_required(input.name, "tutor name")?,
            soul_markdown: clean_soul(input.soul_markdown)?,
            avatar: clean_optional(input.avatar),
            default_model_config_id: clean_optional(input.default_model_config_id),
            default_capability: input.default_capability.trim().to_string(),
            allowed_capabilities: normalize_capabilities(input.allowed_capabilities)?,
            learner_memory_access: input.learner_memory_access,
            resource_permissions: normalize_permissions(input.resource_permissions),
            autonomous_memory: input.autonomous_memory,
            built_in: false,
            archived: false,
            created_at: now,
            updated_at: now,
        };
        validate_capability_policy(&profile.default_capability, &profile.allowed_capabilities)?;

        let mut value = self.value.lock().unwrap();
        value.tutors.push(profile.clone());
        self.save_locked(&value)?;
        Ok(profile)
    }

    pub fn update(
        &self,
        id: &str,
        input: UpdateTutorProfile,
    ) -> Result<TutorProfile, TutorStoreError> {
        let mut value = self.value.lock().unwrap();
        let Some(index) = value.tutors.iter().position(|item| item.id == id) else {
            return Err(TutorStoreError::NotFound);
        };
        let mut profile = value.tutors[index].clone();
        if let Some(name) = input.name {
            profile.name = clean_required(name, "tutor name")?;
        }
        if let Some(soul_markdown) = input.soul_markdown {
            profile.soul_markdown = clean_soul(soul_markdown)?;
        }
        if let Some(avatar) = input.avatar {
            profile.avatar = clean_optional(avatar);
        }
        if let Some(config_id) = input.default_model_config_id {
            profile.default_model_config_id = clean_optional(config_id);
        }
        if let Some(capability) = input.default_capability {
            profile.default_capability = capability.trim().to_string();
        }
        if let Some(capabilities) = input.allowed_capabilities {
            profile.allowed_capabilities = normalize_capabilities(capabilities)?;
        }
        if let Some(access) = input.learner_memory_access {
            profile.learner_memory_access = access;
        }
        if let Some(permissions) = input.resource_permissions {
            profile.resource_permissions = normalize_permissions(permissions);
        }
        if let Some(enabled) = input.autonomous_memory {
            profile.autonomous_memory = enabled;
        }
        if let Some(archived) = input.archived {
            if profile.built_in && archived {
                return Err(TutorStoreError::BuiltInTutor);
            }
            profile.archived = archived;
        }
        validate_capability_policy(&profile.default_capability, &profile.allowed_capabilities)?;
        profile.updated_at = Utc::now();
        value.tutors[index] = profile.clone();
        self.save_locked(&value)?;
        Ok(profile)
    }

    pub fn archive(&self, id: &str) -> Result<(), TutorStoreError> {
        let profile = self.get(id).ok_or(TutorStoreError::NotFound)?;
        if profile.built_in {
            return Err(TutorStoreError::BuiltInTutor);
        }
        self.update(
            id,
            UpdateTutorProfile {
                archived: Some(true),
                ..Default::default()
            },
        )?;
        Ok(())
    }

    pub fn reset_general_tutor(&self, id: &str) -> Result<TutorProfile, TutorStoreError> {
        if id != GENERAL_TUTOR_ID {
            return Err(TutorStoreError::Validation(
                "only the built-in General Tutor can reset its profile".into(),
            ));
        }
        let mut value = self.value.lock().unwrap();
        let Some(index) = value.tutors.iter().position(|item| item.id == id) else {
            return Err(TutorStoreError::NotFound);
        };
        let created_at = value.tutors[index].created_at;
        let mut profile = general_tutor();
        profile.created_at = created_at;
        profile.updated_at = Utc::now();
        value.tutors[index] = profile.clone();
        self.save_locked(&value)?;
        Ok(profile)
    }

    fn save_locked(&self, value: &TutorFile) -> Result<(), TutorStoreError> {
        save_file(&self.path, value)?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn root(&self) -> &PathBuf {
        &self.root
    }
}

fn general_tutor() -> TutorProfile {
    let now = Utc::now();
    TutorProfile {
        id: GENERAL_TUTOR_ID.into(),
        name: "通用导师".into(),
        soul_markdown: default_general_soul(),
        avatar: None,
        default_model_config_id: None,
        default_capability: "chat".into(),
        allowed_capabilities: default_allowed_capabilities(),
        learner_memory_access: true,
        resource_permissions: TutorResourcePermissions::default(),
        autonomous_memory: true,
        built_in: true,
        archived: false,
        created_at: now,
        updated_at: now,
    }
}

fn save_file(path: &PathBuf, value: &TutorFile) -> anyhow::Result<()> {
    let temp = path.with_extension(format!("json.{}.tmp", uuid::Uuid::new_v4()));
    fs::write(&temp, serde_json::to_vec_pretty(value)?)?;
    if let Err(rename_error) = fs::rename(&temp, path) {
        fs::copy(&temp, path).map_err(|_| rename_error)?;
        let _ = fs::remove_file(&temp);
    }
    Ok(())
}

fn load_file(text: &str) -> anyhow::Result<(TutorFile, bool)> {
    let mut value = serde_json::from_str::<serde_json::Value>(text)?;
    let mut migrated = false;
    if let Some(tutors) = value
        .get_mut("tutors")
        .and_then(serde_json::Value::as_array_mut)
    {
        for tutor in tutors {
            let Some(profile) = tutor.as_object_mut() else {
                continue;
            };
            let missing_soul = profile
                .get("soul_markdown")
                .and_then(serde_json::Value::as_str)
                .is_none_or(|soul| soul.trim().is_empty());
            if missing_soul {
                let role = profile
                    .get("role")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                profile.insert(
                    "soul_markdown".into(),
                    serde_json::Value::String(legacy_soul(&role)),
                );
                migrated = true;
            }
            migrated |= profile.remove("role").is_some();
            migrated |= profile.remove("goal").is_some();
        }
    }
    if value
        .get("schema_version")
        .and_then(serde_json::Value::as_u64)
        != Some(2)
    {
        value["schema_version"] = serde_json::json!(2);
        migrated = true;
    }
    Ok((serde_json::from_value(value)?, migrated))
}

fn clean_required(value: String, label: &str) -> Result<String, TutorStoreError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(TutorStoreError::Validation(format!("{label} is required")));
    }
    Ok(value)
}

fn clean_soul(value: String) -> Result<String, TutorStoreError> {
    let value = clean_required(value, "tutor soul")?;
    if value.chars().count() > MAX_SOUL_CHARS {
        return Err(TutorStoreError::Validation(format!(
            "tutor soul exceeds {MAX_SOUL_CHARS} characters"
        )));
    }
    Ok(value)
}

fn legacy_soul(role: &str) -> String {
    let role = role.trim();
    if role.is_empty() {
        "# 核心身份\n\n请根据学习者的需要提供清晰、可靠的教学帮助。".into()
    } else {
        format!("# 核心身份\n\n{role}")
    }
}

fn default_general_soul() -> String {
    "# 核心身份\n\n你是一位通用学习导师，帮助学习者理解当前问题并持续推进学习。\n\n# 教学风格\n\n- 先确认学习者真正想解决的问题。\n- 使用清晰解释、追问和练习促进理解。\n- 根据学习者的反馈调整讲解深度与节奏。\n\n# 教学原则\n\n- 区分事实、推测和建议。\n- 不假装学习者已经理解。\n- 复杂问题先建立直觉，再展开细节。\n\n# 边界\n\n- 不记录敏感个人信息。\n- 不在证据不足时评价学习者的能力。"
        .into()
}

fn clean_optional(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn normalize_capabilities(values: Vec<String>) -> Result<Vec<String>, TutorStoreError> {
    let mut result = Vec::new();
    for value in values {
        let value = value.trim().to_string();
        if !SUPPORTED_CAPABILITIES.contains(&value.as_str()) {
            return Err(TutorStoreError::Validation(format!(
                "unsupported capability: {value}"
            )));
        }
        if !result.contains(&value) {
            result.push(value);
        }
    }
    if result.is_empty() {
        return Err(TutorStoreError::Validation(
            "at least one capability is required".into(),
        ));
    }
    Ok(result)
}

fn validate_capability_policy(
    default_capability: &str,
    allowed_capabilities: &[String],
) -> Result<(), TutorStoreError> {
    if !allowed_capabilities
        .iter()
        .any(|item| item == default_capability)
    {
        return Err(TutorStoreError::Validation(
            "default capability must be allowed".into(),
        ));
    }
    Ok(())
}

fn normalize_permissions(mut permissions: TutorResourcePermissions) -> TutorResourcePermissions {
    permissions.knowledge_base_ids = permissions
        .knowledge_base_ids
        .into_iter()
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
        .fold(Vec::new(), |mut values, item| {
            if !values.contains(&item) {
                values.push(item);
            }
            values
        });
    permissions
}

fn default_true() -> bool {
    true
}

fn schema_version() -> u32 {
    2
}

fn default_capability() -> String {
    "chat".into()
}

fn default_allowed_capabilities() -> Vec<String> {
    ["chat", "quiz", "research", "organize"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

fn migrate_retired_capabilities(value: &mut TutorFile) -> bool {
    let mut changed = false;
    for tutor in &mut value.tutors {
        let previous_len = tutor.allowed_capabilities.len();
        tutor
            .allowed_capabilities
            .retain(|capability| capability != "deep_solve");
        changed |= tutor.allowed_capabilities.len() != previous_len;
        if tutor.allowed_capabilities.is_empty() {
            tutor.allowed_capabilities.push("chat".into());
            changed = true;
        }
        if tutor.default_capability == "deep_solve"
            || !tutor
                .allowed_capabilities
                .iter()
                .any(|capability| capability == &tutor.default_capability)
        {
            tutor.default_capability = "chat".into();
            if !tutor
                .allowed_capabilities
                .iter()
                .any(|capability| capability == "chat")
            {
                tutor.allowed_capabilities.push("chat".into());
            }
            changed = true;
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_input(name: &str) -> CreateTutorProfile {
        CreateTutorProfile {
            name: name.into(),
            soul_markdown: "# Identity\n\nTeach carefully.".into(),
            avatar: None,
            default_model_config_id: None,
            default_capability: "chat".into(),
            allowed_capabilities: vec!["chat".into(), "quiz".into()],
            learner_memory_access: true,
            resource_permissions: TutorResourcePermissions::default(),
            autonomous_memory: true,
        }
    }

    #[test]
    fn seeds_general_tutor_idempotently() {
        let dir = tempfile::tempdir().unwrap();
        let store = TutorStore::new_with_root(dir.path());
        assert_eq!(store.list(false).len(), 1);
        drop(store);

        let reopened = TutorStore::new_with_root(dir.path());
        assert_eq!(reopened.list(false).len(), 1);
        assert!(reopened.get(GENERAL_TUTOR_ID).unwrap().built_in);
    }

    #[test]
    fn migrates_retired_deep_solve_capability_to_chat() {
        let dir = tempfile::tempdir().unwrap();
        let mut legacy = general_tutor();
        legacy.default_capability = "deep_solve".into();
        legacy.allowed_capabilities = vec!["deep_solve".into(), "research".into()];
        save_file(
            &dir.path().join("tutors.json"),
            &TutorFile {
                schema_version: schema_version(),
                tutors: vec![legacy],
            },
        )
        .unwrap();

        let store = TutorStore::new_with_root(dir.path());
        let migrated = store.get(GENERAL_TUTOR_ID).unwrap();
        assert_eq!(migrated.default_capability, "chat");
        assert_eq!(
            migrated.allowed_capabilities,
            vec!["research".to_string(), "chat".to_string()]
        );
    }

    #[test]
    fn persists_create_update_and_archive() {
        let dir = tempfile::tempdir().unwrap();
        let store = TutorStore::new_with_root(dir.path());
        let created = store.create(test_input("Math Tutor")).unwrap();
        store
            .update(
                &created.id,
                UpdateTutorProfile {
                    soul_markdown: Some("# Identity\n\nTeach algebra carefully.".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        drop(store);

        let reopened = TutorStore::new_with_root(dir.path());
        assert!(
            reopened
                .get(&created.id)
                .unwrap()
                .soul_markdown
                .contains("algebra")
        );
        reopened.archive(&created.id).unwrap();
        assert!(reopened.get_available(&created.id).is_none());
        assert!(
            reopened
                .list(false)
                .iter()
                .all(|item| item.id != created.id)
        );
        assert!(reopened.list(true).iter().any(|item| item.id == created.id));
    }

    #[test]
    fn rejects_invalid_capability_policy_and_builtin_archive() {
        let dir = tempfile::tempdir().unwrap();
        let store = TutorStore::new_with_root(dir.path());
        let mut input = test_input("Invalid");
        input.default_capability = "research".into();
        assert!(matches!(
            store.create(input),
            Err(TutorStoreError::Validation(_))
        ));
        assert!(matches!(
            store.archive(GENERAL_TUTOR_ID),
            Err(TutorStoreError::BuiltInTutor)
        ));
    }

    #[test]
    fn migrates_only_stable_legacy_role_to_soul_markdown() {
        let dir = tempfile::tempdir().unwrap();
        let now = Utc::now().to_rfc3339();
        fs::write(
            dir.path().join("tutors.json"),
            format!(
                r#"{{"schema_version":1,"tutors":[{{"id":"legacy","name":"Legacy","role":"Teach math","goal":"Learn algebra","default_capability":"chat","allowed_capabilities":["chat"],"created_at":"{now}","updated_at":"{now}"}}]}}"#
            ),
        )
        .unwrap();

        let store = TutorStore::new_with_root(dir.path());
        let tutor = store.get("legacy").unwrap();
        assert!(tutor.soul_markdown.contains("Teach math"));
        assert!(!tutor.soul_markdown.contains("Learn algebra"));

        let persisted = fs::read_to_string(dir.path().join("tutors.json")).unwrap();
        assert!(persisted.contains("soul_markdown"));
        assert!(!persisted.contains("\"role\""));
        assert!(!persisted.contains("\"goal\""));
    }
}
