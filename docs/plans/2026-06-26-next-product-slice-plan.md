# Next Product Slice Plan

> Status: proposed | Date: 2026-06-26 | Scope: implement the next coherent product slice after the initial Space, Notebook, Memory, Quiz Bank, Research, and web-search work.

## 1. Goal

Make `llm-tutor` feel like a learning workspace rather than separate feature pages.

The next slice should connect five product loops:

```text
Conversation -> Notebook -> Space review
Conversation -> Quiz -> Quiz Bank
Research -> Notebook -> optional Book
User activity -> Markdown Memory -> Student Profile
Memory -> Agent personalization
```

The implementation should keep runtime concerns in `llm-harness-runtime` /
`llm-harness-agent` and keep this repo focused on product data, UI, and thin
tool/session adapters.

## 2. Product Decisions

- Space is the main durable workspace.
- Space initially contains Notebook, Quiz Bank, and Student Profile.
- Research reports are Notebook entries, not a separate `ResearchReport` store.
- Books are polished outputs created from Notebook entries when needed.
- Quiz generation stays in chat through the composer mode.
- Quiz Bank only reviews and manages historical quiz records.
- The standalone Quiz page should be removed from primary navigation after Space Quiz Bank reaches parity.
- Student Profile is a visible projection of Markdown Memory plus lightweight stats.
- Learner memory is Markdown-first and user-editable.
- Agents read memory through `read_memory`; memory is not injected into every prompt by default.

## 3. Phase 1: Stabilize Space as the Home for Learning Artifacts

Status: in progress.

Tasks:

- [ ] Keep Space as a tabbed workspace with Notebook, Quiz Bank, and Student Profile.
- [ ] Ensure Notebook list/detail/edit/delete flows are stable.
- [ ] Ensure Research "save" action writes a Notebook entry by default.
- [ ] Ensure Notebook entries can be sent to Books as chapters.
- [ ] Ensure Quiz Bank lists completed quiz records from chat-generated quizzes.
- [ ] Ensure Quiz Bank can show questions, selected answers, correct answers, explanations, and citations.
- [ ] Add basic source/reference display for Student Profile memory claims.
- [ ] Remove standalone Quiz navigation only after Quiz Bank review covers current needs.

Acceptance:

- User can create or save durable content without leaving the normal chat flow.
- User can review Notebook entries and Quiz records from Space.
- User can understand Student Profile as editable learning memory, not hidden analytics.

## 4. Phase 2: Make Quiz Work from Conversation, Notebook, and Knowledge Sources

Status: planned.

Tasks:

- [ ] Allow Quiz mode without requiring a selected knowledge base.
- [ ] Generate quizzes from current conversation context.
- [ ] Generate quizzes from selected Notebook entries later.
- [ ] Use RAG only when a knowledge base is selected or the user asks about uploaded material.
- [ ] Use web search only when the quiz source needs external/current facts.
- [ ] Validate answer/explanation consistency before rendering a quiz.
- [ ] Improve citation mapping so each question cites the correct source chunk or web source.
- [ ] Record quiz generation, answers, scores, and weak points into L1 memory.
- [ ] Add tests that do not depend on a real LLM for quiz source handling and persistence.

Acceptance:

- User can ask "make a quiz from what we just discussed" and get a quiz.
- User can ask for a source-grounded quiz and see citations only where real sources were used.
- Quiz answers, explanations, and citations do not contradict each other.

## 5. Phase 3: Finish Markdown Memory and Student Profile

Status: in progress.

Tasks:

- [ ] Keep L1 raw events for chat, quiz, notebook, and research.
- [ ] Keep L3 Markdown files as the first agent-readable memory surface.
- [ ] Add manual consolidation preview and apply flow.
- [ ] Show recent events that will be used before consolidation.
- [ ] Render Student Profile from `L3/profile.md`, `L3/recent.md`, and `L3/teaching_strategy.md`.
- [ ] Add source references from memory claims back to quiz, notebook, research, or chat evidence where possible.
- [ ] Keep profile editing as Markdown editing.
- [ ] Defer automatic consolidation until manual consolidation is reliable.

Acceptance:

- User can inspect and edit what the agent may remember.
- Agent personalization can use memory without treating it as factual source evidence.
- Memory changes can be traced back to product events where references exist.

## 6. Phase 4: Improve Research as a Report Workflow

Status: planned.

Tasks:

- [ ] Make Research mode stricter about using search/fetch for external facts.
- [ ] Show search/fetch failures as clear reasons.
- [ ] Render Research result as a structured report component.
- [ ] Attach a dedicated source list to each report.
- [ ] Save Research reports to Notebook as `type = research_report`.
- [ ] Preserve query, sources, session id, and tool trace ids in Notebook metadata.
- [ ] Support "Generate Quiz from this report" after Notebook source selection exists.
- [ ] Support "Send to Book" from the Notebook report detail.

Acceptance:

- Research output is clearly separated from ordinary chat.
- Reports are durable Notebook entries with sources.
- Books can be created from cleaned-up Notebook material.

## 7. Phase 5: Harden Web Search and External Evidence

Status: planned.

Tasks:

- [ ] Prefer provider APIs over fragile scraping when configured.
- [ ] Keep DuckDuckGo as a best-effort fallback, not the only reliable path.
- [ ] Support configured paid/free providers from Settings.
- [ ] Deduplicate search results.
- [ ] Add simple source quality scoring.
- [ ] Make search failures visible to the user and agent.
- [ ] Ensure chat/research prompts require search for current or external fact-collection tasks.
- [ ] Ensure the final answer does not claim searched evidence unless `web_search` or `web_fetch` actually succeeded.

Acceptance:

- User can configure a reliable search provider.
- Agent can distinguish "I searched and found" from "I could not search".
- Search sources are traceable in final answers and reports.

## 8. Phase 6: Persistence and Session UX

Status: planned.

Tasks:

- [ ] Persist message, status, trace, and compact summary data wherever runtime support exists.
- [ ] Keep product-to-runtime session mappings thin.
- [ ] Surface context usage from runtime/provider usage where available.
- [ ] Trigger automatic compaction using runtime capabilities when context approaches the configured window.
- [ ] Avoid mode switching resetting the current runtime session.
- [ ] Keep trace collapsed by default and show product-relevant progress in the chat surface.

Acceptance:

- Reloading does not lose important session context.
- Status and trace history are understandable but not noisy.
- Context capacity is visible enough for the user to trust long conversations.

## 9. Phase 7: Tests and Quality Gate

Status: planned.

Tasks:

- [ ] Add Notebook store/API tests.
- [ ] Add Quiz Bank list/detail tests.
- [ ] Add Memory parser/consolidation tests.
- [ ] Add `read_memory` tool tests.
- [ ] Add non-real-LLM RAG retrieval tests.
- [ ] Add non-real-LLM Quiz source tests.
- [ ] Add Research mock tests for search/fetch/report metadata.
- [ ] Keep `npm run build` passing.
- [ ] Keep `cargo test -p tutor-web --lib`, `cargo test -p tutor-agent`, and `cargo test -p tutor-tools` passing.

Acceptance:

- Core product stores and adapters are covered by boundary tests.
- Agent workflows have mock tests where real providers are not required.
- UI builds after each slice.

## 10. Recommended Order

1. Finish Space parity: Notebook, Quiz Bank, Student Profile references.
2. Make Quiz work from conversation context without requiring a knowledge base.
3. Harden Quiz answer/explanation/citation consistency.
4. Finish manual memory consolidation and reference display.
5. Make Research save reports into Notebook with structured metadata.
6. Add Notebook-to-Book and Notebook-to-Quiz follow-up flows.
7. Harden web search provider behavior and failure reporting.
8. Improve persistence for trace/status/context summaries.

## 11. Deferred Work

- Multi-space management.
- Automatic memory consolidation.
- Structured StudentProfile cache.
- Notebook entries automatically indexed into RAG.
- Research report versioning.
- Long-running parallel deep research.
- Spaced repetition scheduling.
- Hosted multi-user auth and permissions.
