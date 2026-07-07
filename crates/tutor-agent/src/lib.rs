pub mod capability;
pub mod chat;
pub mod code_exec;
pub mod deep_solve_events;
pub mod error;
pub mod event_sink;
pub mod governance;
pub mod llm_provider;
pub mod memory;
pub mod quiz;
pub mod runtime_engine;
pub mod runtime_harness;
pub mod runtime_workflow;
pub mod solve_orchestrator;
pub mod terminal_approver;

pub use capability::{Capability, CapabilityRouter};
pub use error::{Result, TutorError};
pub use llm_provider::{LlmConfig, LlmProviderKind};
