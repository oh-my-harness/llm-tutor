use std::str::FromStr;
use std::sync::Arc;

use llm_adapter::provider::Provider;
use llm_harness_types::ExecutionEnv;

use crate::error::{Result, TutorError};
use crate::governance::GovernanceConfig;
use crate::llm_provider::LlmConfig;

/// Supported teaching modes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Capability {
    /// Conversational Q&A with RAG knowledge base.
    Chat,
    /// Multi-phase guided problem solving (Pre-retrieve → Plan → Solve → Synthesize).
    DeepSolve,
    /// Execute user code with explanation.
    CodeExec,
}

impl FromStr for Capability {
    type Err = TutorError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "chat" => Ok(Self::Chat),
            "deep_solve" => Ok(Self::DeepSolve),
            "code_exec" => Ok(Self::CodeExec),
            other => Err(TutorError::UnsupportedCapability(other.into())),
        }
    }
}

/// Entry point for all capabilities.
pub struct CapabilityRouter {
    pub env: Arc<dyn ExecutionEnv>,
    pub llm: LlmConfig,
    pub governance: GovernanceConfig,
    client: Option<Arc<dyn Provider>>,
}

impl CapabilityRouter {
    pub fn new(env: Arc<dyn ExecutionEnv>, llm: LlmConfig, governance: GovernanceConfig) -> Self {
        Self {
            env,
            llm,
            governance,
            client: None,
        }
    }

    /// Inject a custom LLM client; skips `LlmConfig::build_client()` and auth.
    pub fn with_client(mut self, client: Arc<dyn Provider>) -> Self {
        self.client = Some(client);
        self
    }

    /// Returns the injected client or builds one from `LlmConfig`.
    pub(crate) fn make_client(&self) -> Arc<dyn Provider> {
        if let Some(c) = &self.client {
            return c.clone();
        }
        self.llm.build_client()
    }

    /// Returns an auth hook; `None` when a mock client is injected.
    pub(crate) fn auth_hook(&self) -> Option<Arc<dyn llm_harness_types::AuthHook>> {
        if self.client.is_some() {
            return None;
        }
        use llm_harness_types::AuthHook;
        self.llm
            .auth_hook()
            .map(|h| Arc::new(h) as Arc<dyn AuthHook>)
    }

    /// Route a question to the appropriate capability.
    pub async fn run(&self, capability: Capability, question: &str) -> Result<String> {
        match capability {
            Capability::Chat => crate::chat::run_chat(self, question).await,
            Capability::DeepSolve => {
                let client = self.make_client();
                let mut orchestrator = crate::solve_orchestrator::SolveOrchestrator::new(
                    question,
                    self.env.clone(),
                    self.llm.clone(),
                    self.governance.clone(),
                )
                .with_client(client);
                orchestrator.run(None).await
            }
            Capability::CodeExec => Err(TutorError::UnsupportedCapability(
                "CodeExec (Phase 2+)".into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_from_str() {
        assert!(matches!(
            Capability::from_str("chat").unwrap(),
            Capability::Chat
        ));
        assert!(matches!(
            Capability::from_str("deep_solve").unwrap(),
            Capability::DeepSolve
        ));
        assert!(Capability::from_str("unknown").is_err());
    }
}
