use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use crate::stream::{StreamEvent, TutorStream};

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

/// Metadata for an active tutor session.
#[derive(Clone)]
pub struct SessionEntry {
    pub id: String,
    pub capability: String,
    pub kb: Option<String>,
    pub llm: Option<LlmSessionConfig>,
    pub stream: TutorStream,
}

/// Thread-safe pool of active sessions.
pub struct SessionPool {
    sessions: Mutex<HashMap<String, SessionEntry>>,
    receivers: Mutex<HashMap<String, tokio::sync::mpsc::Receiver<StreamEvent>>>,
}

impl SessionPool {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            sessions: Mutex::new(HashMap::new()),
            receivers: Mutex::new(HashMap::new()),
        })
    }

    /// Create a new session and return its ID.
    pub fn create(
        &self,
        capability: &str,
        kb: Option<String>,
        llm: Option<LlmSessionConfig>,
    ) -> String {
        let id = Uuid::new_v4().to_string();
        let (stream, rx) = TutorStream::new(128);
        let entry = SessionEntry {
            id: id.clone(),
            capability: capability.to_string(),
            kb,
            llm,
            stream,
        };
        self.sessions.lock().unwrap().insert(id.clone(), entry);
        self.receivers.lock().unwrap().insert(id.clone(), rx);
        id
    }

    /// Called by the WS handler to get the event receiver for forwarding to the client.
    pub fn take_rx(&self, id: &str) -> Option<tokio::sync::mpsc::Receiver<StreamEvent>> {
        self.receivers.lock().unwrap().remove(id)
    }

    pub fn get(&self, id: &str) -> Option<SessionEntry> {
        self.sessions.lock().unwrap().get(id).cloned()
    }
}

impl Default for SessionPool {
    fn default() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            receivers: Mutex::new(HashMap::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_pool_creates_and_retrieves() {
        let pool = SessionPool::new();
        let id = pool.create("chat", None, None);
        let entry = pool.get(&id);
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.capability, "chat");
        assert!(pool.take_rx(&id).is_some());
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let pool = SessionPool::new();
        assert!(pool.get("nonexistent-id").is_none());
    }
}
