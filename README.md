# tutor_agent

A Rust-based AI tutor powered by [llm-harness-runtime](https://github.com/oh-my-harness/llm-harness-runtime).

## Capabilities

| Capability | Description |
|-----------|-------------|
| **Chat** | Conversational Q&A with RAG knowledge base retrieval (stub in v0.1) |
| **Deep Solve** | Multi-phase problem solving: Pre-retrieve → Plan → Solve → Synthesize, with REPLAN back-edge |
| **Code Exec** | Execute Python/Bash code with explanation via OsEnv sandbox |

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

# Deep Solve (requires ANTHROPIC_API_KEY)
ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY cargo run -p tutor-agent -- --capability deep_solve "Evaluate the integral of x^2 from 0 to 2"
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

### Run Tests

```bash
# Rust unit tests (workspace-wide)
cargo test --workspace

# Deep Solve integration tests (requires ANTHROPIC_API_KEY)
ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY cargo test --test deep_solve_integration -- --ignored --nocapture

# TypeScript type check
cd web-ui && npx tsc --noEmit

# Vite production build
cd web-ui && npm run build
```

## Architecture

```
web-ui (Vite + React + Tailwind)
  ↕ WebSocket / REST
tutor-web (axum server)
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

## Project Structure

```
tutor_agent/
├── Cargo.toml                    (workspace)
├── crates/
│   ├── tutor-tools/              (Tool implementations)
│   │   └── src/
│   │       ├── rag_search.rs
│   │       ├── web_search.rs
│   │       └── code_exec.rs
│   ├── tutor-agent/              (Orchestration core)
│   │   └── src/
│   │       ├── capability.rs
│   │       ├── chat.rs
│   │       ├── solve_orchestrator.rs
│   │       ├── solve_context.rs
│   │       ├── replan_hook.rs
│   │       ├── phase_manager.rs
│   │       ├── governance.rs
│   │       ├── terminal_approver.rs
│   │       └── main.rs          (CLI entry point)
│   └── tutor-web/                (HTTP server)
│       └── src/
│           ├── stream.rs        (TutorStream + StreamEvent)
│           ├── session.rs       (SessionPool)
│           ├── routes/
│           ├── main.rs          (Server entry point)
├── web-ui/                       (Vite + React + Tailwind)
│   └── src/
│       ├── App.tsx
│       ├── hooks/
│       └── components/
└── docs/
    ├── specs/
    └── plans/
```

## v0.1 Scope Limits

- RAG search is a stub (returns placeholder text) — replace with real vector store in v0.2
- Web search is a stub — replace with real HTTP search in v0.2
- Code execution uses OsEnvSandbox (no real isolation) — add bwrap/seatbelt in v0.2
- Single-user only — no multi-user session isolation
- Audit events use placeholder session/trace IDs — wire real tracing in v0.2

## License

MIT
