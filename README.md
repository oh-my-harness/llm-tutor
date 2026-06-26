# llm-tutor

An AI learning workspace built on top of
[llm-harness-runtime](https://github.com/oh-my-harness/llm-harness-runtime).

The project combines chat, RAG knowledge bases, guided problem solving, quiz
generation, web research, and lightweight book/report persistence.

## Current Status

`llm-tutor` is a single-user local product prototype. The core learning loop is
usable, but several parts are still MVP-quality and need quality/polish work.

Implemented:

- Runtime-backed chat sessions and conversation history.
- Streaming assistant output over WebSocket.
- Trace/status events for tools and long-running workflows.
- LLM, embedding, and web search configuration in the UI.
- Knowledge bases with document upload, PDF/text parsing, LanceDB indexing, and retrieval.
- RAG citations and source chunk display.
- Chat attachments, including PDF/text parsing for current-turn context.
- Deep Solve structured problem-solving UX.
- Code execution tool.
- Quiz generation and answer flow.
- Web search and web page fetch tools.
- Research mode that searches, reads, and produces a Markdown report.
- Saving research reports into books as chapters.
- Basic book/chapter browser.

Still early:

- Research reports are not yet stored as first-class `ResearchReport` records.
- Book editing is a simple chapter viewer, not a rich editor.
- Chunking is still basic and should become paragraph/token-aware.
- Citation quality and source validation need more work.
- Local persistence is mostly JSON plus runtime session storage; SQLite may be needed later.
- Multi-user, auth, permissions, deployment, and collaboration are out of scope for now.

## Capabilities

| Capability | Description |
|---|---|
| **Chat** | Conversational Q&A with optional RAG, web search, web fetch, code execution, and attachments. |
| **Deep Solve** | Structured multi-step solving workflow with plan, steps, evidence, citations, and final answer. |
| **Code Exec** | Code-oriented mode that can use `code_exec` for verification. |
| **Quiz** | Generates and runs quiz sessions from a knowledge base or conversation material. |
| **Research** | Searches external sources, fetches pages, synthesizes a cited Markdown report, and can save it to books. |

## Quickstart

### Requirements

- Rust 2024 edition (`rustup update stable`)
- Node.js 20+
- One supported LLM provider API key
- Protobuf compiler (`protoc`) for LanceDB
  - Windows: `winget install --id Google.Protobuf`

### Run the Backend

```bash
cargo run -p tutor-web
```

The backend listens on:

```text
http://127.0.0.1:8080
```

### Run the Frontend

```bash
cd web-ui
npm install
npm run dev
```

Open:

```text
http://localhost:5173
```

Most provider configuration can be entered from the Settings page.

### Optional CLI

```bash
# Chat
cargo run -p tutor-agent -- "What is integration by parts?"

# Deep Solve
cargo run -p tutor-agent -- --capability deep_solve "Evaluate the integral of x^2 from 0 to 2"
```

## Provider Configuration

The Web UI supports configurable LLM, embedding, and search providers.

For CLI or environment-driven runs, the usual variables are:

```bash
export LLM_PROVIDER=openai
export OPENAI_API_KEY=sk-...
export LLM_MODEL=gpt-4o-mini
export OPENAI_BASE_URL=https://api.openai.com
export OPENAI_CHAT_PATH=/v1/chat/completions
```

DeepSeek or other OpenAI-compatible gateways can use the OpenAI-compatible mode
with their own base URL and model name.

## Architecture

```text
web-ui (React + Vite + Tailwind)
  -> REST / WebSocket
tutor-web (Axum)
  -> SessionPool / runtime sessions
  -> tutor-agent
      |-- CapabilityRouter
      |-- Chat / Research harness runs
      |-- Deep Solve orchestrator
      |-- Quiz generation helpers
      `-- Governance / budget / audit hooks
  -> tutor-tools
      |-- rag_search
      |-- web_search
      |-- web_fetch
      `-- code_exec
  -> tutor-rag
      `-- LanceDB + embedding-backed retrieval
  -> llm-harness-runtime / llm-harness-agent
```

### Crates

```text
crates/tutor-agent   Agent capabilities, prompts, router, Deep Solve, Quiz generation.
crates/tutor-tools   Tools exposed to the agent: RAG, web search/fetch, code execution.
crates/tutor-rag     LanceDB ingestion/search and embedding integration.
crates/tutor-web     Axum server, WebSocket, session APIs, knowledge, quiz, books.
web-ui               React frontend.
docs                 Roadmaps, specs, framework feedback, and feature plans.
```

## Local Data

Runtime and product data are stored under `.llm-tutor/` in the project root.

Typical files include:

```text
.llm-tutor/knowledge-bases.json
.llm-tutor/quizzes.json
.llm-tutor/books.json
```

Vector data is managed by LanceDB under the configured RAG root.

## Tests

```bash
# Rust tests
cargo test --workspace -j 1

# Agent mock integration tests
cargo test -p tutor-agent --test mock_integration -j 1

# Backend API/store tests
cargo test -p tutor-web --lib -j 1

# Frontend build
cd web-ui
npm run build
```

Real-provider integration tests may require API keys and are usually ignored by
default.

## Development Principles

See [AGENTS.md](./AGENTS.md).

The short version:

- Use `llm-harness-runtime` / `llm-harness-agent` first.
- Do not reimplement runtime session, context building, tool orchestration,
  trace, compaction, or provider behavior in this repo.
- Keep this repo focused on product data and UI: knowledge bases, documents,
  spaces, books, quizzes, settings, research reports, and mappings to runtime
  sessions.

## License

MIT
