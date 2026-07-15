use std::{fs, path::PathBuf, sync::Mutex};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const GENERAL_TUTOR_ID: &str = "general-tutor";

const SUPPORTED_CAPABILITIES: &[&str] = &[
    "chat",
    "deep_solve",
    "code_exec",
    "quiz",
    "research",
    "organize",
];

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
    pub role: String,
    pub goal: String,
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
    pub role: String,
    #[serde(default)]
    pub goal: String,
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
    pub role: Option<String>,
    pub goal: Option<String>,
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
        let mut value = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<TutorFile>(&text).ok())
            .unwrap_or_default();
        if !value.tutors.iter().any(|item| item.id == GENERAL_TUTOR_ID) {
            value.tutors.push(general_tutor());
            save_file(&path, &value).expect("failed to seed General Tutor");
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
            role: clean_required(input.role, "tutor role")?,
            goal: input.goal.trim().to_string(),
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
        if let Some(role) = input.role {
            profile.role = clean_required(role, "tutor role")?;
        }
        if let Some(goal) = input.goal {
            profile.goal = goal.trim().to_string();
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
        role: "根据学习目标提供清晰讲解、追问、练习与阶段性建议。".into(),
        goal: "帮助用户理解当前问题并持续推进学习。".into(),
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

fn clean_required(value: String, label: &str) -> Result<String, TutorStoreError> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(TutorStoreError::Validation(format!("{label} is required")));
    }
    Ok(value)
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
    1
}

fn default_capability() -> String {
    "chat".into()
}

fn default_allowed_capabilities() -> Vec<String> {
    ["chat", "deep_solve", "quiz", "research", "organize"]
        .into_iter()
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_input(name: &str) -> CreateTutorProfile {
        CreateTutorProfile {
            name: name.into(),
            role: "Teach carefully".into(),
            goal: "Learn".into(),
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
    fn persists_create_update_and_archive() {
        let dir = tempfile::tempdir().unwrap();
        let store = TutorStore::new_with_root(dir.path());
        let created = store.create(test_input("Math Tutor")).unwrap();
        store
            .update(
                &created.id,
                UpdateTutorProfile {
                    goal: Some("Master algebra".into()),
                    ..Default::default()
                },
            )
            .unwrap();
        drop(store);

        let reopened = TutorStore::new_with_root(dir.path());
        assert_eq!(reopened.get(&created.id).unwrap().goal, "Master algebra");
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
}
