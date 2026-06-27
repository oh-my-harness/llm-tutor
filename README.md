# llm-tutor

An AI learning workspace built on top of
[llm-harness-runtime](https://github.com/oh-my-harness/llm-harness-runtime).

The project combines chat, RAG knowledge bases, guided problem solving, quiz
generation, web research, and lightweight book/report persistence.

## 使用方式

### Requirements

- Rust 2024 edition (`rustup update stable`)
- Node.js 20+
- One supported LLM provider API key.
- Optional: embedding provider API key for knowledge-base/RAG indexing.
- Optional: web search API key for paid search providers.
- Protobuf compiler (`protoc`) for LanceDB
  - Windows: `winget install --id Google.Protobuf`

### Install

```powershell
# install frontend dependencies once
cd web-ui
npm install
cd ..

# optional sanity check
cargo test -p tutor-web --lib
```

### Start

Run the backend:

```powershell
cargo run -p tutor-web
```

The backend listens on `http://127.0.0.1:8080`.

Run the frontend in another terminal:

```powershell
cd web-ui
npm run dev
```

Open `http://localhost:5173`.

### Web Configuration

Open **Settings** in the Web UI and configure these items:

1. **LLM**
   - Add a model config.
   - Choose an interface mode such as OpenAI-compatible, Anthropic, or DeepSeek.
   - Fill in `base_url`, API key, model name, optional chat path, and context window.
   - Use the test button to verify the model connection.

2. **Embedding Model**
   - Required for creating and indexing knowledge bases.
   - Add an OpenAI-compatible embedding config.
   - Fill in `base_url`, API key, model, optional `/v1/embeddings` path, dimensions, and whether to send the `dimensions` parameter.
   - Use the test button to confirm the returned embedding dimension.

3. **Search**
   - Optional for normal chat, recommended for Research mode.
   - DuckDuckGo can be used as a free fallback, but quality and availability are unstable.
   - Paid providers such as Bing, Brave, Tavily, Serper, SerpAPI, or Exa can be configured for more reliable research.

4. **Knowledge Base**
   - Go to **知识库**.
   - Create a knowledge base and select an embedding config.
   - Upload PDF or text documents.
   - Wait for parsing, chunking, embedding, and LanceDB indexing to finish.
   - Select the knowledge base in chat when you want RAG retrieval.

Local product data is stored under `.llm-tutor/` in the project root.

### What You Can Do

| Area | What it supports |
|---|---|
| **Chat** | Conversational Q&A with streaming output, conversation history, attachments, optional RAG, web search/fetch, code execution, and trace events. |
| **Knowledge Base / RAG** | Create knowledge bases, upload documents, parse PDF/text, index with LanceDB, retrieve chunks, and show answer citations when `rag_search` is actually used. |
| **Deep Solve** | Structured problem solving with plan, steps, evidence, citations, final answer, and trace/status events. |
| **Quiz** | Generate and answer quizzes from a knowledge base or conversation material, with structured output and source-backed questions where source material exists. |
| **Research** | Search the web, fetch pages, synthesize a Markdown research report, show progress/trace events, and save reports into books/notes flows. |
| **Space** | Notebook, Quiz Bank, and Student Profile surfaces for organizing learning artifacts and memory projections. |
| **Memory** | L1/L2/L3 Markdown memory, manual update/check/dedupe workbench, source refs, proposed facts/edits preview, and `read_memory`/`write_memory` tools for agents. |
| **Books** | Lightweight book/chapter storage for polished reports and saved outputs. |

### Optional CLI

```powershell
# Chat
cargo run -p tutor-agent -- "What is integration by parts?"

# Deep Solve
cargo run -p tutor-agent -- --capability deep_solve "Evaluate the integral of x^2 from 0 to 2"
```

For CLI or environment-driven runs, the usual variables are:

```powershell
$env:LLM_PROVIDER="openai"
$env:OPENAI_API_KEY="sk-..."
$env:LLM_MODEL="gpt-4o-mini"
$env:OPENAI_BASE_URL="https://api.openai.com"
$env:OPENAI_CHAT_PATH="/v1/chat/completions"
```

DeepSeek or other OpenAI-compatible gateways can use the OpenAI-compatible mode
with their own base URL and model name.

On Bash/Zsh, use `export NAME=value` instead.

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
- Space, Notebook, Quiz Bank, Student Profile, and Memory surfaces.
- L1/L2/L3 Markdown memory consolidation with structured update/check/dedupe.
- Basic book/chapter browser.

Still early:

- Research reports are not yet stored as first-class `ResearchReport` records.
- Book editing is a simple chapter viewer, not a rich editor.
- RAG chunking is still basic and should become paragraph/token-aware.
- Citation quality and source validation need more work.
- Local persistence is mostly JSON plus runtime session storage; SQLite may be needed later.
- Multi-user, auth, permissions, deployment, and collaboration are out of scope for now.

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
