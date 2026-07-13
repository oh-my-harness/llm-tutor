use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use llm_adapter::provider::Provider;
use llm_harness_agent::Session;
use llm_harness_types::{AgentMessage, ContentBlock, ExecutionEnv, Tool};
use tokio_util::sync::CancellationToken;
use tutor_rag::KnowledgeRetriever;

use crate::error::{Result, TutorError};
use crate::event_sink::SharedEventSink;
use crate::governance::GovernanceConfig;
use crate::llm_provider::LlmConfig;
use tutor_tools::WebSearchConfig;

/// Supported teaching modes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Capability {
    /// Conversational Q&A with RAG knowledge base.
    Chat,
    /// Multi-phase guided problem solving (Pre-retrieve → Plan → Solve → Synthesize).
    DeepSolve,
    /// Execute user code with explanation.
    CodeExec,
    /// Generate and answer knowledge-base quizzes in the product UI.
    Quiz,
    /// Research external/internal sources and synthesize a cited report.
    Research,
    /// Organize Notebook/Space content through search and preview-only proposals.
    Organize,
}

impl FromStr for Capability {
    type Err = TutorError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "chat" => Ok(Self::Chat),
            "deep_solve" => Ok(Self::DeepSolve),
            "code_exec" => Ok(Self::CodeExec),
            "quiz" => Ok(Self::Quiz),
            "research" => Ok(Self::Research),
            "organize" => Ok(Self::Organize),
            other => Err(TutorError::UnsupportedCapability(other.into())),
        }
    }
}

/// Entry point for all capabilities.
#[derive(Clone)]
pub struct CapabilityRouter {
    pub env: Arc<dyn ExecutionEnv>,
    pub llm: LlmConfig,
    pub governance: GovernanceConfig,
    pub event_sink: Option<SharedEventSink>,
    pub retriever: Option<Arc<dyn KnowledgeRetriever>>,
    pub associated_kb: Option<String>,
    pub web_search: Option<WebSearchConfig>,
    pub product_tools: Vec<Arc<dyn Tool>>,
    pub workflow_root: Option<PathBuf>,
    client: Option<Arc<dyn Provider>>,
}

impl CapabilityRouter {
    pub fn new(env: Arc<dyn ExecutionEnv>, llm: LlmConfig, governance: GovernanceConfig) -> Self {
        Self {
            env,
            llm,
            governance,
            event_sink: None,
            retriever: None,
            associated_kb: None,
            web_search: None,
            product_tools: vec![],
            workflow_root: None,
            client: None,
        }
    }

    /// Inject a custom LLM client; skips `LlmConfig::build_client()` and auth.
    pub fn with_client(mut self, client: Arc<dyn Provider>) -> Self {
        self.client = Some(client);
        self
    }

    /// Attach an optional trace sink for web sessions.
    pub fn with_event_sink(mut self, sink: SharedEventSink) -> Self {
        self.event_sink = Some(sink);
        self
    }

    pub fn with_retriever(mut self, retriever: Arc<dyn KnowledgeRetriever>) -> Self {
        self.retriever = Some(retriever);
        self
    }

    pub fn with_associated_kb(mut self, kb: impl Into<String>) -> Self {
        let kb = kb.into();
        if !kb.trim().is_empty() {
            self.associated_kb = Some(kb);
        }
        self
    }

    pub fn with_web_search(mut self, config: WebSearchConfig) -> Self {
        self.web_search = Some(config);
        self
    }

    pub fn with_product_tool(mut self, tool: Arc<dyn Tool>) -> Self {
        self.product_tools.push(tool);
        self
    }

    pub fn with_workflow_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.workflow_root = Some(root.into());
        self
    }

    /// Returns the injected client or builds one from `LlmConfig`.
    pub(crate) fn make_client(&self) -> Arc<dyn Provider> {
        if let Some(c) = &self.client {
            return c.clone();
        }
        self.llm.build_client()
    }

    /// Route a question to the appropriate capability.
    pub async fn run(&self, capability: Capability, question: &str) -> Result<String> {
        self.run_with_messages(capability, vec![crate::chat::user_message(question)])
            .await
    }

    /// Route an explicit message history to the appropriate capability.
    pub async fn run_with_messages(
        &self,
        capability: Capability,
        messages: Vec<AgentMessage>,
    ) -> Result<String> {
        match capability {
            Capability::Chat => crate::chat::run_chat_with_messages(self, messages).await,
            Capability::Research => crate::chat::run_research_with_messages(self, messages).await,
            Capability::Organize => crate::chat::run_organize_with_messages(self, messages).await,
            Capability::Quiz => crate::chat::run_quiz_with_messages(self, messages).await,
            Capability::DeepSolve => {
                let question = question_from_messages(&messages);
                let client = self.make_client();
                let mut orchestrator = crate::solve_orchestrator::SolveOrchestrator::new(
                    question,
                    self.env.clone(),
                    self.llm.clone(),
                    self.governance.clone(),
                )
                .with_event_sink(self.event_sink.clone())
                .with_web_search(self.web_search.clone())
                .with_workflow_root(self.workflow_root.clone())
                .with_client(client);
                orchestrator.run(None).await
            }
            Capability::CodeExec => {
                crate::code_exec::run_code_exec_with_messages(self, messages).await
            }
        }
    }

    /// Route a question using a runtime-backed session for context and persistence.
    pub async fn run_with_session(
        &self,
        capability: Capability,
        session: Session,
        question: &str,
    ) -> Result<String> {
        self.run_with_session_cancel(capability, session, question, None)
            .await
    }

    /// Route a question using a runtime-backed session and an optional abort token.
    pub async fn run_with_session_cancel(
        &self,
        capability: Capability,
        session: Session,
        question: &str,
        abort_token: Option<CancellationToken>,
    ) -> Result<String> {
        match capability {
            Capability::Chat => {
                crate::chat::run_chat_with_session_cancel(self, session, question, abort_token)
                    .await
            }
            Capability::Research => {
                crate::chat::run_research_with_session_cancel(self, session, question, abort_token)
                    .await
            }
            Capability::Organize => {
                crate::chat::run_organize_with_session_cancel(self, session, question, abort_token)
                    .await
            }
            Capability::Quiz => {
                crate::chat::run_quiz_with_session_cancel(self, session, question, abort_token)
                    .await
            }
            Capability::CodeExec => {
                crate::code_exec::run_code_exec_with_session_cancel(
                    self,
                    session,
                    question,
                    abort_token,
                )
                .await
            }
            Capability::DeepSolve => {
                let existing = session
                    .build_context()
                    .await
                    .map_err(|err| TutorError::Internal(err.to_string()))?
                    .messages;
                let mut messages = existing;
                messages.push(crate::chat::user_message(question));
                let answer = self.run_with_messages(capability, messages).await?;
                session
                    .append_message(crate::chat::user_message(question))
                    .await
                    .map_err(|err| TutorError::Internal(err.to_string()))?;
                session
                    .append_message(crate::chat::assistant_message(&answer))
                    .await
                    .map_err(|err| TutorError::Internal(err.to_string()))?;
                Ok(answer)
            }
        }
    }
}

fn question_from_messages(messages: &[AgentMessage]) -> String {
    let Some(last_user_text) = messages.iter().rev().find_map(|message| match message {
        AgentMessage::User(_) => agent_message_text(message),
        _ => None,
    }) else {
        return String::new();
    };

    if messages.len() <= 1 {
        return last_user_text;
    }

    let context = messages
        .iter()
        .take(messages.len().saturating_sub(1))
        .filter_map(|message| match message {
            AgentMessage::User(_) => {
                agent_message_text(message).map(|text| format!("User: {text}"))
            }
            AgentMessage::Assistant(_) => {
                agent_message_text(message).map(|text| format!("Assistant: {text}"))
            }
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!("Conversation context:\n{context}\n\nCurrent question:\n{last_user_text}")
}

fn agent_message_text(message: &AgentMessage) -> Option<String> {
    let content = match message {
        AgentMessage::User(message) => &message.content,
        AgentMessage::Assistant(message) => &message.content,
        _ => return None,
    };

    let text = content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    if text.trim().is_empty() {
        None
    } else {
        Some(text)
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
        assert!(matches!(
            Capability::from_str("quiz").unwrap(),
            Capability::Quiz
        ));
        assert!(matches!(
            Capability::from_str("research").unwrap(),
            Capability::Research
        ));
        assert!(matches!(
            Capability::from_str("organize").unwrap(),
            Capability::Organize
        ));
        assert!(Capability::from_str("unknown").is_err());
    }
}
