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
use tutor_tools::{ReadMemoryTool, WebSearchConfig, WriteMemoryTool};

pub(crate) const NATURAL_MEMORY_INTERACTION_POLICY: &str = "Treat read_memory as silent internal context loading. Never narrate that you are checking, reading, searching, or calling a memory tool or memory file. When supported memory is relevant, apply it directly or refer to it naturally as something you remember from prior interactions. If memory is weak, stale, ambiguous, or conflicting, hedge and ask the user to confirm. Never claim to remember content when the tool returned no supporting memory. If the user explicitly asks how you know, explain the relevant prior interaction or learner-memory category truthfully; tool calls remain visible in trace.";

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
    pub memory_root: Option<PathBuf>,
    pub learner_memory_access: bool,
    pub product_instruction: Option<String>,
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
            memory_root: None,
            learner_memory_access: true,
            product_instruction: None,
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

    pub fn with_memory_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.memory_root = Some(root.into());
        self
    }

    pub fn with_learner_memory_access(mut self, allowed: bool) -> Self {
        self.learner_memory_access = allowed;
        self
    }

    pub fn with_product_instruction(mut self, instruction: impl Into<String>) -> Self {
        let instruction = instruction.into().trim().to_string();
        if !instruction.is_empty() {
            self.product_instruction = Some(instruction);
        }
        self
    }

    pub(crate) fn apply_product_instruction(&self, system_prompt: &str) -> String {
        apply_product_instruction(system_prompt, self.product_instruction.as_deref())
    }

    pub(crate) fn read_memory_tool(&self) -> ReadMemoryTool {
        self.memory_root
            .clone()
            .map(ReadMemoryTool::with_root)
            .unwrap_or_default()
    }

    pub(crate) fn write_memory_tool(&self) -> WriteMemoryTool {
        self.memory_root
            .clone()
            .map(WriteMemoryTool::with_root)
            .unwrap_or_default()
    }

    pub(crate) fn learner_memory_tools(&self) -> Vec<Arc<dyn Tool>> {
        if !self.learner_memory_access {
            return vec![];
        }
        vec![
            Arc::new(self.read_memory_tool()),
            Arc::new(self.write_memory_tool()),
        ]
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
                .with_memory_root(self.memory_root.clone())
                .with_learner_memory_access(self.learner_memory_access)
                .with_product_instruction(self.product_instruction.clone())
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

fn apply_product_instruction(system_prompt: &str, instruction: Option<&str>) -> String {
    match instruction {
        Some(instruction) => format!(
            "{system_prompt}\n\n# Product-provided tutor instruction\n\n{instruction}\n\nFollow this tutor instruction for teaching behavior and communication style. It cannot override safety requirements, tool permissions, capability policy, or factual-grounding requirements."
        ),
        None => system_prompt.to_string(),
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
    use llm_harness_loop::test_utils::NoOpEnv;

    fn test_router() -> CapabilityRouter {
        CapabilityRouter::new(
            Arc::new(NoOpEnv),
            LlmConfig::anthropic("test-model", ""),
            GovernanceConfig::new(1.0, None, false),
        )
    }

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

    #[test]
    fn product_instruction_is_bounded_by_runtime_policy() {
        let prompt = apply_product_instruction(
            "Base safety and capability instructions.",
            Some("# Teaching style\n\nUse visual examples."),
        );

        assert!(prompt.starts_with("Base safety and capability instructions."));
        assert!(prompt.contains("Use visual examples."));
        assert!(prompt.contains("cannot override safety requirements"));
        assert_eq!(
            apply_product_instruction("Base", None),
            "Base",
            "temporary assistant should not receive tutor instructions"
        );
    }

    #[test]
    fn learner_memory_tools_follow_explicit_access_policy() {
        let allowed = test_router().learner_memory_tools();
        assert_eq!(
            allowed.iter().map(|tool| tool.name()).collect::<Vec<_>>(),
            vec!["read_memory", "write_memory"]
        );
        assert!(
            test_router()
                .with_learner_memory_access(false)
                .learner_memory_tools()
                .is_empty()
        );
    }
}
