# AI Tutor Product Roadmap

> Status: in progress | Date: 2026-06-20 | Last updated: 2026-06-30 | Scope: turn `llm-tutor` from a runtime demo into a usable AI learning workspace.

> Superseding decision (2026-07-14): Books are retired and Research reports use
> Notebook as their only durable destination. Historical Book milestones below
> describe earlier implementation work and are not future product direction.

## 0. Current Planning Entry Points

Use this roadmap for product direction and milestone context. For execution,
prefer the newer focused plans:

- Current product slice:
  `docs/plans/2026-06-26-next-product-slice-plan.md`
- Consolidated requirements:
  `docs/specs/2026-06-26-product-requirements-spec.md`
- Desktop release hardening:
  `docs/plans/2026-06-28-tauri-desktop-release-plan.md`

The original 2026-06-13 Phase 1-5 implementation checklists are historical and
have been superseded by the current product plans.

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
| Space | Learning workspace | Notebook, quiz bank, and student profile |
| Settings | Runtime control | LLM provider, model, API key, budget limit |

### Explicitly Out of MVP

- TutorBot channel integrations.
- Multi-user auth and admin dashboard.
- Separate Book or publication layer.
- Math Animator.
- Deep Research with parallel sub-agents.
- Full plugin marketplace.

## 3. Technical Direction

The current `llm-tutor` Rust workspace should remain the starting point.

### Backend

- Keep Rust as the orchestration backend.
- Keep `llm-harness-runtime` as the runtime foundation.
- Keep local JSON stores for MVP product data; consider SQLite when schema churn grows.
- Use WebSocket for streaming content, trace events, and tool status.
- Keep capabilities behind a clear router: `chat`, `deep_solve`, `code_exec`, `quiz`, `research`, later `visualize`.

### Frontend

- Keep the current React UI for the next step.
- Improve it into a learning workspace rather than a debug console.
- Defer a Next.js migration until routing, auth, or server-side rendering becomes valuable.

### RAG

Current implementation is simple and local:

- Document formats: Markdown, TXT, PDF first.
- Chunking: basic text chunks now; later token or paragraph-based chunks.
- Embeddings: configurable provider.
- Store: LanceDB.
- Retrieval: top-k semantic search with source metadata.

Current implementation uses JSON metadata plus LanceDB vectors. SQLite is still a reasonable next storage step when product data becomes more relational.

### Storage Model

Minimum entities:

- `sessions`
- `messages`
- `knowledge_bases`
- `documents`
- `chunks`
- `retrieval_hits`
- `spaces`
- `notebook_entries`
- `quiz_questions`
- `quiz_attempts`

## 4. Phase Plan

### Phase 0: Stabilize Runtime Baseline

Goal: make the current app reproducible and easy to run.

- [x] Finalize dependency strategy: git dependencies or local path dependencies, but not a mix that causes version drift.
- [x] Update README with the correct backend port and current launch steps.
- [ ] Add a smoke test for `tutor-web` startup.
- [x] Keep `cargo check --workspace`, `cargo test --workspace`, and `npm run build` green.

#### v0.1 Carryover Checklist

These items come from the earlier Phase 1-5 plans. They should be closed before starting larger product work, otherwise later phases will build on an uneven foundation.

- [x] Wire top-level `Capability::CodeExec` instead of returning `UnsupportedCapability`.
- [x] Decide whether `code_exec` requires approval in CLI, Web, both, or only when configured.
- [ ] Re-enable runtime budget enforcement once `llm-harness-runtime` exposes a safe app-level budget policy for ordinary one-turn harnesses and workflows. Current code keeps session budget configuration but avoids direct `BudgetControlAdapter` wiring because the latest tested hook semantics can hang Chat/Code Exec mock runs.
- [x] Emit real `TutorStream::trace` events from Chat and Deep Solve, especially phase transitions, tool calls, and replan events.
- [x] Confirm WebSocket output semantics: final-only response, chunked text stream, or mixed content/trace/status stream.
- [x] Replace Deep Solve `run_pre_retrieve` stub with either real RAG retrieval or an explicit no-KB branch.
- [x] Update README with accurate backend port, provider setup, dependency strategy, and known v0.1 limitations.
- [x] Add or remove the planned `docs/quickstart-deep-solve.md`; avoid stale file references in plans.
- [ ] Run `cargo clippy --workspace --all-targets --all-features -- -D warnings` and either fix warnings or document accepted warnings.
- [ ] Document how to run ignored real-provider tests and what API keys are required.

Acceptance:

- A clean clone can run backend and frontend with documented commands.
- No local sibling repository is required unless explicitly documented.
- All historical Phase 1-5 carryover items are either complete or explicitly moved into a later product phase.

### Phase 1: Real Knowledge Base

Goal: replace RAG stubs with real document ingestion and retrieval.

- [x] Add `knowledge_bases` and `documents` storage.
- [x] Add document upload endpoint.
- [x] Parse TXT and Markdown.
- [x] Parse PDF text.
- [x] Chunk documents with stable chunk IDs.
- [x] Add embedding provider configuration.
- [x] Store chunk vectors and metadata.
- [x] Replace `RagSearchTool` placeholder with real retrieval.
- [x] Return citations to the UI.

Acceptance:

- User uploads a document and asks a question.
- Answer includes cited source chunks.
- Retrieval can be tested without a real LLM by querying the index directly.

### Phase 2: Session Persistence

Goal: turn chat from ephemeral messages into a resumable learning history.

- [x] Persist sessions and messages.
- [x] Add session list API.
- [x] Add session detail API.
- [x] Add resume session support in Web UI.
- [x] Store selected capability and knowledge base per session.
- [x] Persist trace events or compact task summaries.

Acceptance:

- Refreshing the browser does not lose the conversation.
- User can resume a previous session and continue with context.

### Phase 3: Deep Solve UX

Goal: make long reasoning inspectable and useful.

- [x] Stream phase events: plan, solve step, replan, synthesize.
- [x] Show a structured Trace Panel with phase status.
- [x] Show intermediate step results separately from the final answer.
- [ ] Allow user to stop a running solve.
- [x] Add mock tests for replan and phase event emission.

Acceptance:

- Deep Solve feels like a guided workflow, not a delayed chat response.
- The user can see what stage the system is in.

### Phase 4: Quiz and Answer Judging

Goal: add the first active learning workflow.

- [x] Add `quiz` capability.
- [x] Generate questions from selected knowledge base chunks.
- [x] Support single-choice questions first.
- [x] Add answer judging/scoring flow.
- [x] Save quiz questions and attempts.
- [x] Show explanations and source references.

Acceptance:

- User can generate questions from a document.
- User can answer and receive feedback.
- Wrong answers can be saved for later review.

### Phase 5: Space, Notebook, and Learning Records

Goal: make learning outputs reusable.

- [x] Add default Space.
- [x] Add Notebook entries.
- [x] Save research reports to Notebook as `type = research_report`.
- [x] Move Quiz history/review into Space / Quiz Bank.
- [x] Add Student Profile module.
- [x] Add Markdown-based Memory module with L1 events, L2 summaries, and L3 learner memory.
- [x] Add manual memory consolidation from the Memory module.
- [x] Add `read_memory` tool so Quiz, Research, Chat, and Deep Solve can actively inspect learner memory.
- [ ] Save chat answers to Notebook.
- [ ] Save quiz summaries to Notebook.
- [ ] Save source snippets.
- [ ] Remove the retired Book implementation and stale save-to-Book paths.

Acceptance:

- User can keep important learning records without leaving the chat.
- Future turns can reference saved records.

### Phase 6: Product Polish

Goal: make the app feel like a learning product, not only a developer demo.

- [x] Redesign the UI around chat, knowledge, quiz, books, settings, and trace.
- [x] Redesign Space around Notebook, Quiz Bank, and Student Profile.
- [x] Add clear empty states for major views.
- [x] Add upload progress and indexing status.
- [ ] Add model/provider health checks.
- [x] Add error recovery for missing API keys and failed embeddings.
- [ ] Add export for chat and book entries.

Acceptance:

- A new user can understand the app without reading source docs.
- Common failures produce actionable messages.

## 5. Later Expansion

After the MVP loop works, consider larger DeepTutor-like surfaces:

| Feature | Why Later |
|---|---|
| Deeper Research | Current Research MVP exists; needs robust citations, report store, regeneration, and parallel task orchestration |
| Automatic Memory Consolidation | Start with manual Markdown consolidation first; automatic triggers need trust and good review UX |
| TutorBot | Needs auth, workspace isolation, and channel security |
| Multi-user | Changes storage, permissions, and deployment model |
| Math Animator | Requires media rendering pipeline and visual QA |

## 6. Immediate Next Tasks

Recommended next implementation order:

1. Improve source/citation quality for Research and Quiz.
2. Add source references from Student Profile memory claims back to their evidence.
3. Add chat-to-notebook and quiz-summary-to-notebook save flows.
4. Upgrade RAG chunking from basic character chunks to paragraph/token-aware chunks.
5. Add model/provider health checks in Settings.
6. Add export for chat, Notebook entries, and reports.
7. Add a smoke test for `tutor-web` startup.
8. Decide whether local JSON stores should move to SQLite.

## 7. Risks

| Risk | Mitigation |
|---|---|
| Scope creep from copying DeepTutor wholesale | Keep MVP centered on upload -> ask -> solve -> quiz -> save |
| RAG quality is poor | Start with source-visible retrieval debugging and chunk previews |
| Provider differences break tool calls | Keep mock provider tests and provider-specific integration tests |
| Long tasks feel opaque | Stream trace/status events early |
| Storage model churn | Keep JSON while local MVP is simple; move to SQLite when reports/sessions need relational queries |
| UI becomes a debug console | Design around learner workflows, not internal events |

## 8. Success Metrics

Early qualitative metrics:

- User can upload a course note and get a grounded answer in under 2 minutes.
- User can solve a difficult question with visible intermediate steps.
- User can generate useful quiz questions from their own material.
- User can save and later find an important answer, report, or mistake.

Engineering metrics:

- `cargo test --workspace` passes.
- Frontend build passes.
- RAG retrieval has deterministic tests.
- At least one end-to-end smoke path covers document upload -> retrieval -> answer.
