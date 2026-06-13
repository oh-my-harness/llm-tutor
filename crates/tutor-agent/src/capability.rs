use std::str::FromStr;
use std::sync::Arc;

use llm_harness_types::ExecutionEnv;

use crate::error::{Result, TutorError};
use crate::governance::GovernanceConfig;

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
    pub model: String,
    pub anthropic_api_key: String,
    pub governance: GovernanceConfig,
}

impl CapabilityRouter {
    pub fn new(
        env: Arc<dyn ExecutionEnv>,
        model: impl Into<String>,
        anthropic_api_key: impl Into<String>,
        governance: GovernanceConfig,
    ) -> Self {
        Self {
            env,
            model: model.into(),
            anthropic_api_key: anthropic_api_key.into(),
            governance,
        }
    }

    /// Route a question to the appropriate capability.
    pub async fn run(&self, capability: Capability, question: &str) -> Result<String> {
        match capability {
            Capability::Chat => crate::chat::run_chat(self, question).await,
            Capability::DeepSolve => {
                let mut orchestrator = crate::solve_orchestrator::SolveOrchestrator::new(
                    question,
                    self.env.clone(),
                    &self.model,
                    self.governance.clone(),
                );
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
