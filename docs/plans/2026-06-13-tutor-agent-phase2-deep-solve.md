# Tutor Agent Phase 2: Deep Solve (SolveOrchestrator + REPLAN)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the four-phase Deep Solve pipeline (Pre-retrieve → Plan → Solve → Synthesize) with a REPLAN back-edge driven by `BeforeToolCallHook` interception.

**Architecture:** `SolveOrchestrator` owns a `SolveContext` (shared across phases), and runs four independent `AgentHarness` sessions sequentially. `ReplanHook` implements `BeforeToolCallHook` and intercepts the `replan()` tool call by writing the reason into `SolveContext` and returning `BeforeToolCallDecision::Deny(ToolResult)`. `PhaseManager` implements `PrepareNextTurnHook` to restrict the active tool set within a Solve step. `SolveOrchestrator.run()` loops Plan → Solve until no replan is requested or `max_replans` is reached.

**Tech Stack:** `llm-harness` hooks (`BeforeToolCallHook`, `PrepareNextTurnHook`, `ShouldStopHook`), `std::sync::Mutex` for shared state, `BoxFuture` for hook return types, `serde_json` for plan JSON parsing.

---

## File Map

| File | Responsibility |
|------|---------------|
| `crates/tutor-agent/src/solve_context.rs` | `SolveContext`, `Plan`, `PlanStep`, `StepResult` data types |
| `crates/tutor-agent/src/replan_hook.rs` | `ReplanHook` — `BeforeToolCallHook` that intercepts `replan()` |
| `crates/tutor-agent/src/phase_manager.rs` | `PhaseManager` — `PrepareNextTurnHook` that sets `active_tools` |
| `crates/tutor-agent/src/solve_orchestrator.rs` | `SolveOrchestrator` — drives four harness sessions |
| `crates/tutor-agent/src/replan_tool.rs` | `ReplanTool` — `Tool` impl that the LLM can call |

Update `crates/tutor-agent/src/lib.rs` and `src/capability.rs` to wire DeepSolve.

---

### Task 1: SolveContext data types

**Files:**
- Create: `crates/tutor-agent/src/solve_context.rs`

- [ ] **Step 1: Write test**

```rust
// At the bottom of crates/tutor-agent/src/solve_context.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_starts_empty() {
        let ctx = SolveContext::new("What is e?");
        assert_eq!(ctx.question, "What is e?");
        assert!(ctx.plan.is_none());
        assert!(ctx.replan_reason.is_none());
        assert_eq!(ctx.replan_count, 0);
        assert_eq!(ctx.max_replans, 2);
    }

    #[test]
    fn reset_for_replan_clears_plan_and_steps() {
        let mut ctx = SolveContext::new("q");
        ctx.plan = Some(Plan {
            analysis: "a".into(),
            steps: vec![PlanStep { id: "1".into(), goal: "g".into() }],
        });
        ctx.step_results.push(StepResult { step_id: "1".into(), finish_text: "done".into() });
        ctx.replan_reason = Some("better approach".into());
        ctx.reset_for_replan();
        assert!(ctx.plan.is_none());
        assert!(ctx.step_results.is_empty());
        assert!(ctx.replan_reason.is_none());
        assert_eq!(ctx.replan_count, 1);
    }
}
```

- [ ] **Step 2: Run to verify fail**

```bash
cd /Users/hhl/Documents/projs/tutor_agent
cargo test -p tutor-agent solve_context -- --nocapture 2>&1
```

- [ ] **Step 3: Implement SolveContext**

```rust
// crates/tutor-agent/src/solve_context.rs
use serde::{Deserialize, Serialize};

/// Shared state across all four Deep Solve phases.
pub struct SolveContext {
    pub question: String,
    pub kb_summary: Option<String>,
    pub plan: Option<Plan>,
    pub step_results: Vec<StepResult>,
    pub replan_count: usize,
    pub replan_reason: Option<String>,
    pub max_replans: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Plan {
    pub analysis: String,
    pub steps: Vec<PlanStep>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PlanStep {
    pub id: String,
    pub goal: String,
}

#[derive(Debug, Clone)]
pub struct StepResult {
    pub step_id: String,
    pub finish_text: String,
}

impl SolveContext {
    pub fn new(question: impl Into<String>) -> Self {
        Self {
            question: question.into(),
            kb_summary: None,
            plan: None,
            step_results: Vec::new(),
            replan_count: 0,
            replan_reason: None,
            max_replans: 2,
        }
    }

    /// Prepare for a new Plan attempt after a REPLAN signal.
    pub fn reset_for_replan(&mut self) {
        self.plan = None;
        self.step_results.clear();
        self.replan_count += 1;
        self.replan_reason = None;
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p tutor-agent solve_context -- --nocapture 2>&1
```

Expected: both tests pass.

- [ ] **Step 5: Add module to lib.rs and commit**

In `crates/tutor-agent/src/lib.rs`, add `pub mod solve_context;`.

```bash
cargo fmt && cargo clippy -p tutor-agent --all-targets
git -C /Users/hhl/Documents/projs/tutor_agent add crates/tutor-agent/src/solve_context.rs crates/tutor-agent/src/lib.rs
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(agent): SolveContext data types for Deep Solve phases"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 2: ReplanTool

**Files:**
- Create: `crates/tutor-agent/src/replan_tool.rs`

The `ReplanTool` is a Tool stub that the LLM can call. The actual effect is handled by `ReplanHook` which intercepts and denies this call — the tool's `execute()` body is never reached. It exists only to expose the tool definition to the LLM's tool schema.

- [ ] **Step 1: Write test**

```rust
// At the bottom of crates/tutor-agent/src/replan_tool.rs
#[cfg(test)]
mod tests {
    use super::*;
    use llm_harness_types::Tool;

    #[test]
    fn replan_tool_has_correct_name_and_schema() {
        let t = ReplanTool;
        assert_eq!(t.name(), "replan");
        let schema = t.parameters_schema();
        assert!(schema["properties"]["reason"].is_object());
        assert_eq!(schema["required"][0], "reason");
    }
}
```

- [ ] **Step 2: Implement ReplanTool**

```rust
// crates/tutor-agent/src/replan_tool.rs
use futures::future::BoxFuture;
use llm_harness_types::{ContentBlock, Tool, ToolContext, ToolError, ToolResult};
use serde_json::json;

static SCHEMA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();

/// Signals that the current plan should be abandoned and a new plan created.
/// This tool's execute() is never called — ReplanHook intercepts and denies it.
pub struct ReplanTool;

impl Tool for ReplanTool {
    fn name(&self) -> &str {
        "replan"
    }

    fn description(&self) -> &str {
        "Signal that the current plan is insufficient and request a new plan. \
         Provide a specific reason explaining what's wrong and what approach to try instead."
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        SCHEMA.get_or_init(|| {
            json!({
                "type": "object",
                "properties": {
                    "reason": {
                        "type": "string",
                        "description": "Why the current plan failed and what alternative to try"
                    }
                },
                "required": ["reason"]
            })
        })
    }

    fn execute<'a>(
        &'a self,
        _args: serde_json::Value,
        _ctx: &'a ToolContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        // ReplanHook denies this tool call before it reaches execute().
        Box::pin(async {
            Ok(ToolResult {
                content: vec![ContentBlock::Text {
                    text: "replan acknowledged".into(),
                }],
                details: serde_json::Value::Null,
                terminate: false,
            })
        })
    }
}
```

- [ ] **Step 3: Run test and commit**

```bash
cargo test -p tutor-agent replan_tool -- --nocapture 2>&1
cargo fmt && cargo clippy -p tutor-agent --all-targets
git -C /Users/hhl/Documents/projs/tutor_agent add crates/tutor-agent/src/replan_tool.rs crates/tutor-agent/src/lib.rs
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(agent): ReplanTool stub for LLM tool schema"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 3: ReplanHook

**Files:**
- Create: `crates/tutor-agent/src/replan_hook.rs`

`ReplanHook` implements `BeforeToolCallHook`. When it sees `tool_name == "replan"`, it writes `ctx.args["reason"]` into `SolveContext.replan_reason` and returns `BeforeToolCallDecision::Deny(ToolResult)`. All other tools pass through with `Allow`.

- [ ] **Step 1: Write tests**

```rust
// At the bottom of crates/tutor-agent/src/replan_hook.rs
#[cfg(test)]
mod tests {
    use super::*;
    use llm_harness_types::{BeforeToolCallCtx, ContentBlock};
    use std::sync::{Arc, Mutex};

    fn make_ctx<'a>(
        tool_name: &'a str,
        args: &'a serde_json::Value,
    ) -> BeforeToolCallCtx<'a> {
        BeforeToolCallCtx {
            tool_use_id: "test-id",
            tool_name,
            args,
        }
    }

    #[tokio::test]
    async fn non_replan_tool_is_allowed() {
        let ctx = Arc::new(Mutex::new(SolveContext::new("q")));
        let hook = ReplanHook::new(ctx.clone());
        let args = serde_json::json!({ "query": "something" });
        let decision = hook.on_call(make_ctx("rag_search", &args)).await;
        assert!(matches!(decision, BeforeToolCallDecision::Allow));
        assert!(ctx.lock().unwrap().replan_reason.is_none());
    }

    #[tokio::test]
    async fn replan_tool_is_denied_and_reason_written() {
        let ctx = Arc::new(Mutex::new(SolveContext::new("q")));
        let hook = ReplanHook::new(ctx.clone());
        let args = serde_json::json!({ "reason": "symbolic math needed" });
        let decision = hook.on_call(make_ctx("replan", &args)).await;
        match decision {
            BeforeToolCallDecision::Deny(result) => {
                match &result.content[0] {
                    ContentBlock::Text { text } => assert!(text.contains("replan")),
                    _ => panic!("expected text"),
                }
            }
            _ => panic!("expected Deny, got {decision:?}"),
        }
        assert_eq!(
            ctx.lock().unwrap().replan_reason.as_deref(),
            Some("symbolic math needed")
        );
    }

    #[tokio::test]
    async fn empty_reason_defaults_to_empty_string() {
        let ctx = Arc::new(Mutex::new(SolveContext::new("q")));
        let hook = ReplanHook::new(ctx.clone());
        let args = serde_json::json!({});
        let _ = hook.on_call(make_ctx("replan", &args)).await;
        assert_eq!(ctx.lock().unwrap().replan_reason.as_deref(), Some(""));
    }
}
```

- [ ] **Step 2: Run to verify fail**

```bash
cargo test -p tutor-agent replan_hook -- --nocapture 2>&1
```

- [ ] **Step 3: Implement ReplanHook**

```rust
// crates/tutor-agent/src/replan_hook.rs
use std::sync::{Arc, Mutex};

use futures::future::BoxFuture;
use llm_harness_types::{
    BeforeToolCallCtx, BeforeToolCallDecision, BeforeToolCallHook, ContentBlock, ToolResult,
};
use serde_json::json;

use crate::solve_context::SolveContext;

/// Intercepts `replan()` tool calls, records the reason in SolveContext,
/// and returns Deny so the harness never executes the tool body.
pub struct ReplanHook {
    context: Arc<Mutex<SolveContext>>,
}

impl ReplanHook {
    pub fn new(context: Arc<Mutex<SolveContext>>) -> Self {
        Self { context }
    }
}

impl BeforeToolCallHook for ReplanHook {
    fn on_call<'a>(
        &'a self,
        ctx: BeforeToolCallCtx<'a>,
    ) -> BoxFuture<'a, BeforeToolCallDecision> {
        Box::pin(async move {
            if ctx.tool_name != "replan" {
                return BeforeToolCallDecision::Allow;
            }
            let reason = ctx.args["reason"]
                .as_str()
                .unwrap_or("")
                .to_string();
            // Lock → write → immediately drop
            self.context.lock().unwrap().replan_reason = Some(reason.clone());
            BeforeToolCallDecision::Deny(ToolResult {
                content: vec![ContentBlock::Text {
                    text: format!("replan triggered: {reason}"),
                }],
                details: json!({ "replan_reason": reason }),
                terminate: false,
            })
        })
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p tutor-agent replan_hook -- --nocapture 2>&1
```

Expected: all three tests pass.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -p tutor-agent --all-targets
git -C /Users/hhl/Documents/projs/tutor_agent add crates/tutor-agent/src/replan_hook.rs crates/tutor-agent/src/lib.rs
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(agent): ReplanHook intercepts replan() via BeforeToolCallHook"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 4: PhaseManager

**Files:**
- Create: `crates/tutor-agent/src/phase_manager.rs`

`PhaseManager` implements `PrepareNextTurnHook`. It returns a `NextTurnDirective` with `active_tools` set to the whitelist passed at construction. All other fields are `None` (use harness defaults).

- [ ] **Step 1: Write tests**

```rust
// At the bottom of crates/tutor-agent/src/phase_manager.rs
#[cfg(test)]
mod tests {
    use super::*;
    use llm_harness_types::{PrepareNextTurnCtx, PrepareNextTurnHook};
    use std::collections::HashSet;

    // Minimal stub that satisfies PrepareNextTurnCtx's lifetime
    // (the ctx fields are not used by PhaseManager)
    fn make_ctx() -> serde_json::Value {
        serde_json::json!({})
    }

    #[tokio::test]
    async fn returns_correct_active_tools() {
        let manager = PhaseManager::new(vec!["rag_search".into(), "replan".into()]);
        // PrepareNextTurnCtx requires a lifetime-bound reference; use a dummy value
        // The actual ctx fields are unused by PhaseManager
        let directive = {
            // We can't easily construct a real PrepareNextTurnCtx without a running harness.
            // Test the logic directly by calling the internal helper.
            manager.active_tools_set()
        };
        assert!(directive.contains("rag_search"));
        assert!(directive.contains("replan"));
        assert!(!directive.contains("save_note"));
    }
}
```

- [ ] **Step 2: Implement PhaseManager**

```rust
// crates/tutor-agent/src/phase_manager.rs
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
                tools: None,
                active_tools: Some(tools),
            })
        })
    }
}
```

- [ ] **Step 3: Run test**

```bash
cargo test -p tutor-agent phase_manager -- --nocapture 2>&1
```

Expected: `ok`

- [ ] **Step 4: Commit**

```bash
cargo fmt && cargo clippy -p tutor-agent --all-targets
git -C /Users/hhl/Documents/projs/tutor_agent add crates/tutor-agent/src/phase_manager.rs crates/tutor-agent/src/lib.rs
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(agent): PhaseManager controls active_tools via PrepareNextTurnHook"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 5: SolveOrchestrator skeleton

**Files:**
- Create: `crates/tutor-agent/src/solve_orchestrator.rs`

- [ ] **Step 1: Write tests for orchestration loop logic**

```rust
// At the bottom of crates/tutor-agent/src/solve_orchestrator.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::solve_context::SolveContext;

    #[test]
    fn should_replan_when_reason_set_and_under_limit() {
        let mut ctx = SolveContext::new("q");
        ctx.replan_reason = Some("try another way".into());
        ctx.replan_count = 0;
        assert!(should_replan(&ctx));
    }

    #[test]
    fn should_not_replan_when_limit_reached() {
        let mut ctx = SolveContext::new("q");
        ctx.replan_reason = Some("try again".into());
        ctx.replan_count = 2; // equals max_replans
        assert!(!should_replan(&ctx));
    }

    #[test]
    fn should_not_replan_when_no_reason() {
        let ctx = SolveContext::new("q");
        assert!(!should_replan(&ctx));
    }
}
```

- [ ] **Step 2: Run to verify fail**

```bash
cargo test -p tutor-agent solve_orchestrator -- --nocapture 2>&1
```

- [ ] **Step 3: Implement SolveOrchestrator skeleton**

```rust
// crates/tutor-agent/src/solve_orchestrator.rs
use std::sync::{Arc, Mutex};

use llm_harness_types::ExecutionEnv;

use crate::error::{Result, TutorError};
use crate::solve_context::{Plan, SolveContext, StepResult};

/// Drives the four-phase Deep Solve pipeline.
pub struct SolveOrchestrator {
    context: Arc<Mutex<SolveContext>>,
    env: Arc<dyn ExecutionEnv>,
    model: String,
    anthropic_api_key: String,
}

impl SolveOrchestrator {
    pub fn new(
        question: impl Into<String>,
        env: Arc<dyn ExecutionEnv>,
        model: impl Into<String>,
        anthropic_api_key: impl Into<String>,
    ) -> Self {
        Self {
            context: Arc::new(Mutex::new(SolveContext::new(question))),
            env,
            model: model.into(),
            anthropic_api_key: anthropic_api_key.into(),
        }
    }

    /// Run the full pipeline: [Pre-retrieve] → Plan → (Solve → [REPLAN])* → Synthesize.
    pub async fn run(&mut self, kb: Option<&str>) -> Result<String> {
        if let Some(kb_text) = kb {
            self.run_pre_retrieve(kb_text).await?;
        }

        loop {
            self.run_plan().await?;
            self.run_solve_steps().await?;

            if !should_replan(&self.context.lock().unwrap()) {
                self.context.lock().unwrap().replan_reason = None;
                break;
            }
            self.context.lock().unwrap().reset_for_replan();
        }

        self.run_synthesize().await
    }

    async fn run_pre_retrieve(&mut self, kb: &str) -> Result<()> {
        // Phase 1 stub: set kb_summary to the kb parameter
        self.context.lock().unwrap().kb_summary = Some(format!("[KB summary for: {kb}]"));
        Ok(())
    }

    async fn run_plan(&mut self) -> Result<()> {
        // TODO Phase 2 Task 6: real harness call to generate JSON plan
        // Stub: create a single-step plan
        let question = self.context.lock().unwrap().question.clone();
        self.context.lock().unwrap().plan = Some(Plan {
            analysis: format!("Analyze: {question}"),
            steps: vec![crate::solve_context::PlanStep {
                id: "step-1".into(),
                goal: format!("Solve: {question}"),
            }],
        });
        Ok(())
    }

    async fn run_solve_steps(&mut self) -> Result<()> {
        // TODO Phase 2 Task 7: real harness per step
        let steps = self
            .context
            .lock().unwrap()
            .plan
            .as_ref()
            .ok_or_else(|| TutorError::Internal("no plan".into()))?
            .steps
            .clone();

        for step in steps {
            // Stub: append a result without calling the LLM
            self.context.lock().unwrap().step_results.push(StepResult {
                step_id: step.id.clone(),
                finish_text: format!("[stub result for {}]", step.goal),
            });
        }
        Ok(())
    }

    async fn run_synthesize(&mut self) -> Result<String> {
        // TODO Phase 2 Task 8: real harness call to synthesize
        let summary = self
            .context
            .lock().unwrap()
            .step_results
            .iter()
            .map(|r| r.finish_text.clone())
            .collect::<Vec<_>>()
            .join("\n");
        Ok(format!("Synthesis:\n{summary}"))
    }
}

/// True if a replan should be triggered: reason is set AND under the limit.
pub fn should_replan(ctx: &SolveContext) -> bool {
    ctx.replan_reason.is_some() && ctx.replan_count < ctx.max_replans
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p tutor-agent solve_orchestrator -- --nocapture 2>&1
```

Expected: all three tests pass.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -p tutor-agent --all-targets
git -C /Users/hhl/Documents/projs/tutor_agent add crates/tutor-agent/src/solve_orchestrator.rs crates/tutor-agent/src/lib.rs
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(agent): SolveOrchestrator skeleton with REPLAN loop logic"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 6: run_plan() with real harness

Replace the stub `run_plan()` with a real `AgentHarness` call that outputs structured JSON.

**Files:**
- Modify: `crates/tutor-agent/src/solve_orchestrator.rs`

- [ ] **Step 1: Write test for JSON plan parsing**

```rust
// In crates/tutor-agent/src/solve_orchestrator.rs tests
#[test]
fn parse_plan_from_json() {
    let raw = r#"{"analysis":"use calculus","steps":[{"id":"s1","goal":"integrate"},{"id":"s2","goal":"simplify"}]}"#;
    let plan: crate::solve_context::Plan = serde_json::from_str(raw).unwrap();
    assert_eq!(plan.steps.len(), 2);
    assert_eq!(plan.steps[0].id, "s1");
}
```

- [ ] **Step 2: Replace stub run_plan() with real harness**

```rust
async fn run_plan(&mut self) -> Result<()> {
    use std::sync::Arc;
    use llm_adapter::anthropic::AnthropicProvider;
    use llm_harness::{AgentHarness, AgentHarnessEvent, AgentHarnessOptions};
    use llm_harness_runtime_auth::EnvAuthHook;
    use llm_harness_types::{AgentEvent, ContentBlock};

    let (question, kb_summary, replan_reason, prev_plan) = {
        let ctx = self.context.lock().unwrap();
        (ctx.question.clone(),
         ctx.kb_summary.clone(),
         ctx.replan_reason.clone(),
         ctx.plan.clone())
    };

    let prompt = if let Some(reason) = &replan_reason {
        // REPLAN path: include previous plan + reason
        let prev = prev_plan.as_ref()
            .map(|p| serde_json::to_string_pretty(p).unwrap_or_default())
            .unwrap_or_default();
        format!(
            "Question: {}\nKB summary: {}\n\nPrevious plan (now abandoned):\n{prev}\n\
             Replan reason: {reason}\n\n\
             Create a NEW step-by-step plan in JSON: \
             {{\"analysis\":\"...\",\"steps\":[{{\"id\":\"s1\",\"goal\":\"...\"}},...]}}\n\
             Output ONLY the JSON, no prose.",
            question,
            kb_summary.as_deref().unwrap_or("none")
        )
    } else {
        format!(
            "Question: {}\nKB summary: {}\n\n\
             Create a step-by-step plan in JSON: \
             {{\"analysis\":\"...\",\"steps\":[{{\"id\":\"s1\",\"goal\":\"...\"}},...]}}\n\
             Output ONLY the JSON, no prose.",
            question,
            kb_summary.as_deref().unwrap_or("none")
        )
    };

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| TutorError::Internal("ANTHROPIC_API_KEY not set".into()))?;
    let client = Arc::new(AnthropicProvider::builder(api_key).build());

    let opts = AgentHarnessOptions {
        model: self.model.clone(),
        tools: vec![],  // Plan phase has no tools
        system_prompt: Some(
            "You are a math tutor planning a structured solution. \
             Respond only with the requested JSON."
                .into(),
        ),
        auth: Some(Arc::new(EnvAuthHook::for_provider("anthropic"))),
        ..AgentHarnessOptions::new(self.model.clone())
    };

    let harness = AgentHarness::new_in_memory(client, self.env.clone(), opts).await;
    let mut rx = harness.subscribe();

    harness.prompt(prompt).await?;

    // Collect last assistant message text
    let mut raw = String::new();
    while let Ok(event) = rx.recv().await {
        match event.as_ref() {
            AgentHarnessEvent::Agent(AgentEvent::MessageEnd { message }) => {
                for block in &message.content {
                    if let ContentBlock::Text { text } = block {
                        raw = text.clone();
                    }
                }
            }
            AgentHarnessEvent::Settled | AgentHarnessEvent::Aborted => break,
            _ => {}
        }
    }

    if raw.is_empty() {
        return Err(TutorError::Internal("no plan output".into()));
    }

    // Extract JSON even if the LLM added surrounding prose
    let json_start = raw.find('{').unwrap_or(0);
    let json_end = raw.rfind('}').map(|i| i + 1).unwrap_or(raw.len());
    let json_str = &raw[json_start..json_end];

    let plan: Plan = serde_json::from_str(json_str)
        .map_err(|e| TutorError::Internal(format!("plan parse error: {e}\nraw: {raw}")))?;

    self.context.lock().unwrap().plan = Some(plan);
    Ok(())
}
```

- [ ] **Step 3: Commit**

```bash
cargo fmt && cargo clippy -p tutor-agent --all-targets
git -C /Users/hhl/Documents/projs/tutor_agent add crates/tutor-agent/
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(agent): run_plan() with real harness and JSON plan parsing"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 7: run_solve_steps() with ReplanHook

Replace the stub `run_solve_steps()` with real per-step harness calls that wire `ReplanHook` and `PhaseManager`.

**Files:**
- Modify: `crates/tutor-agent/src/solve_orchestrator.rs`

- [ ] **Step 1: Write integration test for REPLAN detection**

```rust
// In solve_orchestrator tests
#[tokio::test]
async fn replan_written_to_context_is_detected() {
    use crate::solve_context::SolveContext;
    let mut ctx = SolveContext::new("integral question");
    ctx.plan = Some(crate::solve_context::Plan {
        analysis: "a".into(),
        steps: vec![crate::solve_context::PlanStep { id: "s1".into(), goal: "integrate".into() }],
    });
    // Simulate what ReplanHook does
    ctx.replan_reason = Some("use sympy".into());
    assert!(should_replan(&ctx));
    ctx.reset_for_replan();
    assert_eq!(ctx.replan_count, 1);
    assert!(ctx.plan.is_none());
}
```

- [ ] **Step 2: Replace stub run_solve_steps() with real harness per step**

```rust
async fn run_solve_steps(&mut self) -> Result<()> {
    use std::sync::Arc;
    use llm_adapter::anthropic::AnthropicProvider;
    use llm_harness::{AgentHarness, AgentHarnessEvent, AgentHarnessOptions, HarnessHooks};
    use llm_harness_runtime_auth::EnvAuthHook;
    use llm_harness_types::{AgentEvent, ContentBlock};
    use tutor_tools::{CodeExecTool, RagSearchTool, WebSearchTool};

    use crate::phase_manager::PhaseManager;
    use crate::replan_hook::ReplanHook;
    use crate::replan_tool::ReplanTool;

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| TutorError::Internal("ANTHROPIC_API_KEY not set".into()))?;

    let steps = self
        .context
        .lock().unwrap()
        .plan
        .as_ref()
        .ok_or_else(|| TutorError::Internal("no plan".into()))?
        .steps
        .clone();

    // ReplanHook shares the orchestrator's context directly via Arc.
    let replan_hook = Arc::new(ReplanHook::new(self.context.clone()));

    for step in &steps {

        let solve_tools: Vec<Arc<dyn llm_harness_types::Tool>> = vec![
            Arc::new(RagSearchTool::new()),
            Arc::new(WebSearchTool::new()),
            Arc::new(CodeExecTool::new()),
            Arc::new(ReplanTool),
        ];

        let phase_mgr = Arc::new(PhaseManager::new(vec![
            "rag_search".into(),
            "web_search".into(),
            "code_exec".into(),
            "replan".into(),
        ]));

        let opts = AgentHarnessOptions {
            model: self.model.clone(),
            tools: solve_tools,
            system_prompt: Some(format!(
                "You are solving step {id}: {goal}\n\
                 Use rag_search and web_search for information, code_exec to run code.\n\
                 If the current plan is fundamentally wrong, call replan(reason) — \
                 this aborts the step and triggers a new plan.\n\
                 When done, write FINISH: followed by your conclusion for this step.",
                id = step.id,
                goal = step.goal,
            )),
            auth: Some(Arc::new(EnvAuthHook::for_provider("anthropic"))),
            hooks: HarnessHooks {
                before_tool_call: Some(replan_hook),
                prepare_next_turn: Some(phase_mgr),
                ..HarnessHooks::none()
            },
            ..AgentHarnessOptions::new(self.model.clone())
        };

        let client = Arc::new(AnthropicProvider::builder(api_key.clone()).build());
        let harness = AgentHarness::new_in_memory(client, self.env.clone(), opts).await;
        let mut rx = harness.subscribe();

        harness.prompt(format!("Solve step {}: {}", step.id, step.goal)).await?;

        // Collect last assistant text
        let mut raw = String::new();
        while let Ok(event) = rx.recv().await {
            match event.as_ref() {
                AgentHarnessEvent::Agent(AgentEvent::MessageEnd { message }) => {
                    for block in &message.content {
                        if let ContentBlock::Text { text } = block {
                            raw = text.clone();
                        }
                    }
                }
                AgentHarnessEvent::Settled | AgentHarnessEvent::Aborted => break,
                _ => {}
            }
        }

        // Check if replan was triggered (ReplanHook wrote to shared context)
        let step_reason = self.context.lock().unwrap().replan_reason.clone();
        if let Some(reason) = step_reason {
            // Return early — outer loop detects should_replan() and calls reset_for_replan()
            return Ok(());
        }

        let finish_text = raw
            .lines()
            .skip_while(|l| !l.starts_with("FINISH:"))
            .collect::<Vec<_>>()
            .join("\n");

        self.context.lock().unwrap().step_results.push(crate::solve_context::StepResult {
            step_id: step.id.clone(),
            finish_text: if finish_text.is_empty() { raw } else { finish_text },
        });
    }
    Ok(())
}
```

- [ ] **Step 4: Run integration test**

```bash
cargo test -p tutor-agent -- --nocapture 2>&1
```

Expected: all unit tests pass, including the new REPLAN detection test.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -p tutor-agent --all-targets
git -C /Users/hhl/Documents/projs/tutor_agent add crates/tutor-agent/
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(agent): run_solve_steps() with ReplanHook and PhaseManager"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 8: run_synthesize() + TaskVerifier

**Files:**
- Modify: `crates/tutor-agent/src/solve_orchestrator.rs`

- [ ] **Step 1: Write test for synthesize output format**

```rust
#[test]
fn step_results_format_for_synthesis() {
    let results = vec![
        crate::solve_context::StepResult {
            step_id: "s1".into(),
            finish_text: "The integral is 8/3.".into(),
        },
        crate::solve_context::StepResult {
            step_id: "s2".into(),
            finish_text: "Simplified: 2.67.".into(),
        },
    ];
    let formatted = format_step_results(&results);
    assert!(formatted.contains("s1"));
    assert!(formatted.contains("8/3"));
}
```

- [ ] **Step 2: Implement run_synthesize() and format_step_results()**

```rust
fn format_step_results(results: &[crate::solve_context::StepResult]) -> String {
    results
        .iter()
        .map(|r| format!("Step {}: {}", r.step_id, r.finish_text))
        .collect::<Vec<_>>()
        .join("\n\n")
}

async fn run_synthesize(&mut self) -> Result<String> {
    use std::sync::Arc;
    use llm_adapter::anthropic::AnthropicProvider;
    use llm_harness::{AgentHarness, AgentHarnessEvent, AgentHarnessOptions};
    use llm_harness_runtime_auth::EnvAuthHook;
    use llm_harness_types::{AgentEvent, ContentBlock};

    let (question, steps_summary) = {
        let ctx = self.context.lock().unwrap();
        (ctx.question.clone(), format_step_results(&ctx.step_results))
    };
    let prompt = format!(
        "Question: {}\n\nStep-by-step work:\n{steps_summary}\n\n\
         Synthesize a clear, complete final answer for the student. \
         Start with the direct answer, then provide explanation.",
        question
    );

    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| TutorError::Internal("ANTHROPIC_API_KEY not set".into()))?;
    let client = Arc::new(AnthropicProvider::builder(api_key).build());

    let opts = AgentHarnessOptions {
        model: self.model.clone(),
        tools: vec![],
        system_prompt: Some(
            "You are a math tutor writing a final answer synthesis. \
             Be clear, structured, and educational."
                .into(),
        ),
        auth: Some(Arc::new(EnvAuthHook::for_provider("anthropic"))),
        ..AgentHarnessOptions::new(self.model.clone())
    };

    let harness = AgentHarness::new_in_memory(client, self.env.clone(), opts).await;
    let mut rx = harness.subscribe();

    harness.prompt(prompt).await?;

    let mut last_text = String::new();
    while let Ok(event) = rx.recv().await {
        match event.as_ref() {
            AgentHarnessEvent::Agent(AgentEvent::MessageEnd { message }) => {
                for block in &message.content {
                    if let ContentBlock::Text { text } = block {
                        last_text = text.clone();
                    }
                }
            }
            AgentHarnessEvent::Settled | AgentHarnessEvent::Aborted => break,
            _ => {}
        }
    }

    Ok(if last_text.is_empty() {
        "No synthesis generated.".into()
    } else {
        last_text
    })
}
```

- [ ] **Step 3: Wire DeepSolve into CapabilityRouter**

In `capability.rs`, update the `DeepSolve` arm:

```rust
Capability::DeepSolve => {
    let mut orchestrator = crate::solve_orchestrator::SolveOrchestrator::new(
        question,
        self.env.clone(),
        &self.model,
        &self.anthropic_api_key,
    );
    orchestrator.run(None).await
}
```

- [ ] **Step 4: Commit**

```bash
cargo fmt && cargo clippy -p tutor-agent --all-targets
git -C /Users/hhl/Documents/projs/tutor_agent add crates/tutor-agent/
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(agent): run_synthesize() and wire DeepSolve into CapabilityRouter"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 9: End-to-end integration test

- [ ] **Step 1: Write integration test**

Create `crates/tutor-agent/tests/deep_solve_integration.rs`:

```rust
// crates/tutor-agent/tests/deep_solve_integration.rs
// Run with: cargo test --test deep_solve_integration -- --ignored
// Requires ANTHROPIC_API_KEY to be set.

use std::sync::Arc;
use llm_harness_runtime_sandbox_os::OsEnv;
use tutor_agent::{Capability, CapabilityRouter};

#[tokio::test]
#[ignore = "requires ANTHROPIC_API_KEY and network"]
async fn deep_solve_end_to_end() {
    let api_key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY required");
    let tmp = tempfile::tempdir().unwrap();
    let env = Arc::new(OsEnv::new(tmp.path()));
    let router = CapabilityRouter::new(env, "claude-haiku-4-5-20251001", api_key);

    let result = router.run(Capability::DeepSolve, "What is the integral of x^2 from 0 to 2?").await;
    assert!(result.is_ok(), "error: {:?}", result.err());
    let answer = result.unwrap();
    assert!(!answer.is_empty());
    // The answer should mention 8/3 or approximately 2.67
    println!("Deep Solve answer:\n{answer}");
}

#[tokio::test]
#[ignore = "requires ANTHROPIC_API_KEY and network"]  
async fn deep_solve_replan_triggers_and_recovers() {
    // This test verifies that when the LLM calls replan(), the orchestrator
    // loops back to Plan phase and eventually produces a result.
    // Hard to force deterministically — run as a smoke test.
    let api_key = std::env::var("ANTHROPIC_API_KEY").expect("ANTHROPIC_API_KEY required");
    let tmp = tempfile::tempdir().unwrap();
    let env = Arc::new(OsEnv::new(tmp.path()));
    let router = CapabilityRouter::new(env, "claude-haiku-4-5-20251001", api_key);

    let result = router.run(
        Capability::DeepSolve,
        "Solve x^3 - 6x^2 + 11x - 6 = 0 and verify using code_exec"
    ).await;
    assert!(result.is_ok(), "error: {:?}", result.err());
    println!("Answer:\n{}", result.unwrap());
}
```

- [ ] **Step 2: Add tempfile dev-dep to tutor-agent**

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Run all non-ignored tests**

```bash
cd /Users/hhl/Documents/projs/tutor_agent
cargo test --workspace 2>&1
```

Expected: all tests pass (integration tests are `#[ignore]`, so they're skipped).

- [ ] **Step 4: Commit Phase 2**

```bash
cargo fmt && cargo clippy --workspace --all-targets -- -D warnings
git -C /Users/hhl/Documents/projs/tutor_agent add crates/tutor-agent/
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "test(agent): integration tests for Deep Solve pipeline"
git -C /Users/hhl/Documents/projs/tutor_agent push
git -C /Users/hhl/Documents/projs/tutor_agent tag phase2-complete
git -C /Users/hhl/Documents/projs/tutor_agent push --tags
```

---

## Phase 2 Success Criteria

- `cargo test --workspace` passes (unit tests only)
- `ReplanHook` test: `replan()` call is denied and `replan_reason` is written to context
- `PhaseManager` test: `active_tools_set()` returns the correct tool names
- `SolveOrchestrator` test: `should_replan()` logic is correct
- `SolveContext` test: `reset_for_replan()` increments counter and clears state
- Integration tests exist and are `#[ignore]`-tagged for manual verification with real API key
