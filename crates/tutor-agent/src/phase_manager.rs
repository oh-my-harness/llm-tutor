use std::collections::HashSet;

use futures::future::BoxFuture;
use llm_harness_types::{AgentError, NextTurnDirective, PrepareNextTurnCtx, PrepareNextTurnHook};

/// Controls which tools are active within a single Solve step.
/// The outer phase transitions (Pre-retrieve → Plan → Solve → Synthesize)
/// are driven by SolveOrchestrator, not by this hook.
pub struct PhaseManager {
    allowed_tools: Vec<String>,
}

impl PhaseManager {
    pub fn new(allowed_tools: Vec<String>) -> Self {
        Self { allowed_tools }
    }

    /// Exposed for unit tests that cannot construct a real PrepareNextTurnCtx.
    pub fn active_tools_set(&self) -> HashSet<String> {
        self.allowed_tools.iter().cloned().collect()
    }
}

impl PrepareNextTurnHook for PhaseManager {
    fn prepare<'a>(
        &'a self,
        _ctx: PrepareNextTurnCtx<'a>,
    ) -> BoxFuture<'a, Result<NextTurnDirective, AgentError>> {
        let tools = self.active_tools_set();
        Box::pin(async move {
            Ok(NextTurnDirective {
                context: None,
                model: None,
                thinking_level: None,
                temperature: None,
                tools: None,
                active_tools: Some(tools),
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_correct_active_tools() {
        let manager = PhaseManager::new(vec!["rag_search".into(), "replan".into()]);
        let directive = manager.active_tools_set();
        assert!(directive.contains("rag_search"));
        assert!(directive.contains("replan"));
        assert!(!directive.contains("save_note"));
    }
}
