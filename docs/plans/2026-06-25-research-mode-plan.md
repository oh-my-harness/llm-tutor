# Research Mode Plan

> Status: in progress | Date: 2026-06-25 | Last updated: 2026-07-09 | Scope: add a research workflow that searches, reads, cites, produces a report, and can save the report into books.

## 1. Goal

Research mode should turn an open topic into a sourced, reusable research report.

The user should be able to:

- ask for research on a topic from chat,
- see compact progress for planning, searching, reading, and synthesis,
- get a final Markdown report with citations,
- inspect the sources used by the report,
- save the report as a book chapter,
- continue asking questions and, from Chat Quiz mode, ask to generate a Quiz
  based on the saved report or current conversation.

Research is different from normal chat:

- chat is optimized for fast answers,
- research is optimized for evidence gathering and durable output,
- research should use search/read tools when current information or external facts are needed,
- intermediate progress can be visible as its own transient progress surface,
  but must not be merged into the durable final assistant answer,
- the final report is the main user-visible answer.

## 2. First Version Scope

V1 should focus on a reliable research-to-report-to-book loop.

Included:

- `research` capability in the chat mode selector,
- research prompt and tool policy,
- web search through the existing web search tool,
- web page fetch/extract tool,
- report synthesis with citations,
- source list rendering under the report,
- save report to book as a Markdown chapter,
- local durable Notebook entry and optional Book chapter metadata.

Out of scope for V1:

- automatic source quality scoring,
- academic paper search,
- recursive crawling,
- collaborative editing,
- rich block-based book editor.

Long-running and parallel research are no longer out of scope for the product
direction. The first parallel sub-task slice exists, but durable background
execution and session rejoin are tracked separately in
`2026-07-13-background-session-resilience-plan.md`.

## 3. Product Shape

The user chooses `Research` in the composer and sends a topic:

```text
Research Rust crates and services that are suitable for web search in an agent product.
```

The UI renders one structured assistant result:

```text
Research
  Planning
  Searching
  Reading
  Synthesizing

Final Report
  title
  summary
  key findings
  sections
  limitations
  next questions
  sources

Actions
  Save to book
  Ask in Chat to generate Quiz
  Continue research
```

Progress items are small and collapsible. The final report is the only full-size assistant content.

### Research Chat and Workflow Split

Research mode should support both conversational clarification and a detailed
research workflow.

- Research Chat is the default interaction surface. The agent can discuss the
  user's topic, clarify goals, scope, source preferences, output format, depth,
  time range, and whether to use Notebook or Knowledge Base context.
- Detailed Research Workflow is a capability available inside Research mode.
  It should execute structured search, source reading, source selection,
  synthesis, citation checking, and report generation after the research need is
  clear.
- Entering the detailed workflow is not mandatory for every Research message.
  If the request is ambiguous, the agent should continue normal conversation and
  ask focused follow-up questions.
- When the request is clear enough, the agent should propose a brief research
  plan and start the detailed workflow only after the user explicitly asks to
  begin, confirms the plan, or gives an unambiguous instruction to produce the
  report.
- Starting the detailed workflow should use the same product-tool boundary as
  Quiz generation. Research Chat should call a dedicated
  `create_research_report` tool after confirmation; that tool owns the runtime
  workflow execution and returns the report, sources, and report metadata.
  Keyword-based pre-routing may exist only as a temporary migration bridge, not
  as the target architecture.

Target interaction states:

```text
research_chat
  -> clarifying
  -> plan_proposed
  -> workflow_running
  -> report_ready
```

`research_chat`, `clarifying`, and `plan_proposed` should behave like normal
chat: assistant text deltas stream through the final-answer `content` channel,
the UI shows the response as an ordinary streaming assistant bubble, and the
assistant may ask questions or refine the plan.
`workflow_running` should behave like a structured task: progress is shown as
compact progress/trace events, intermediate drafts should not be merged into the
final assistant bubble, and only the completed report should become the durable
final answer.

### Streaming and Message Contract

Research mode needs an explicit split between what the model is saying while it
works and what the product records as the answer.

- `content` is the final-answer channel. Chunks on this channel are assembled
  into one durable assistant message. When a non-chunk `content` event arrives,
  the UI commits that assembled text into the conversation history.
- `progress_content` is the transient workflow-progress channel. TextDelta that
  is useful to show after the detailed Research workflow has started should be
  sent here. Research Chat, clarification, and plan proposal turns must not use
  this channel for ordinary assistant replies; they should keep the same
  streaming behavior as normal Chat. The UI renders `progress_content` in a
  separate transient progress bubble or status surface, clears it when the final
  answer is committed, and does not persist it as the answer body.
- `trace` and `status` are structured operational events for tool calls,
  workflow stages, sources, errors, and metadata. They should stay structured
  and should not be flattened into the final assistant text.

This means Research has two streaming behaviors. Before the detailed workflow
starts, Research behaves like Chat. While the detailed workflow is running, model
narration is allowed to appear as progress, but it is not treated as the same
message as the final report. If a Research turn only produces ordinary chat text
and no workflow/final-answer tool result, that text belongs to the final-answer
channel so greetings and clarifying questions stream and are stored like normal
conversation.

### Product Tool Boundary

Research should align with Quiz mode:

```text
Research Chat agent
  -> ordinary streaming chat for clarification
  -> propose_research_plan tool when a plan should be confirmed
  -> create_research_report tool after explicit confirmation
      -> runtime WorkflowEngine
      -> report markdown + sources + metadata
  -> assistant final response / Research report card
```

Target responsibilities:

- `propose_research_plan` remains a lightweight planning tool. It does not
  search, read sources, or create a report.
- `create_research_report` is the only normal entry point for the detailed
  Research workflow from Chat. The outer agent calls it after the user confirms
  a plan or gives an unambiguous report-generation request.
- `create_research_report` runs the runtime `WorkflowEngine`, bridges workflow
  step events to trace/status/progress events, persists the completed report to
  the runtime session as `AssistantMessageKind::FinalAnswer`, and returns
  structured report metadata in its tool result.
- The WebSocket/UI should treat the `create_research_report` tool call as the
  boundary where the turn changes from ordinary Research Chat streaming to
  workflow progress rendering.
- The existing keyword/confirmation pre-router should be removed once the tool
  path is in place. If a compatibility bridge remains temporarily, it must
  delegate to the same product-tool/workflow path and must not change TextDelta
  routing for ordinary Research Chat turns.

## 4. Layering

### Runtime / Agent Layer

Use runtime and agent framework capabilities first:

- runtime sessions,
- model provider calls,
- tool orchestration,
- trace/status events,
- compaction,
- durable session entries where available.

Do not build a parallel agent loop in `llm-tutor`.

Runtime should not own Tutor-specific product concepts such as books, chapters, or research reports.

Potential framework feedback:

- first-class persisted trace events,
- clearer progress-vs-final-answer conventions,
- easier manual tool policy hooks for capability-specific behavior.

### `tutor-agent` Layer

Owns the research chat prompt, workflow definition, and report-generation
behavior behind the product-tool boundary.

Expected behavior:

- create a short research plan,
- keep ordinary Research Chat conversational until the user confirms the
  workflow,
- expose the runtime Research workflow through `create_research_report` rather
  than through capability-level keyword pre-routing,
- inside the workflow, decide search queries, call `web_search`, call
  `web_fetch` for promising sources, optionally call `rag_search` when a
  knowledge base is associated, synthesize a report grounded in collected
  sources, and emit structured research trace events.

Research mode should strongly prefer tools for external facts. If search fails, the final answer must say what failed and what is based on model prior knowledge.

### `tutor-tools` Layer

Owns reusable tools:

- `web_search(query, top_k)`,
- `web_fetch(url)`,
- later: `web_search_news`, `scholar_search`, `pdf_read`.

Search provider configuration stays in product settings. Tool implementations should be provider-neutral.

### `tutor-web` Layer

Owns product APIs and persistence:

- `propose_research_plan` product tool,
- `create_research_report` product tool,
- notebook entry store for research reports,
- book store,
- save report endpoint,
- save report to book endpoint,
- session restore mapping for report messages,
- streaming research trace events over WebSocket.

### `web-ui` Layer

Owns product experience:

- mode selector entry,
- structured research message rendering,
- source list rendering,
- save-to-book action,
- book/chapter browsing,
- progress display.

## 5. Data Model

Start with JSON-backed local stores. Move to SQLite only when schema evolution becomes painful.

```ts
NotebookEntry {
  id: string
  spaceId: string
  type: 'research_report'
  sessionId: string
  title: string
  query: string
  markdown: string
  summary: string
  sources: ResearchSource[]
  createdAt: string
  updatedAt: string
}
```

```ts
ResearchSource {
  id: string
  title: string
  url: string
  snippet?: string
  extractedText?: string
  accessedAt: string
}
```

```ts
Book {
  id: string
  title: string
  description?: string
  chapters: BookChapter[]
  createdAt: string
  updatedAt: string
}
```

```ts
BookChapter {
  id: string
  title: string
  markdown: string
  sourceReportId?: string
  sourceSessionId?: string
  createdAt: string
  updatedAt: string
}
```

## 6. Report Format

The final report should be Markdown in V1:

```md
# Title

## Summary

## Key Findings

## Analysis

## Limitations

## Follow-up Questions

## Sources
```

Citation requirements:

- factual claims from web sources should cite a source,
- citations should reference the source list index,
- sources must include URL and access time,
- if the model uses prior knowledge, the report should mark it as uncited background.

## 7. Event Schema

Research mode should emit structured events that the UI can attach to one research result.

| Event | Purpose |
|---|---|
| `research_stage_start` | A stage begins |
| `research_stage_done` | A stage completes |
| `research_plan` | Research questions and search plan |
| `research_search` | Search query and result count |
| `research_read` | URL read/extract result |
| `research_source_selected` | Source chosen for final report |
| `research_report_done` | Report metadata |

Minimum event shape:

```ts
ResearchEvent {
  kind: string
  capability: 'research'
  reportId?: string
  stage?: 'plan' | 'search' | 'read' | 'synthesize' | 'save'
  title?: string
  summary?: string
  payload?: Record<string, unknown>
}
```

## 8. Tool Policy

Research mode should use stricter tool rules than chat.

Use web tools when:

- user asks for latest, current, recent, price, version, availability, news, law, product, public figure, or external facts,
- user asks to research, investigate, compare, survey, collect sources, or write a report,
- the topic depends on information outside the current conversation or selected knowledge base.

Use RAG when:

- a knowledge base is selected,
- the user asks about uploaded material,
- the research should combine internal material with web sources.

Do not cite sources that were not actually searched or fetched.

## 9. Book Integration

Books are the durable organization layer.

Research reports can be saved as:

- a new book,
- a new chapter in an existing book,
- a replacement for an existing chapter in a later version.

V1 action:

```text
Save to book -> choose existing book or create book -> create chapter from report Markdown
```

The report remains a Notebook research entry even after saving. A future book chapter should store `sourceNotebookEntryId` so the user can trace it back.

## 10. Implementation Plan

### Phase 1: Research Capability Skeleton

- [x] Add `research` to capability types.
- [x] Add Research option to composer mode selector.
- [x] Add capability label and session creation support.
- [x] Add basic research prompt in `tutor-agent`.
- [x] Render research status as attached progress, not standalone assistant text.

### Phase 2: Search and Read Tools

- [x] Stabilize `web_search` provider configuration.
- [x] Add `web_fetch` for page retrieval and text extraction.
- [x] Emit trace events for search queries and read results.
- [x] Show source snippets in trace/debug view.

### Phase 3: Report Result

- [x] Add `ResearchReport` UI message component.
- [x] Parse/attach report sources from trace or structured metadata.
- [x] Render final report Markdown.
- [x] Add a dedicated source list attached to the report metadata.
- [x] Add source citation display beyond generic message citations.

### Phase 4: Persistence

- [x] Add Notebook entry store.
- [x] Add create/read Notebook entry APIs.
- [x] Persist research report metadata in `NotebookEntry(type = research_report)`.
- [x] Restore research reports from Notebook entries and session links.

### Phase 5: Book Save

- [x] Add minimal book store.
- [x] Add create book/list books APIs.
- [x] Add save report as chapter API.
- [x] Add Save to book UI action.
- [x] Add basic Book page chapter viewer.

### Phase 6: Research Chat Before Workflow

- [x] Split Research behavior into conversational planning and detailed workflow
  execution.
- [x] Update the Research prompt so the agent does not automatically search or
  write a report when the user's goal is underspecified.
- [x] Add a structured research-plan proposal surface or tool result that
  captures topic, scope, source preferences, output format, depth, time range,
  Notebook/Knowledge Base usage, and estimated workflow steps.
- [x] Add UI affordance for confirming or revising the proposed research plan.
- [x] Keep Research clarification and plan proposal conversational, while
  routing intermediate progress text through `progress_content` instead of
  merging it into the final assistant answer; durable assistant bubbles should
  come from the final-answer channel.
- [x] Add tests where ambiguous Research requests produce a clarification
  question instead of calling `web_search`.
- [x] Add tests where an explicit "start research" or confirmed plan enters the
  detailed workflow path.
- [x] Move detailed workflow execution from the prompt-driven harness path into
  the runtime `WorkflowEngine`.
- [x] Replace capability-level keyword/confirmation pre-routing with a
  Quiz-aligned `create_research_report` product tool boundary.
- [x] Keep ordinary Research Chat TextDelta streaming identical to Chat until
  the `create_research_report` tool call starts.

### Phase 7: Detailed Research Workflow

- [x] Model the detailed research run as a runtime workflow using the available
  runtime workflow APIs, instead of relying on a single prompt-driven harness
  turn.
- [x] Define workflow steps for scope confirmation, search query generation,
  search, source selection, source reading, synthesis, citation checking, and
  report publishing.
- [x] Preserve runtime ownership of provider calls, tool orchestration, trace,
  compaction, and session history; keep product code limited to plan/report
  schemas, product persistence, and UI event mapping.
- [x] Add bounded repair behavior for insufficient sources, failed fetches, or
  citation mismatches.
- [x] Ensure the completed report is persisted as a durable
  `AssistantMessageKind::FinalAnswer` and as a `NotebookEntry(type =
  research_report)` when saved.
- [x] Add non-real-LLM workflow tests for search/fetch/report metadata and
  citation verification.
- [x] Return completed report metadata through `create_research_report`
  `tool_result` so the UI can attach a structured Research report card using the
  same pattern as Quiz cards.
- [x] Bridge workflow step progress from `create_research_report` to
  trace/status/progress events without converting intermediate drafts into
  assistant answer bubbles.

### Phase 8: Report Quality and Persistence Hardening

- [x] Ensure reloading the session preserves the report, source list, citations,
  and research-plan metadata.
- [x] Improve source quality scoring.
- [x] Add report regeneration/versioning.

### Phase 9: Follow-up Work

- [x] Support Chat Quiz follow-up prompts that use a saved report as source material.
- [x] Add PDF/webpage source ingestion into knowledge base.
- [x] Add longer-running deep research with parallel sub tasks.

### Phase 10: Background Run and Rejoin Hardening

- [ ] Persist a stable run identity for each `create_research_report` workflow
  so the UI can leave and later rejoin the same run.
- [ ] Restore current Research stage and completed report attachment when the
  user returns to the originating session.
- [ ] Ensure reconnecting to a session subscribes to the existing run instead
  of starting a duplicate Research workflow.
- [ ] Preserve final report metadata and source list if the report completes
  while the user is viewing another session.
- [ ] Add QA that starts a long Research run, switches sessions, returns, and
  verifies progress or completion state.

## 11. Acceptance Criteria

V1 is complete when:

- a user can select Research mode in chat,
- an ambiguous research request can stay in normal chat and produce focused
  clarification questions instead of immediately starting web search,
- a clear request or confirmed research plan can explicitly start the detailed
  research workflow through the `create_research_report` product tool,
- a detailed research workflow triggers web search for external information,
- the final answer is a report with a visible source list,
- ordinary Research Chat before the `create_research_report` tool call streams
  like normal Chat,
- search/read/model progress during the detailed workflow is visible as
  transient progress or structured trace, but is not merged into the durable
  final assistant bubble,
- leaving and returning to the session during a long Research run restores the
  current run state or final report without starting a duplicate workflow,
- the report can be saved as a book chapter,
- reloading the session preserves the report and sources,
- report generation can fail with a clear reason when search or fetch fails.
