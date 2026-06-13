# Tutor Agent Phase 5: Docs + Framework Feedback

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Write user-facing README + quickstart, run the full end-to-end scenario in the browser, and write a structured framework feedback report for `llm-harness-runtime`.

**Architecture:** This phase is documentation and validation only — no new production code. The framework feedback goes into a structured markdown file under `docs/` and may become GitHub issues or PRs in the `llm-harness-runtime` repo.

**Tech Stack:** Markdown, `cargo test`, manual browser testing, `gh issue create` (optional).

---

## File Map

| File | Responsibility |
|------|---------------|
| `README.md` | Project overview, quickstart, capability descriptions |
| `docs/framework-feedback.md` | Structured report on llm-harness-runtime usage |
| `docs/quickstart-deep-solve.md` | Step-by-step Deep Solve walkthrough |

---

### Task 1: README

**Files:**
- Create: `README.md`

- [ ] **Step 1: Write README**

```markdown
# tutor_agent

A Rust-based AI tutor powered by [llm-harness-runtime](https://github.com/oh-my-harness/llm-harness-runtime).

## Capabilities

| Capability | Description |
|-----------|-------------|
| **Chat** | Conversational Q&A with RAG knowledge base retrieval |
| **Deep Solve** | Multi-phase problem solving: Pre-retrieve → Plan → Solve → Synthesize, with REPLAN back-edge |
| **Code Exec** | Execute Python/Bash code with explanation |

## Quickstart

### Requirements

- Rust 2024 edition (`rustup update stable`)
- Node.js 20+ (for the web UI)
- `ANTHROPIC_API_KEY` environment variable

### Run the CLI

```bash
cd /path/to/tutor_agent
export ANTHROPIC_API_KEY=sk-ant-...

# Chat capability
cargo run -p tutor-agent -- "What is integration by parts?"

# Deep Solve
cargo run -p tutor-agent -- --capability deep_solve "Evaluate the integral of x^2 from 0 to 2"
```

### Run the Web UI

```bash
# Terminal 1: start backend
ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY cargo run -p tutor-web

# Terminal 2: start frontend
cd web-ui
npm install
npm run dev
```

Open `http://localhost:5173` in your browser.

## Architecture

```
web-ui (Vite + React + Tailwind)
  ↕ WebSocket / REST
tutor-web (axum)
  ↓
tutor-agent
  ├── Chat capability
  ├── SolveOrchestrator (Deep Solve)
  │   ├── ReplanHook (BeforeToolCallHook)
  │   └── PhaseManager (PrepareNextTurnHook)
  └── GovernanceConfig
      ├── BudgetControlAdapter
      ├── JsonlAuditSink
      └── HumanApprovalWrapper
  ↓
tutor-tools
  ├── RagSearchTool
  ├── WebSearchTool
  └── CodeExecTool (OsEnv)
  ↓
llm-harness-runtime
```

## v0.1 Scope Limits

- RAG search is a stub (returns placeholder text) — replace with real vector store in v0.2
- Web search is a stub — replace with real HTTP search in v0.2
- Code execution uses OsEnvSandbox (no real isolation) — add bwrap/seatbelt in v0.2
- Single-user only — no multi-user session isolation
```

- [ ] **Step 2: Commit**

```bash
git -C /Users/hhl/Documents/projs/tutor_agent add README.md
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "docs: add README with quickstart and architecture overview"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 2: Full end-to-end validation

- [ ] **Step 1: Run all unit tests**

```bash
cd /Users/hhl/Documents/projs/tutor_agent
cargo test --workspace 2>&1
```

Expected: all tests pass with zero failures.

- [ ] **Step 2: Run clippy**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings 2>&1
```

Expected: zero warnings.

- [ ] **Step 3: Build frontend**

```bash
cd /Users/hhl/Documents/projs/tutor_agent/web-ui
npm run build 2>&1
npx tsc --noEmit 2>&1
```

Expected: both succeed with no errors.

- [ ] **Step 4: Smoke test Chat capability via CLI**

```bash
cd /Users/hhl/Documents/projs/tutor_agent
ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY cargo run -p tutor-agent -- "What is integration by parts?"
```

Expected: non-empty answer printed to stdout.

- [ ] **Step 5: Smoke test Deep Solve via CLI (with real ANTHROPIC_API_KEY)**

```bash
ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY cargo test --test deep_solve_integration -- --ignored --nocapture 2>&1
```

Expected: Deep Solve pipeline runs and prints a final answer containing a numeric result.

- [ ] **Step 6: Browser end-to-end**

Start both servers (see README quickstart). In the browser:
1. Select **Chat** tab → send "What is the derivative of sin(x)?" → verify streaming response appears
2. Select **Deep Solve** tab → send "What is ∫x² dx from 0 to 2?" → verify TracePanel shows phase_start events
3. Select **Code Exec** tab → send "Run: print(sum(range(10)))" → verify ApprovalDialog appears, approve it, verify answer includes 45

---

### Task 3: Framework feedback report

**Files:**
- Create: `docs/framework-feedback.md`

- [ ] **Step 1: Write framework feedback document**

After completing Phases 1–4, write this document based on actual experience. The template below must be filled in with real observations:

```markdown
# llm-harness-runtime v0.2 Framework Feedback

> Written after implementing tutor_agent v0.1 (2026-06-13).
> This document records friction points, missing APIs, and positive validations
> to inform llm-harness-runtime v0.3 planning.

## Hooks Used

| Hook | Use Case | Verdict |
|------|---------|---------|
| `BeforeToolCallHook` | ReplanHook intercepts `replan()` | ✅ worked as designed |
| `PrepareNextTurnHook` | PhaseManager sets active_tools per step | ✅ worked as designed |
| `AfterProviderResponseHook` | BudgetControlAdapter accumulates cost | ✅ worked as designed |
| `ShouldStopHook` | BudgetControlAdapter hard-stops loop | ✅ worked as designed |

## Friction Points

### [Fill in after implementation]

Example format:
- **`AgentHarness::new_in_memory` takes an `Arc<dyn LlmClient>` but auth resolution is unclear**
  - Expected: auth hook to be the sole auth source
  - Actual: still need to pass a client explicitly
  - Suggestion: provide `AgentHarness::with_auth_hook()` that creates the client from the hook

- **`BeforeToolCallDecision::Deny` takes `ToolResult`, not a plain error string**
  - Finding: non-obvious that `content: vec![]` is valid
  - Suggestion: add a `ToolResult::error(msg)` convenience constructor

### Positive Validations

- **CompositeBeforeToolCallHook** chains ReplanHook and HumanApprovalWrapper cleanly
- **BudgetControlAdapter** dual-role as `AfterProviderResponseHook` + `ShouldStopHook` is elegant
- **`active_tools` in `NextTurnDirective`** is exactly the right granularity for PhaseManager

## API Gaps

| Gap | Description | Severity |
|-----|-------------|----------|
| [Fill in] | | |

## Proposed v0.3 Changes

1. [Fill in based on actual friction]
2. [Fill in]

## Issues to File

- [ ] [Describe each issue that warrants a GitHub issue in llm-harness-runtime]
```

- [ ] **Step 2: File GitHub issues (optional)**

For each friction point that warrants a framework change:

```bash
cd /Users/hhl/Documents/projs/llm-harness-runtime
gh issue create \
  --title "[Feedback from tutor_agent] <title>" \
  --body "<description from framework-feedback.md>" \
  --label "feedback"
```

- [ ] **Step 3: Commit**

```bash
git -C /Users/hhl/Documents/projs/tutor_agent add docs/framework-feedback.md
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "docs: framework feedback report after v0.1 implementation"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 4: Final tag and cleanup

- [ ] **Step 1: Final test run**

```bash
cd /Users/hhl/Documents/projs/tutor_agent
cargo test --workspace 2>&1
cargo clippy --workspace --all-targets -- -D warnings 2>&1
```

Expected: zero failures, zero warnings.

- [ ] **Step 2: Tag v0.1.0**

```bash
git -C /Users/hhl/Documents/projs/tutor_agent tag -a v0.1.0 -m "tutor_agent v0.1.0: Chat + Deep Solve + Code Exec"
git -C /Users/hhl/Documents/projs/tutor_agent push --tags
```

- [ ] **Step 3: Write summary comment in each phase plan**

Add a one-line "Phase N complete: <date>" comment at the top of each plan file to mark completion.

---

## Phase 5 Success Criteria

- `cargo test --workspace` passes
- `npm run build` in `web-ui/` succeeds
- README accurately describes how to run both CLI and web UI
- Framework feedback document has at least 3 concrete observations (positive or negative)
- `v0.1.0` tag is pushed to the repo
