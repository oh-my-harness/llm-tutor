# AI Tutor Product Roadmap

> Status: planning | Date: 2026-06-20 | Scope: turn `llm-tutor` from a runtime demo into a usable AI learning workspace.

## 1. Product Direction

The product should start as a focused AI learning workspace:

> Upload learning materials, build a knowledge base, ask questions, solve hard problems step by step, generate quizzes, and save useful learning records.

This keeps the first product loop narrow enough to ship while leaving room to grow toward a DeepTutor-like platform.

### Target Users

- Self-learners who study from PDFs, notes, textbooks, or course handouts.
- Students who need guided explanations, practice questions, and review history.
- Teachers or small teams who want to turn learning materials into Q&A and quiz workflows.

### Product Principles

- Knowledge-grounded answers before broad general chat.
- Learning workflows before general productivity features.
- Inspectable reasoning traces for long tasks.
- Persistent learning records, not disposable one-off chats.
- Add platform features only after the core learning loop feels useful.

## 2. MVP Scope

### Core Loop

1. User creates or selects a knowledge base.
2. User uploads documents.
3. System parses, chunks, embeds, and indexes the documents.
4. User asks questions against the knowledge base.
5. System answers with citations and source snippets.
6. User escalates a hard question into Deep Solve.
7. User generates quiz questions from the same materials.
8. User saves important answers, notes, or wrong questions.

### MVP Modules

| Module | Purpose | MVP Capability |
|---|---|---|
| Chat | Main interaction surface | Ask questions with optional RAG grounding |
| Knowledge Base | User-owned document library | Upload, parse, index, retrieve, show sources |
| Deep Solve | Guided problem solving | Plan -> solve steps -> synthesize final answer |
| Quiz | Practice and assessment | Generate questions, collect answer, judge response |
| Notebook | Persistent review | Save answers, notes, quiz results, source links |
| Settings | Runtime control | LLM provider, model, API key, budget limit |

### Explicitly Out of MVP

- TutorBot channel integrations.
- Multi-user auth and admin dashboard.
- Three-layer memory.
- Book Engine / living books.
- Math Animator.
- Deep Research with parallel sub-agents.
- Full plugin marketplace.

## 3. Technical Direction

The current `llm-tutor` Rust workspace should remain the starting point.

### Backend

- Keep Rust as the orchestration backend.
- Keep `llm-harness-runtime` as the runtime foundation.
- Add persistent storage with SQLite first.
- Use WebSocket for streaming content, trace events, and tool status.
- Keep capabilities behind a clear router: `chat`, `deep_solve`, `quiz`, later `research` and `visualize`.

### Frontend

- Keep the current React UI for the next step.
- Improve it into a learning workspace rather than a debug console.
- Defer a Next.js migration until routing, auth, or server-side rendering becomes valuable.

### RAG

Start simple and local:

- Document formats: Markdown, TXT, PDF first.
- Chunking: token or paragraph-based chunks.
- Embeddings: configurable provider.
- Store: SQLite + vector extension, LanceDB, or Qdrant.
- Retrieval: top-k semantic search with source metadata.

Recommended first implementation: SQLite for metadata plus a small local vector store abstraction, so the app can swap storage later.

### Storage Model

Minimum entities:

- `sessions`
- `messages`
- `knowledge_bases`
- `documents`
- `chunks`
- `retrieval_hits`
- `notebooks`
- `notebook_entries`
- `quiz_questions`
- `quiz_attempts`

## 4. Phase Plan

### Phase 0: Stabilize Runtime Baseline

Goal: make the current app reproducible and easy to run.

- [ ] Finalize dependency strategy: git dependencies or local path dependencies, but not a mix that causes version drift.
- [ ] Update README with the correct backend port and current launch steps.
- [ ] Add a smoke test for `tutor-web` startup.
- [ ] Keep `cargo check --workspace`, `cargo test --workspace`, and `npm run build` green.

#### v0.1 Carryover Checklist

These items come from the earlier Phase 1-5 plans. They should be closed before starting larger product work, otherwise later phases will build on an uneven foundation.

- [ ] Wire top-level `Capability::CodeExec` instead of returning `UnsupportedCapability`.
- [ ] Decide whether `code_exec` requires approval in CLI, Web, both, or only when configured.
- [ ] Wire `BudgetControlAdapter` into both cost accumulation and stop/limit behavior for every harness.
- [ ] Emit real `TutorStream::trace` events from Chat and Deep Solve, especially phase transitions, tool calls, and replan events.
- [ ] Confirm WebSocket output semantics: final-only response, chunked text stream, or mixed content/trace/status stream.
- [ ] Replace Deep Solve `run_pre_retrieve` stub with either real RAG retrieval or an explicit no-KB branch.
- [ ] Update README with accurate backend port, provider setup, dependency strategy, and known v0.1 limitations.
- [ ] Add or remove the planned `docs/quickstart-deep-solve.md`; avoid stale file references in plans.
- [ ] Run `cargo clippy --workspace --all-targets --all-features -- -D warnings` and either fix warnings or document accepted warnings.
- [ ] Document how to run ignored real-provider tests and what API keys are required.

Acceptance:

- A clean clone can run backend and frontend with documented commands.
- No local sibling repository is required unless explicitly documented.
- All historical Phase 1-5 carryover items are either complete or explicitly moved into a later product phase.

### Phase 1: Real Knowledge Base

Goal: replace RAG stubs with real document ingestion and retrieval.

- [ ] Add `knowledge_bases` and `documents` storage.
- [ ] Add document upload endpoint.
- [ ] Parse TXT and Markdown.
- [ ] Parse PDF text.
- [ ] Chunk documents with stable chunk IDs.
- [ ] Add embedding provider configuration.
- [ ] Store chunk vectors and metadata.
- [ ] Replace `RagSearchTool` placeholder with real retrieval.
- [ ] Return citations to the UI.

Acceptance:

- User uploads a document and asks a question.
- Answer includes cited source chunks.
- Retrieval can be tested without a real LLM by querying the index directly.

### Phase 2: Session Persistence

Goal: turn chat from ephemeral messages into a resumable learning history.

- [ ] Persist sessions and messages.
- [ ] Add session list API.
- [ ] Add session detail API.
- [ ] Add resume session support in Web UI.
- [ ] Store selected capability and knowledge base per session.
- [ ] Persist trace events or compact task summaries.

Acceptance:

- Refreshing the browser does not lose the conversation.
- User can resume a previous session and continue with context.

### Phase 3: Deep Solve UX

Goal: make long reasoning inspectable and useful.

- [ ] Stream phase events: plan, solve step, replan, synthesize.
- [ ] Show a structured Trace Panel with phase status.
- [ ] Show intermediate step results separately from the final answer.
- [ ] Allow user to stop a running solve.
- [ ] Add mock tests for replan and phase event emission.

Acceptance:

- Deep Solve feels like a guided workflow, not a delayed chat response.
- The user can see what stage the system is in.

### Phase 4: Quiz and Answer Judging

Goal: add the first active learning workflow.

- [ ] Add `quiz` capability.
- [ ] Generate questions from selected knowledge base chunks.
- [ ] Support short answer and multiple choice first.
- [ ] Add answer judging prompt.
- [ ] Save quiz questions and attempts.
- [ ] Show explanations and source references.

Acceptance:

- User can generate questions from a document.
- User can answer and receive feedback.
- Wrong answers can be saved for later review.

### Phase 5: Notebook

Goal: make learning outputs reusable.

- [ ] Add notebooks and notebook entries.
- [ ] Save chat answers to notebook.
- [ ] Save quiz results to notebook.
- [ ] Save source snippets.
- [ ] Add notebook browser UI.
- [ ] Add `write_note` and `list_notebook` tools.

Acceptance:

- User can keep important learning records without leaving the chat.
- Future turns can reference saved notes.

### Phase 6: Product Polish

Goal: make the app feel like a learning product, not only a developer demo.

- [ ] Redesign the UI around three areas: Chat, Knowledge, Notebook.
- [ ] Add clear empty states.
- [ ] Add upload progress and indexing status.
- [ ] Add model/provider health checks.
- [ ] Add error recovery for missing API keys and failed embeddings.
- [ ] Add export for chat and notebook entries.

Acceptance:

- A new user can understand the app without reading source docs.
- Common failures produce actionable messages.

## 5. Later Expansion

After the MVP loop works, consider larger DeepTutor-like surfaces:

| Feature | Why Later |
|---|---|
| Deep Research | Needs robust citations, parallel task orchestration, and report editing |
| Book Engine | Requires stable document model and many UI block types |
| Three-layer Memory | Needs enough real usage data to justify memory consolidation |
| TutorBot | Needs auth, workspace isolation, and channel security |
| Multi-user | Changes storage, permissions, and deployment model |
| Math Animator | Requires media rendering pipeline and visual QA |

## 6. Immediate Next Tasks

Recommended next implementation order:

1. Decide dependency strategy and commit the current runtime alignment.
2. Update README launch instructions.
3. Implement SQLite persistence scaffolding.
4. Implement real RAG ingestion for TXT/Markdown.
5. Extend to PDF parsing.
6. Wire `RagSearchTool` to the real retriever.
7. Add citations in Web UI.

## 7. Risks

| Risk | Mitigation |
|---|---|
| Scope creep from copying DeepTutor wholesale | Keep MVP centered on upload -> ask -> solve -> quiz -> save |
| RAG quality is poor | Start with source-visible retrieval debugging and chunk previews |
| Provider differences break tool calls | Keep mock provider tests and provider-specific integration tests |
| Long tasks feel opaque | Stream trace/status events early |
| Storage model churn | Use simple SQLite migrations and avoid premature multi-user design |
| UI becomes a debug console | Design around learner workflows, not internal events |

## 8. Success Metrics

Early qualitative metrics:

- User can upload a course note and get a grounded answer in under 2 minutes.
- User can solve a difficult question with visible intermediate steps.
- User can generate useful quiz questions from their own material.
- User can save and later find an important answer or mistake.

Engineering metrics:

- `cargo test --workspace` passes.
- Frontend build passes.
- RAG retrieval has deterministic tests.
- At least one end-to-end smoke path covers document upload -> retrieval -> answer.
