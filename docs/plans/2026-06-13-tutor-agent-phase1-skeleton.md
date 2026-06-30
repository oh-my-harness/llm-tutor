# Tutor Agent Phase 1: Workspace Skeleton + Tools + Chat Capability

> Status: superseded | Date: 2026-06-13 | Superseded on: 2026-06-30.
> This was an implementation checklist for the original runtime demo skeleton.
> It is kept for historical context only. Do not use the unchecked tasks below
> as current project status. Current product work is tracked in
> `docs/plans/2026-06-26-next-product-slice-plan.md` and the active product
> requirements spec.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stand up the `tutor_agent` Rust workspace with `tutor-tools` (rag_search, web_search, code_exec) and a `Chat` capability that runs end-to-end against a real LLM.

**Architecture:** `tutor-tools` implements the `Tool` trait from `llm-harness-types` for three domain tools. `tutor-agent` assembles an `AgentHarness` via `AgentHarness::new_in_memory`, wires tools and auth, and exposes a `CapabilityRouter` with a `run_chat()` entry point. A small CLI binary (`tutor-agent/src/main.rs`) drives the whole thing from the terminal.

**Tech Stack:** Rust 2024 edition, `llm-harness` + `llm-harness-types` (git rev `9ad7292`), `llm_adapter` (re-exported as `LlmClient` by `llm-harness-loop`; use `AnthropicProvider::builder(key).build()`), `llm-harness-runtime` (path dep), `llm-harness-runtime-auth`, `llm-harness-runtime-sandbox-os`, `tokio`, `serde_json`, `anyhow`.

---

## File Map

| File | Responsibility |
|------|---------------|
| `Cargo.toml` | workspace declaration, shared deps |
| `.cargo/config.toml` | optional local path overrides for llm-harness-core |
| `crates/tutor-tools/Cargo.toml` | tutor-tools crate deps |
| `crates/tutor-tools/src/lib.rs` | re-exports all tools |
| `crates/tutor-tools/src/rag_search.rs` | `RagSearchTool` — stub keyword search |
| `crates/tutor-tools/src/web_search.rs` | `WebSearchTool` — stub web search |
| `crates/tutor-tools/src/code_exec.rs` | `CodeExecTool` — real shell execution via `OsEnv` |
| `crates/tutor-agent/Cargo.toml` | tutor-agent crate deps |
| `crates/tutor-agent/src/lib.rs` | re-exports `CapabilityRouter` |
| `crates/tutor-agent/src/error.rs` | `TutorError` |
| `crates/tutor-agent/src/capability.rs` | `CapabilityRouter`, `Capability` enum |
| `crates/tutor-agent/src/chat.rs` | `run_chat()` — builds harness, calls `prompt()`, awaits finish |
| `crates/tutor-agent/src/main.rs` | CLI entry point |

---

### Task 1: Workspace Cargo.toml

**Files:**
- Create: `Cargo.toml`
- Create: `.cargo/config.toml`

- [ ] **Step 1: Write workspace Cargo.toml**

```toml
# tutor_agent/Cargo.toml
[workspace]
members = [
    "crates/tutor-tools",
    "crates/tutor-agent",
]
resolver = "2"

[workspace.package]
edition = "2024"
version = "0.1.0"
license = "MIT"

[workspace.dependencies]
# Core (same rev as llm-harness-runtime)
llm-harness       = { git = "https://github.com/oh-my-harness/llm-harness-core", rev = "9ad7292c67c1d7828a907a7fd270a007b5d1f750" }
llm-harness-types = { git = "https://github.com/oh-my-harness/llm-harness-core", rev = "9ad7292c67c1d7828a907a7fd270a007b5d1f750" }

# Runtime crates (path deps relative to this workspace)
llm-harness-runtime            = { path = "../../llm-harness-runtime/crates/llm-harness-runtime" }
llm-harness-runtime-auth       = { path = "../../llm-harness-runtime/crates/llm-harness-runtime-auth" }
llm-harness-runtime-sandbox-os = { path = "../../llm-harness-runtime/crates/llm-harness-runtime-sandbox-os" }
llm-harness-runtime-audit-jsonl = { path = "../../llm-harness-runtime/crates/llm-harness-runtime-audit-jsonl" }

# LLM provider adapter (needed to construct AnthropicProvider)
llm_adapter = { git = "https://github.com/oh-my-harness/llm-api-adapter.git", rev = "c1d2cb87cb2bb94803144cfc28133a394b7fca18" }

# Common
anyhow      = "1"
futures     = "0.3"
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
thiserror   = "2"
tokio       = { version = "1", features = ["full"] }
tokio-util  = { version = "0.7", features = ["rt"] }
```

- [ ] **Step 2: Write .cargo/config.toml for local dev**

```toml
# tutor_agent/.cargo/config.toml
# Uncomment to use local checkouts of llm-harness-core:
# [patch."https://github.com/oh-my-harness/llm-harness-core"]
# llm-harness       = { path = "../../llm-harness-core/crates/llm-harness" }
# llm-harness-types = { path = "../../llm-harness-core/crates/llm-harness-types" }
```

- [ ] **Step 3: Verify workspace parses**

```bash
cd /Users/hhl/Documents/projs/tutor_agent
cargo metadata --no-deps --format-version 1 | python3 -c "import json,sys; d=json.load(sys.stdin); print([w['name'] for w in d['workspace_members']])"
```

Expected output lists `tutor-tools` and `tutor-agent` (after crates are created in later tasks).

- [ ] **Step 4: Commit**

```bash
git -C /Users/hhl/Documents/projs/tutor_agent add Cargo.toml .cargo/config.toml
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "chore(workspace): initialize Cargo workspace with shared deps"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 2: tutor-tools crate skeleton

**Files:**
- Create: `crates/tutor-tools/Cargo.toml`
- Create: `crates/tutor-tools/src/lib.rs`

- [ ] **Step 1: Write crate Cargo.toml**

```toml
# crates/tutor-tools/Cargo.toml
[package]
name = "tutor-tools"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
llm-harness-types = { workspace = true }
llm-harness-runtime-sandbox-os = { workspace = true }
anyhow.workspace = true
futures.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true
tokio-util.workspace = true
```

- [ ] **Step 2: Write lib.rs**

```rust
// crates/tutor-tools/src/lib.rs
pub mod code_exec;
pub mod rag_search;
pub mod web_search;

pub use code_exec::CodeExecTool;
pub use rag_search::RagSearchTool;
pub use web_search::WebSearchTool;
```

- [ ] **Step 3: Verify crate compiles (empty modules OK)**

Create empty placeholder files so cargo parses:

```bash
mkdir -p /Users/hhl/Documents/projs/tutor_agent/crates/tutor-tools/src
touch /Users/hhl/Documents/projs/tutor_agent/crates/tutor-tools/src/rag_search.rs
touch /Users/hhl/Documents/projs/tutor_agent/crates/tutor-tools/src/web_search.rs
touch /Users/hhl/Documents/projs/tutor_agent/crates/tutor-tools/src/code_exec.rs
```

Then:

```bash
cd /Users/hhl/Documents/projs/tutor_agent
cargo check -p tutor-tools 2>&1
```

Expected: compilation errors about missing structs (empty files) — that's fine for now. No `Cargo.toml` parse errors.

---

### Task 3: RagSearchTool

**Files:**
- Create: `crates/tutor-tools/src/rag_search.rs`

- [ ] **Step 1: Write failing test**

```rust
// At the bottom of crates/tutor-tools/src/rag_search.rs
#[cfg(test)]
mod tests {
    use super::*;
    use llm_harness_types::{ContentBlock, UnsupportedEnv};
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    fn make_ctx() -> ToolContext {
        let (tx, _rx) = mpsc::channel(1);
        ToolContext {
            env: Arc::new(UnsupportedEnv::new()),
            abort: CancellationToken::new(),
            tool_use_id: "test-id".into(),
            turn_index: 0,
            assistant_message: Arc::new(llm_harness_types::AssistantMessage {
                content: vec![],
                usage: None,
                stop_reason: None,
            }),
            update_tx: tx,
        }
    }

    #[tokio::test]
    async fn rag_search_returns_text_content() {
        let tool = RagSearchTool::new();
        let args = serde_json::json!({ "query": "integration by parts", "kb": "calculus" });
        let ctx = make_ctx();
        let result = tool.execute(args, &ctx).await.unwrap();
        assert!(!result.content.is_empty());
        match &result.content[0] {
            ContentBlock::Text { text } => assert!(!text.is_empty()),
            _ => panic!("expected text content"),
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cd /Users/hhl/Documents/projs/tutor_agent
cargo test -p tutor-tools rag_search -- --nocapture 2>&1
```

Expected: FAIL — `RagSearchTool` not found.

- [ ] **Step 3: Implement RagSearchTool**

```rust
// crates/tutor-tools/src/rag_search.rs
use futures::future::BoxFuture;
use llm_harness_types::{ContentBlock, Tool, ToolContext, ToolError, ToolResult};
use serde_json::json;

static SCHEMA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();

/// Stub RAG knowledge-base search tool.
/// v0.1: returns a static snippet keyed on the query.
/// Replace the body of `execute` with a real vector-store call in v0.2.
pub struct RagSearchTool;

impl RagSearchTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RagSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for RagSearchTool {
    fn name(&self) -> &str {
        "rag_search"
    }

    fn description(&self) -> &str {
        "Search the course knowledge base for relevant passages about a topic."
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        SCHEMA.get_or_init(|| {
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "kb": { "type": "string", "description": "Knowledge base name (optional)" }
                },
                "required": ["query"]
            })
        })
    }

    fn execute<'a>(
        &'a self,
        args: serde_json::Value,
        _ctx: &'a ToolContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        Box::pin(async move {
            let query = args["query"].as_str().unwrap_or("").to_string();
            let kb = args["kb"].as_str().unwrap_or("default").to_string();
            // v0.1 stub: echo back query with a placeholder passage
            let text = format!(
                "[RAG:{kb}] Found passage for \"{query}\": \
                 This is a stub result. Replace with real vector-store retrieval in v0.2."
            );
            Ok(ToolResult {
                content: vec![ContentBlock::Text { text }],
                details: json!({ "query": query, "kb": kb, "hits": 1 }),
                terminate: false,
            })
        })
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

```bash
cd /Users/hhl/Documents/projs/tutor_agent
cargo test -p tutor-tools rag_search -- --nocapture 2>&1
```

Expected: `test tests::rag_search_returns_text_content ... ok`

- [ ] **Step 5: Commit**

```bash
cd /Users/hhl/Documents/projs/tutor_agent
cargo fmt && cargo clippy -p tutor-tools --all-targets
git add crates/tutor-tools/
git commit -m "feat(tools): implement RagSearchTool stub"
git push
```

---

### Task 4: WebSearchTool

**Files:**
- Create: `crates/tutor-tools/src/web_search.rs`

- [ ] **Step 1: Write failing test**

```rust
// At the bottom of crates/tutor-tools/src/web_search.rs
#[cfg(test)]
mod tests {
    use super::*;
    use llm_harness_types::{ContentBlock, UnsupportedEnv};
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    fn make_ctx() -> ToolContext {
        let (tx, _rx) = mpsc::channel(1);
        ToolContext {
            env: Arc::new(UnsupportedEnv::new()),
            abort: CancellationToken::new(),
            tool_use_id: "test-id".into(),
            turn_index: 0,
            assistant_message: Arc::new(llm_harness_types::AssistantMessage {
                content: vec![],
                usage: None,
                stop_reason: None,
            }),
            update_tx: tx,
        }
    }

    #[tokio::test]
    async fn web_search_returns_text_content() {
        let tool = WebSearchTool::new();
        let args = serde_json::json!({ "query": "Riemann hypothesis" });
        let result = tool.execute(args, &make_ctx()).await.unwrap();
        assert!(!result.content.is_empty());
        match &result.content[0] {
            ContentBlock::Text { text } => assert!(text.contains("Riemann hypothesis")),
            _ => panic!("expected text"),
        }
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

```bash
cargo test -p tutor-tools web_search -- --nocapture 2>&1
```

- [ ] **Step 3: Implement WebSearchTool**

```rust
// crates/tutor-tools/src/web_search.rs
use futures::future::BoxFuture;
use llm_harness_types::{ContentBlock, Tool, ToolContext, ToolError, ToolResult};
use serde_json::json;

static SCHEMA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();

/// Stub web search tool.
/// v0.1: returns a placeholder result. Replace with real HTTP call in v0.2.
pub struct WebSearchTool;

impl WebSearchTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WebSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }

    fn description(&self) -> &str {
        "Search the web for up-to-date information about a topic."
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        SCHEMA.get_or_init(|| {
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" }
                },
                "required": ["query"]
            })
        })
    }

    fn execute<'a>(
        &'a self,
        args: serde_json::Value,
        _ctx: &'a ToolContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        Box::pin(async move {
            let query = args["query"].as_str().unwrap_or("").to_string();
            let text = format!(
                "[WEB] Search results for \"{query}\": \
                 This is a stub result. Replace with real HTTP search in v0.2."
            );
            Ok(ToolResult {
                content: vec![ContentBlock::Text { text }],
                details: json!({ "query": query, "results": 1 }),
                terminate: false,
            })
        })
    }
}
```

- [ ] **Step 4: Run test**

```bash
cargo test -p tutor-tools web_search -- --nocapture 2>&1
```

Expected: `ok`

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -p tutor-tools --all-targets
git -C /Users/hhl/Documents/projs/tutor_agent add crates/tutor-tools/src/web_search.rs
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(tools): implement WebSearchTool stub"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 5: CodeExecTool

**Files:**
- Create: `crates/tutor-tools/src/code_exec.rs`

The tool creates a temp dir via `OsEnv`, writes the source file, then calls `execute_shell`. When `execute_shell` returns `Err(EnvError::ShellFailed { exit_code, stderr })`, that is a successful execution with a non-zero exit code — report it as output, not a `ToolError`.

- [ ] **Step 1: Write failing tests**

```rust
// At the bottom of crates/tutor-tools/src/code_exec.rs
#[cfg(test)]
mod tests {
    use super::*;
    use llm_harness_runtime_sandbox_os::OsEnv;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    fn make_ctx(tmp: &std::path::Path) -> ToolContext {
        let (tx, _rx) = mpsc::channel(1);
        ToolContext {
            env: Arc::new(OsEnv::new(tmp)),
            abort: CancellationToken::new(),
            tool_use_id: "test-id".into(),
            turn_index: 0,
            assistant_message: Arc::new(llm_harness_types::AssistantMessage {
                content: vec![],
                usage: None,
                stop_reason: None,
            }),
            update_tx: tx,
        }
    }

    #[tokio::test]
    async fn python_hello_world() {
        let tmp = tempfile::tempdir().unwrap();
        let tool = CodeExecTool::new();
        let args = serde_json::json!({
            "language": "python",
            "code": "print('hello from test')"
        });
        let result = tool.execute(args, &make_ctx(tmp.path())).await.unwrap();
        let text = match &result.content[0] {
            llm_harness_types::ContentBlock::Text { text } => text.clone(),
            _ => panic!("expected text"),
        };
        assert!(text.contains("hello from test"), "got: {text}");
        assert_eq!(result.details["exit_code"], 0);
    }

    #[tokio::test]
    async fn nonzero_exit_is_not_tool_error() {
        let tmp = tempfile::tempdir().unwrap();
        let tool = CodeExecTool::new();
        let args = serde_json::json!({
            "language": "python",
            "code": "import sys; sys.exit(1)"
        });
        let result = tool.execute(args, &make_ctx(tmp.path())).await.unwrap();
        assert_eq!(result.details["exit_code"], 1);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test -p tutor-tools code_exec -- --nocapture 2>&1
```

Expected: FAIL — `CodeExecTool` not found.

- [ ] **Step 3: Implement CodeExecTool**

```rust
// crates/tutor-tools/src/code_exec.rs
use std::time::Duration;

use futures::future::BoxFuture;
use llm_harness_runtime_sandbox_os::OsEnv;
use llm_harness_types::{
    ContentBlock, EnvError, ExecutionEnv, ShellOptions, Tool, ToolContext, ToolError, ToolResult,
};
use serde_json::json;
use tokio_util::sync::CancellationToken;

static SCHEMA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();

/// Execute user-supplied code in a temporary directory via OsEnv.
/// v0.1: uses OsEnvSandbox (no real isolation). v0.2 will add bwrap/seatbelt.
pub struct CodeExecTool {
    timeout: Duration,
}

impl CodeExecTool {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(30),
        }
    }
}

impl Default for CodeExecTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for CodeExecTool {
    fn name(&self) -> &str {
        "code_exec"
    }

    fn description(&self) -> &str {
        "Execute code in a sandboxed environment and return stdout/stderr. \
         Supports 'python', 'bash'. Returns exit code and output."
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        SCHEMA.get_or_init(|| {
            json!({
                "type": "object",
                "properties": {
                    "language": {
                        "type": "string",
                        "enum": ["python", "bash"],
                        "description": "Programming language"
                    },
                    "code": {
                        "type": "string",
                        "description": "Code to execute"
                    }
                },
                "required": ["language", "code"]
            })
        })
    }

    fn execute<'a>(
        &'a self,
        args: serde_json::Value,
        ctx: &'a ToolContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        Box::pin(async move {
            let language = args["language"].as_str().unwrap_or("bash");
            let code = args["code"]
                .as_str()
                .ok_or_else(|| ToolError::InvalidArguments("missing code".into()))?;

            // Create a temp working directory
            let work_dir = ctx
                .env
                .create_temp_dir("tutor_code_exec")
                .await
                .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            let env = OsEnv::new(&work_dir);

            // Write source file
            let (filename, run_cmd) = match language {
                "python" => ("script.py", format!("python3 script.py")),
                "bash" => ("script.sh", format!("bash script.sh")),
                other => {
                    return Err(ToolError::InvalidArguments(format!(
                        "unsupported language: {other}"
                    )))
                }
            };

            env.write_file(
                std::path::Path::new(filename),
                code.as_bytes(),
                CancellationToken::new(),
            )
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

            // Execute — ShellFailed means non-zero exit, NOT a system error
            let opts = ShellOptions {
                cwd: Some(&work_dir),
                timeout: Some(self.timeout),
                abort: ctx.abort.clone(),
                env_vars: &[],
            };

            let (stdout, stderr, exit_code) = match env.execute_shell(&run_cmd, opts).await {
                Ok(out) => (out.stdout, out.stderr, out.exit_code),
                Err(EnvError::ShellFailed { exit_code, stderr }) => {
                    (String::new(), stderr, exit_code)
                }
                Err(e) => return Err(ToolError::ExecutionFailed(e.to_string())),
            };

            let output = format!(
                "exit_code: {exit_code}\n\
                 stdout:\n{stdout}\
                 {}",
                if stderr.is_empty() {
                    String::new()
                } else {
                    format!("stderr:\n{stderr}")
                }
            );

            Ok(ToolResult {
                content: vec![ContentBlock::Text { text: output }],
                details: json!({
                    "language": language,
                    "exit_code": exit_code,
                    "stdout": stdout,
                    "stderr": stderr,
                }),
                terminate: false,
            })
        })
    }
}
```

- [ ] **Step 4: Add tempfile dev-dep to tutor-tools Cargo.toml**

Add to `crates/tutor-tools/Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 5: Run tests**

```bash
cd /Users/hhl/Documents/projs/tutor_agent
cargo test -p tutor-tools code_exec -- --nocapture 2>&1
```

Expected:
```
test tests::python_hello_world ... ok
test tests::nonzero_exit_is_not_tool_error ... ok
```

- [ ] **Step 6: Commit**

```bash
cargo fmt && cargo clippy -p tutor-tools --all-targets
git -C /Users/hhl/Documents/projs/tutor_agent add crates/tutor-tools/
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(tools): implement CodeExecTool with OsEnv shell execution"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 6: tutor-agent crate + TutorError

**Files:**
- Create: `crates/tutor-agent/Cargo.toml`
- Create: `crates/tutor-agent/src/lib.rs`
- Create: `crates/tutor-agent/src/error.rs`

- [ ] **Step 1: Write Cargo.toml**

```toml
# crates/tutor-agent/Cargo.toml
[package]
name = "tutor-agent"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "tutor-agent"
path = "src/main.rs"

[dependencies]
tutor-tools = { path = "../tutor-tools" }
llm-harness       = { workspace = true }
llm-harness-types = { workspace = true }
llm-harness-runtime = { workspace = true }
llm-harness-runtime-auth = { workspace = true }
anyhow.workspace = true
futures.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true
tokio-util.workspace = true
```

- [ ] **Step 2: Write error.rs**

```rust
// crates/tutor-agent/src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TutorError {
    #[error("harness error: {0}")]
    Harness(#[from] llm_harness::HarnessError),
    #[error("capability not supported: {0}")]
    UnsupportedCapability(String),
    #[error("internal: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, TutorError>;
```

- [ ] **Step 3: Write lib.rs**

```rust
// crates/tutor-agent/src/lib.rs
pub mod capability;
pub mod chat;
pub mod error;

pub use capability::{Capability, CapabilityRouter};
pub use error::{Result, TutorError};
```

- [ ] **Step 4: Check compile**

```bash
cd /Users/hhl/Documents/projs/tutor_agent
# Create placeholder files first
touch crates/tutor-agent/src/capability.rs
touch crates/tutor-agent/src/chat.rs
touch crates/tutor-agent/src/main.rs
cargo check -p tutor-agent 2>&1
```

Expected: errors about missing types in empty files, not parse errors.

---

### Task 7: CapabilityRouter + Chat capability

**Files:**
- Create: `crates/tutor-agent/src/capability.rs`
- Create: `crates/tutor-agent/src/chat.rs`

- [ ] **Step 1: Write failing integration test in capability.rs**

```rust
// At the bottom of crates/tutor-agent/src/capability.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_from_str() {
        assert!(matches!(Capability::from_str("chat").unwrap(), Capability::Chat));
        assert!(matches!(Capability::from_str("deep_solve").unwrap(), Capability::DeepSolve));
        assert!(Capability::from_str("unknown").is_err());
    }
}
```

- [ ] **Step 2: Run to verify fail**

```bash
cargo test -p tutor-agent capability -- --nocapture 2>&1
```

- [ ] **Step 3: Write capability.rs**

```rust
// crates/tutor-agent/src/capability.rs
use std::str::FromStr;
use std::sync::Arc;

use llm_harness_types::ExecutionEnv;

use crate::error::{Result, TutorError};

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
}

impl CapabilityRouter {
    pub fn new(
        env: Arc<dyn ExecutionEnv>,
        model: impl Into<String>,
        anthropic_api_key: impl Into<String>,
    ) -> Self {
        Self {
            env,
            model: model.into(),
            anthropic_api_key: anthropic_api_key.into(),
        }
    }

    /// Route a question to the appropriate capability.
    pub async fn run(
        &self,
        capability: Capability,
        question: &str,
    ) -> Result<String> {
        match capability {
            Capability::Chat => crate::chat::run_chat(self, question).await,
            Capability::DeepSolve => {
                Err(TutorError::UnsupportedCapability("DeepSolve (Phase 2)".into()))
            }
            Capability::CodeExec => {
                Err(TutorError::UnsupportedCapability("CodeExec (Phase 2+)".into()))
            }
        }
    }
}
```

- [ ] **Step 4: Write chat.rs**

```rust
// crates/tutor-agent/src/chat.rs
use std::sync::Arc;

use llm_adapter::anthropic::AnthropicProvider;
use llm_harness::{AgentHarness, AgentHarnessOptions, AgentHarnessEvent};
use llm_harness_runtime_auth::EnvAuthHook;
use llm_harness_types::{AgentEvent, ContentBlock};
use tutor_tools::{RagSearchTool, WebSearchTool};

use crate::capability::CapabilityRouter;
use crate::error::Result;

/// Run a single Chat turn: question → [rag_search + web_search] → answer.
/// Creates a fresh in-memory harness per call (stateless in v0.1).
pub async fn run_chat(router: &CapabilityRouter, question: &str) -> Result<String> {
    let tools: Vec<Arc<dyn llm_harness_types::Tool>> = vec![
        Arc::new(RagSearchTool::new()),
        Arc::new(WebSearchTool::new()),
    ];

    let opts = AgentHarnessOptions {
        model: router.model.clone(),
        tools,
        system_prompt: Some(
            "You are a knowledgeable tutor. Use rag_search to find relevant course material, \
             web_search for supplementary information, then answer clearly and concisely."
                .into(),
        ),
        auth: Some(Arc::new(EnvAuthHook::for_provider("anthropic"))),
        ..AgentHarnessOptions::new(router.model.clone())
    };

    // AnthropicProvider is the concrete LlmClient implementation.
    // Uses the API key passed through CapabilityRouter.
    let client = Arc::new(AnthropicProvider::builder(&router.anthropic_api_key).build());

    // Subscribe before prompt() so we don't miss any events.
    let harness = AgentHarness::new_in_memory(client, router.env.clone(), opts).await;
    let mut rx = harness.subscribe();

    harness.prompt(question).await?;

    // Collect the last complete assistant message.
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
        "(no response)".into()
    } else {
        last_text
    })
}
```

**Note on `harness.subscribe()`:** Subscribe BEFORE calling `harness.prompt()` to avoid a race condition where the Settled event fires before the receiver is registered.

- [ ] **Step 5: Add llm_adapter to tutor-agent Cargo.toml**

Add to `crates/tutor-agent/Cargo.toml` `[dependencies]`:

```toml
llm_adapter.workspace = true
```

- [ ] **Step 6: Commit**

```bash
cd /Users/hhl/Documents/projs/tutor_agent
cargo fmt && cargo clippy -p tutor-agent --all-targets
git add crates/tutor-agent/
git commit -m "feat(agent): CapabilityRouter + Chat capability (Phase 1)"
git push
```

---

### Task 8: CLI binary + end-to-end smoke test

**Files:**
- Create: `crates/tutor-agent/src/main.rs`

- [ ] **Step 1: Write main.rs**

```rust
// crates/tutor-agent/src/main.rs
use std::sync::Arc;

use tutor_agent::{Capability, CapabilityRouter};
use llm_harness_runtime_sandbox_os::OsEnv;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let question = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "What is integration by parts?".into());

    // ANTHROPIC_API_KEY is read by the chat harness at call time via EnvAuthHook.
    if std::env::var("ANTHROPIC_API_KEY").is_err() {
        eprintln!("Error: ANTHROPIC_API_KEY not set");
        std::process::exit(1);
    }

    let env = Arc::new(OsEnv::new(std::env::current_dir()?));
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .expect("ANTHROPIC_API_KEY environment variable required");
    let router = CapabilityRouter::new(env, "claude-haiku-4-5-20251001", &api_key);

    println!("Question: {question}");
    let answer = router.run(Capability::Chat, &question).await?;
    println!("Answer:\n{answer}");

    Ok(())
}
```

- [ ] **Step 2: Build**

```bash
cd /Users/hhl/Documents/projs/tutor_agent
cargo build -p tutor-agent 2>&1
```

Expected: compiles without errors.

- [ ] **Step 3: Run smoke test (requires ANTHROPIC_API_KEY)**

```bash
cd /Users/hhl/Documents/projs/tutor_agent
ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY cargo run -p tutor-agent -- "What is integration by parts?"
```

Expected: the CLI prints a non-empty answer from the LLM using rag_search/web_search stubs.

- [ ] **Step 4: Commit**

```bash
cargo fmt && cargo clippy --all-targets --all-features
git -C /Users/hhl/Documents/projs/tutor_agent add crates/tutor-agent/src/main.rs
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(agent): CLI binary for Chat capability smoke test"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 9: Full test suite pass

- [ ] **Step 1: Run all tests**

```bash
cd /Users/hhl/Documents/projs/tutor_agent
cargo test --workspace 2>&1
```

Expected: all tests pass, no warnings about unused imports.

- [ ] **Step 2: Run clippy**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1
```

Expected: no warnings.

- [ ] **Step 3: Tag Phase 1 complete**

```bash
git -C /Users/hhl/Documents/projs/tutor_agent tag phase1-complete
git -C /Users/hhl/Documents/projs/tutor_agent push --tags
```

---

## Phase 1 Success Criteria

- `cargo test --workspace` passes
- `cargo clippy --workspace --all-targets` passes with no warnings
- CLI binary runs and returns an LLM answer for a Chat question
- `RagSearchTool`, `WebSearchTool`, `CodeExecTool` all have passing unit tests
- `Capability::from_str` correctly routes `"chat"` / `"deep_solve"` / `"code_exec"`
