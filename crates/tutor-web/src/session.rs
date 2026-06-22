use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use llm_harness_agent::{
    JsonlSessionRepo, Session, SessionRepo,
    session::{CreateSessionOptions, ListSessionOptions, SessionMetadata},
};

use crate::stream::TutorStream;

#[derive(Clone, Debug)]
pub struct LlmSessionConfig {
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub chat_path: Option<String>,
    pub budget_limit_usd: Option<f64>,
    pub require_approval: bool,
}

/// Product metadata for an active tutor session.
#[derive(Clone)]
pub struct SessionEntry {
    pub id: String,
    pub capability: String,
    pub kb: Option<String>,
    pub llm: Option<LlmSessionConfig>,
    pub embedding: Option<tutor_rag::EmbeddingConfig>,
    pub stream: TutorStream,
}

#[derive(Clone)]
pub struct RuntimeSessionSummary {
    pub id: String,
    pub name: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub model: Option<String>,
}

/// Thread-safe pool of active web session metadata plus runtime session repo.
pub struct SessionPool {
    sessions: Mutex<HashMap<String, SessionEntry>>,
    repo: Arc<JsonlSessionRepo>,
}

impl SessionPool {
    pub fn new() -> Arc<Self> {
        let root = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".llm-tutor")
            .join("sessions");
        Self::new_with_root(root)
    }

    pub fn new_with_root(root: impl Into<PathBuf>) -> Arc<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root).expect("failed to create runtime session directory");
        Arc::new(Self {
            sessions: Mutex::new(HashMap::new()),
            repo: Arc::new(JsonlSessionRepo::new(root)),
        })
    }

    /// Create a runtime-backed session and return its ID.
    pub async fn create(
        &self,
        capability: &str,
        kb: Option<String>,
        llm: Option<LlmSessionConfig>,
        embedding: Option<tutor_rag::EmbeddingConfig>,
    ) -> Result<String, llm_harness_types::SessionError> {
        let storage = self
            .repo
            .create(CreateSessionOptions {
                name: None,
                initial_model: llm.as_ref().map(|config| config.model.clone()),
                initial_thinking_level: None,
                initial_tools: vec![],
            })
            .await?;
        let meta = storage.metadata().await?;
        let id = meta.id.clone();
        let entry = SessionEntry {
            id: id.clone(),
            capability: capability.to_string(),
            kb,
            llm,
            embedding,
            stream: TutorStream::new(128),
        };
        self.sessions.lock().unwrap().insert(id.clone(), entry);
        Ok(id)
    }

    pub fn get(&self, id: &str) -> Option<SessionEntry> {
        self.sessions.lock().unwrap().get(id).cloned()
    }

    pub async fn ensure_entry(&self, id: &str) -> Option<SessionEntry> {
        if let Some(entry) = self.get(id) {
            return Some(entry);
        }

        let storage = self.repo.open(id).await.ok()?;
        let meta = storage.metadata().await.ok()?;
        let entry = SessionEntry {
            id: meta.id.clone(),
            capability: "chat".into(),
            kb: None,
            llm: None,
            embedding: None,
            stream: TutorStream::new(128),
        };
        self.sessions
            .lock()
            .unwrap()
            .insert(meta.id.clone(), entry.clone());
        Some(entry)
    }

    pub async fn open_runtime_session(
        &self,
        id: &str,
    ) -> Result<Session, llm_harness_types::SessionError> {
        let storage = self.repo.open(id).await?;
        Ok(Session::new(storage))
    }

    pub async fn metadata(
        &self,
        id: &str,
    ) -> Result<SessionMetadata, llm_harness_types::SessionError> {
        self.repo.open(id).await?.metadata().await
    }

    pub async fn history_len(&self, id: &str) -> usize {
        match self.open_runtime_session(id).await {
            Ok(session) => session
                .build_context()
                .await
                .map(|ctx| ctx.messages.len())
                .unwrap_or(0),
            Err(_) => 0,
        }
    }

    pub async fn messages(
        &self,
        id: &str,
    ) -> Result<Vec<llm_harness_types::AgentMessage>, llm_harness_types::SessionError> {
        Ok(self
            .open_runtime_session(id)
            .await?
            .build_context()
            .await?
            .messages)
    }

    pub async fn list(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<RuntimeSessionSummary>, llm_harness_types::SessionError> {
        let metas = self
            .repo
            .list(ListSessionOptions {
                limit,
                ..Default::default()
            })
            .await?;
        Ok(metas
            .into_iter()
            .map(|meta| RuntimeSessionSummary {
                id: meta.id,
                name: meta.name,
                created_at: meta.created_at,
                updated_at: meta.updated_at,
                model: meta.model,
            })
            .collect())
    }

    pub fn set_capability(&self, id: &str, capability: &str) -> bool {
        let mut sessions = self.sessions.lock().unwrap();
        let Some(entry) = sessions.get_mut(id) else {
            return false;
        };

        entry.capability = capability.to_string();
        true
    }

    pub fn set_knowledge(
        &self,
        id: &str,
        kb: Option<String>,
        embedding: Option<tutor_rag::EmbeddingConfig>,
    ) -> bool {
        let mut sessions = self.sessions.lock().unwrap();
        let Some(entry) = sessions.get_mut(id) else {
            return false;
        };

        entry.kb = kb;
        entry.embedding = embedding;
        true
    }

    pub async fn rename(
        &self,
        id: &str,
        name: Option<String>,
    ) -> Result<(), llm_harness_types::SessionError> {
        let storage = self.repo.open(id).await?;
        storage.update_metadata_name(name).await
    }

    pub async fn delete(&self, id: &str) -> Result<(), llm_harness_types::SessionError> {
        self.repo.delete(id).await?;
        self.sessions.lock().unwrap().remove(id);
        Ok(())
    }
}

pub fn message_text(message: &llm_harness_types::AgentMessage) -> String {
    let content = match message {
        llm_harness_types::AgentMessage::User(message) => &message.content,
        llm_harness_types::AgentMessage::Assistant(message) => &message.content,
        _ => return String::new(),
    };

    content
        .iter()
        .filter_map(|block| match block {
            llm_harness_types::ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn message_role(message: &llm_harness_types::AgentMessage) -> Option<&'static str> {
    match message {
        llm_harness_types::AgentMessage::User(_) => Some("user"),
        llm_harness_types::AgentMessage::Assistant(_) => Some("assistant"),
        _ => None,
    }
}

impl Default for SessionPool {
    fn default() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            repo: Arc::new(JsonlSessionRepo::new(".llm-tutor/sessions")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_pool() -> Arc<SessionPool> {
        SessionPool::new_with_root(
            std::env::temp_dir().join(format!("llm-tutor-test-{}", uuid::Uuid::new_v4())),
        )
    }

    #[tokio::test]
    async fn session_pool_creates_and_retrieves() {
        let pool = test_pool();
        let id = pool.create("chat", None, None, None).await.unwrap();
        let entry = pool.get(&id);
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.capability, "chat");
        assert_eq!(entry.id, id);
    }

    #[tokio::test]
    async fn runtime_session_persists_messages() {
        let pool = test_pool();
        let id = pool.create("chat", None, None, None).await.unwrap();
        let session = pool.open_runtime_session(&id).await.unwrap();

        session
            .append_message(tutor_agent::chat::user_message("hello"))
            .await
            .unwrap();

        let reopened = pool.open_runtime_session(&id).await.unwrap();
        let ctx = reopened.build_context().await.unwrap();
        assert_eq!(ctx.messages.len(), 1);
    }

    #[tokio::test]
    async fn session_pool_updates_capability_without_replacing_runtime_session() {
        let pool = test_pool();
        let id = pool.create("chat", None, None, None).await.unwrap();
        assert!(pool.set_capability(&id, "code_exec"));

        let updated = pool.get(&id).unwrap();
        assert_eq!(updated.capability, "code_exec");
        assert!(pool.open_runtime_session(&id).await.is_ok());
    }

    #[tokio::test]
    async fn session_pool_updates_knowledge_binding() {
        let pool = test_pool();
        let id = pool.create("chat", None, None, None).await.unwrap();
        assert!(pool.set_knowledge(
            &id,
            Some("kb-1".into()),
            Some(tutor_rag::EmbeddingConfig {
                provider: "openai".into(),
                model: "text-embedding-3-small".into(),
                api_key: "sk-test".into(),
                base_url: None,
                embeddings_path: None,
                dimensions: Some(1536),
                send_dimensions: false,
            }),
        ));

        let updated = pool.get(&id).unwrap();
        assert_eq!(updated.kb.as_deref(), Some("kb-1"));
        assert_eq!(
            updated
                .embedding
                .as_ref()
                .map(|config| config.model.as_str()),
            Some("text-embedding-3-small")
        );
    }

    #[tokio::test]
    async fn list_returns_runtime_sessions() {
        let pool = test_pool();
        pool.create("chat", None, None, None).await.unwrap();

        let sessions = pool.list(Some(10)).await.unwrap();
        assert_eq!(sessions.len(), 1);
    }

    #[tokio::test]
    async fn rename_updates_runtime_metadata() {
        let pool = test_pool();
        let id = pool.create("chat", None, None, None).await.unwrap();

        pool.rename(&id, Some("Algebra review".into()))
            .await
            .unwrap();

        let meta = pool.metadata(&id).await.unwrap();
        assert_eq!(meta.name.as_deref(), Some("Algebra review"));
    }

    #[tokio::test]
    async fn delete_removes_runtime_session() {
        let pool = test_pool();
        let id = pool.create("chat", None, None, None).await.unwrap();

        pool.delete(&id).await.unwrap();

        assert!(pool.get(&id).is_none());
        assert!(pool.open_runtime_session(&id).await.is_err());
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let pool = test_pool();
        assert!(pool.get("nonexistent-id").is_none());
    }
}
