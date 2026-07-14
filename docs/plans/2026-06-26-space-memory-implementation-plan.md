# Space and Memory Implementation Plan

> Status: proposed | Date: 2026-06-26 | Scope: implement the next product slice after Research, focused on Space, Notebook, Quiz Bank, Markdown Memory, and memory-aware Quiz generation.

## 1. Goal

Turn generated learning content into durable learning assets, then make those
assets available to agents through an explicit memory tool.

Target product loop:

```text
Chat / Research / Quiz produces learning material
  -> user saves or answers it
  -> Space organizes it
  -> Memory consolidates it into readable Markdown
  -> Agent reads memory when planning future teaching or quizzes
```

Do not build a hidden student-profile database in this phase. Student Profile
should be a visible projection of Markdown memory plus lightweight stats.

## 2. Principles

- Keep agent runtime mechanics in `llm-harness-runtime` / `llm-harness-agent`
  where available.
- Keep `llm-tutor` responsible for product data: Space, Notebook entries, Quiz
  records, Memory files, and UI.
- Use Markdown as the first durable representation for learner memory.
- Prefer manual memory consolidation first. Automatic consolidation can come
  after users can inspect and correct memory.
- Do not inject all memory into every prompt. Give agents a `read_memory` tool
  and clear instructions for when to use it.
- Memory should personalize teaching behavior; it is not an external factual
  source.

## 3. Phase 1: Space Shell

Status: completed for the first UI slice on 2026-06-26.

### Scope

Replace the placeholder Space surface with a usable tabbed workspace.

### Tasks

- [x] Add a default Space shell.
- [x] Add Space page shell.
- [x] Add tabs: Notebook, Quiz Bank, Student Profile.
- [x] Keep UI consistent with the current blue/white/gray style.
- [x] Keep standalone Quiz navigation until Quiz Bank reaches parity.

### Acceptance

- [x] User can open Space from the sidebar.
- [x] Space shows the three planned modules.
- [x] Empty states explain what will appear there without becoming a landing page.

## 4. Phase 2: NotebookEntry Store

Status: completed for the first durable Notebook slice on 2026-06-26.

### Scope

Add the durable record layer needed by Research reports and later chat/quiz saves.

### Data Model

```ts
NotebookEntry {
  id: string
  spaceId: string
  type:
    | 'note'
    | 'research_report'
    | 'chat_answer'
    | 'source_snippet'
    | 'quiz_summary'
    | 'deep_solve_result'
  title: string
  markdown: string
  metadata?: Record<string, unknown>
  sourceSessionId?: string
  sourceMessageId?: string
  createdAt: string
  updatedAt: string
}
```

### Tasks

- [x] Add local NotebookEntry store.
- [x] Add list/create/read/update/delete APIs.
- [x] Add Notebook list/detail UI.
- [x] Add manual note creation.
- [x] Save Research reports as `type = research_report`.
- [x] Change Research primary save action from "save to book" to "save to notebook".
- [x] Add secondary "send to book" action from a Notebook entry.
- [x] Add edit UI for existing Notebook entries.

### Acceptance

- [x] User can save a Research report into Notebook.
- [x] User can reopen Space and find saved notes.
- [x] User can reopen Space and find the saved report.
- [x] User can edit Notebook entries.
- [x] User can delete Notebook entries.
- [x] Books remain available as polished outputs, not raw report storage.

## 5. Phase 3: Quiz Bank Migration

Status: completed for the first Quiz Bank migration slice on 2026-06-26. Quiz
Bank review UI exists in Space, quiz generation remains in the chat composer,
and the standalone Quiz navigation entry has been removed.

### Scope

Move quiz review into Space while keeping quiz generation in chat.

### Tasks

- [x] Add Quiz Bank tab.
- [x] List historical quiz sessions.
- [x] Show score, answered count, and created time.
- [x] Show questions, selected answers, correct answers, explanations, and citations.
- [x] Support missed-question review.
- [x] Keep composer Quiz mode as the generation entry.
- [x] Remove standalone Quiz nav only after Quiz Bank covers current review needs.

### Acceptance

- [x] User can generate a Quiz in chat.
- [x] User can answer it in chat.
- [x] The resulting record appears in Space / Quiz Bank.
- [x] User can review missed questions without returning to the original chat turn.

## 6. Phase 4: Markdown Memory Module

Status: completed for the first Markdown Memory slice on 2026-06-26. Local
Markdown store, skeleton creation, L2/L3 viewer/editor, marker/reference parser
helpers, L1 event recording, manual consolidation preview/apply, and memory
assist actions have landed. L1 remains a workspace event ledger and is not
visualized as a primary memory layer.

### Scope

Add visible, editable long-term learner memory.

### Storage Layout

```text
memory/
  L1/
    chat_events.jsonl
    quiz_events.jsonl
    notebook_events.jsonl
    research_events.jsonl

  L2/
    chat.md
    quiz.md
    notebook.md
    research.md

  L3/
    recent.md
    profile.md
    scope.md
    preferences.md
    teaching_strategy.md
```

### Markdown Entry Format

```md
# Student profile

## Weaknesses

- Often confuses lithography model and photoresist model. [^1] <!--m_01ABC-->

---

[^1]: quiz:session_123:q_2
```

### Tasks

- [x] Add Memory page/module.
- [x] Create memory directory skeleton on startup or first use.
- [x] Add Markdown file viewer.
- [x] Add Markdown file editor.
- [x] Add stable entry id parser/serializer for memory bullets.
- [x] Add source reference parser/serializer.
- [x] Record L1 events for chat, quiz, notebook, and research.
- [x] Add manual consolidation action.
- [x] Show consolidation preview before applying changes.
- [x] Keep L1 out of the main visualization while retaining it as workspace event storage.
- [x] Add L2/L3 memory workbench with update, check, and dedupe assist actions.

### Acceptance

- [x] User can inspect memory files.
- [x] User can edit `profile.md`, `preferences.md`, and `teaching_strategy.md`.
- [x] User can manually consolidate recent activity into Markdown memory.
- [x] Memory files support source references in Markdown and the profile UI can surface those references.
- [x] User can ask the first-slice Memory workbench to prepare update drafts,
  inspect memory quality, and remove duplicate entries. The draft-oriented
  review is transitional and is superseded by the target design below.

### Target Redesign: Evidence Exploration and Diff Review

Status: planned on 2026-07-14. The approved target contract is defined in
`docs/specs/2026-06-26-memory-consolidation-design.md` and requirements
`REQ-727` through `REQ-747`.

The next Memory workbench slice replaces prompt-injected evidence batches and
full-document drafts with agent-directed L1 exploration, visible flow progress,
structured change sets, and central diff review.

#### Tasks

- [ ] Give every L1 event a stable event/turn reference instead of reusing a
  session-only reference for multiple events.
- [ ] Preserve complete evidence through a bounded snapshot or durable source
  pointer for Chat, Quiz, Notebook, Knowledge, and Research events.
- [ ] Add runtime-native, read-only tools to list/search L1 events, read an
  event, read surrounding context, and resolve the original artifact.
- [ ] Start discovery in the target surface while allowing the agent to expand
  explicitly to other L1 surfaces.
- [ ] Emit product-level flow events for discovery, reads, analysis, proposal,
  validation, review, apply, completion, failure, and cancellation.
- [ ] Replace full `proposed_markdown` and report-oriented output with a
  versioned `MemoryChangeSet` containing findings and evidence-bound changes.
- [ ] Keep the right workbench limited to controls, flow status, counts, errors,
  and a compact completion summary.
- [ ] Add Read/Edit/Review modes to the central document surface.
- [ ] Render insert/replace/delete operations as deterministic inline diff,
  with per-change reasons and navigable source chips.
- [ ] Support accept/reject per change and accept/reject all.
- [ ] Validate read-set refs, sections, anchors, text limits, and base revision
  before apply.
- [ ] Apply accepted changes atomically, record history, and support undo.
- [ ] Add boundary and UI tests for pagination, source expansion, duplicate or
  unread refs, stale revisions, partial acceptance, atomic apply, and undo.

#### Acceptance

- [ ] The Memory agent can address all L1 evidence without receiving the whole
  ledger in its initial prompt.
- [ ] Every evidence read is visible in the run flow and every proposed change
  cites evidence read during that run.
- [ ] The right workbench never uses a full Markdown draft as its normal result.
- [ ] The center document area shows a reviewable diff and source-backed reason
  for each change.
- [ ] No persistent document changes occur before explicit user confirmation.
- [ ] Selected changes apply atomically and can be undone.

## 7. Phase 5: `read_memory` Tool

Status: completed for harness-backed modes on 2026-06-26. `read_memory` landed
in `tutor-tools` and is mounted for Chat, Research, and Deep Solve. Quiz
currently remains a structured generation API, so it uses L3 memory as
personalization context in Phase 6 rather than a real model-decided
`read_memory` tool call. The runtime gap is recorded in
`docs/framework-feedback.md`.

### Scope

Expose learner memory to agents as an explicit tool.

### Tool Contract

```ts
read_memory({
  scope?: 'recent' | 'profile' | 'scope' | 'preferences' | 'teaching_strategy' | 'all',
  query?: string
}) -> {
  markdown: string
  files: string[]
}
```

MVP can ignore `query` and return selected L3 files. Later, `query` can filter
sections or use retrieval.

### Tasks

- [x] Add `read_memory` tool in `tutor-tools`.
- [x] Return L3 Markdown content.
- [x] Mount `read_memory` when memory exists.
- [x] Make the tool available to Chat, Research, and Deep Solve.
- [x] Document the remaining Quiz/runtime structured-tool gap instead of building a parallel orchestration loop.
- [x] Update prompts to instruct agents when to call it.
- [x] Ensure memory is not included in prompts by default.
- [x] Add tests for empty memory and populated memory.

### Acceptance

- [x] Agent can call `read_memory`.
- [x] Empty memory returns a clear "no memory yet" result.
- [x] Populated memory returns readable Markdown.
- [x] Quiz generation can use memory before planning when personalization is relevant, via the structured Quiz API path described in Phase 6.

## 8. Phase 6: Memory-Aware Quiz

Status: completed for the first memory-aware Quiz slice on 2026-06-26. Quiz can
now be generated from current conversation source text or Notebook entries
without requiring a Knowledge Base, writes generation/answer/finish events into
L1 memory, and reads L3 memory for personalization only when the quiz request
indicates review, practice, weak-point, follow-up, or personalized intent. Quiz
still uses a structured API instead of a full runtime tool-call loop.

### Scope

Use memory to improve quiz planning and reduce repeated or mismatched questions.

### Behavior

Quiz should consider:

- prior quiz history,
- weak concepts,
- common misconceptions,
- teaching strategy,
- preferred difficulty or explanation style,
- current source material.

### Tasks

- [x] Add Quiz prompt guidance for `read_memory`.
- [x] In planning, prefer reading memory when user asks for review, practice,
  personalized quiz, or follow-up quizzes.
- [x] Avoid requiring a Knowledge Base for Quiz.
- [x] Support Quiz from conversation context.
- [x] Keep Quiz generation in Chat; Notebook entries remain source material, not a separate generation surface.
- [x] Write Quiz events into L1 memory after generation and answer submission.

### Acceptance

- [x] User can ask for a quiz from current conversation without selecting a KB.
- [x] User can ask in Chat to generate a quiz from current conversation material.
- [x] If memory says the learner is weak on a concept, Quiz planning can target it when the request asks for personalization or review and an LLM config is available.
- [x] Quiz generation still works when memory is empty.
- [x] Quiz does not hallucinate memory; it uses L3 memory only as personalization context and never as a citation source.

## 9. Phase 7: Student Profile Projection

Status: completed for the first Student Profile slice on 2026-06-26. Space /
Student Profile now renders and edits `L3/profile.md`, `L3/recent.md`, and
`L3/teaching_strategy.md` alongside Quiz stats, and shows memory source refs
when Markdown footnotes are present.

### Scope

Render Student Profile from Memory and quiz stats.

### Tasks

- [x] Read `L3/profile.md`.
- [x] Read `L3/recent.md`.
- [x] Read `L3/teaching_strategy.md`.
- [x] Show these as editable/profile sections.
- [x] Add supplemental stats from Quiz Bank.
- [x] Link profile claims back to memory references where available.

### Acceptance

- [x] Student Profile is understandable without exposing raw JSON.
- [x] User can correct the profile by editing Markdown memory.
- [x] Profile does not diverge from Memory.
- [x] Profile shows source-reference targets when memory Markdown contains refs.

## 10. Testing

- [x] NotebookEntry store tests.
- [x] Notebook APIs tests.
- [x] Quiz Bank list/detail tests.
- [x] Memory parser/serializer tests.
- [x] Manual consolidation mock tests.
- [x] `read_memory` tool tests.
- [x] Quiz route test for memory-aware planning intent.
- [x] Frontend build.
- [x] `tutor-web` library tests.

## 11. Recommended Implementation Order

1. Space shell.
2. NotebookEntry store and APIs.
3. Notebook UI and Research save-to-notebook.
4. Quiz Bank migration.
5. Memory directory, Markdown viewer/editor, and parser.
6. L1 event recording.
7. Manual consolidation.
8. `read_memory` tool.
9. Memory-aware Quiz prompt and tests.
10. Student Profile projection.

## 12. Deferred Work

- Automatic memory consolidation.
- Multi-space memory isolation.
- Vector retrieval over memory entries.
- Structured StudentProfile cache.
- Full spaced repetition scheduling.
- Hosted/multi-user memory permissions.
