# Space Workspace Plan

> Status: implemented with Book-removal follow-up | Date: 2026-06-26 | Last updated: 2026-07-14 | Scope: define Space as the project-level learning workspace that contains Notebook, Quiz Bank, and Student Profile.

## 1. Core Positioning

Space is the project-level container for a learning or research subject.

Space is not the same thing as Notebook. Notebook is one module inside Space.

The product split should be:

```text
Chat Session
  Generate answers, solve steps, research reports, and quizzes.

Space
  Organize durable learning artifacts and learning state.

Knowledge Base
  Store original source materials and retrieval indexes.
```

Short version:

```text
Chat generates.
Space organizes.
Knowledge Base grounds.
```

Chat may reference Space content explicitly with `@` mentions. This does not
turn Space into a generation surface. It makes Space artifacts addressable from
the normal conversation flow:

```text
User @mentions a Notebook entry or Quiz item -> Agent reads it through a product tool -> Agent answers, quizzes, or proposes an edit in Chat
```

For larger artifacts, Chat should pass structured references first and let the
agent call a product tool such as `read_space_item`. This keeps prompts small,
preserves traceability, and avoids silently injecting stale or irrelevant
content.

## 2. First Space Modules

For the next product iteration, Space should contain three tabs:

```text
Space
  Notebook
  Quiz Bank
  Student Profile
```

### Notebook

Notebook stores flexible learning records.

Notebook entries can include:

- ordinary notes,
- research reports,
- chat answer excerpts,
- source snippets,
- quiz summaries,
- Deep Solve results.

Research reports are treated as a kind of note:

```ts
NotebookEntry {
  type: 'research_report'
}
```

This avoids introducing a separate `ResearchReportStore` too early. If research later needs versions, source graphs, sub-tasks, or regeneration history, `research_report` entries can be migrated into a first-class store.

Notebook entries are also editable Space artifacts. Agent-assisted editing
should start from Chat:

```text
User @mentions a Notebook entry and asks for a change -> Agent proposes Markdown/diff -> User confirms -> Notebook entry is updated
```

The Notebook page can keep manual editing controls, but it should not become a
separate agent chat surface in the MVP.

### Quiz Bank

Quiz Bank shows historical quizzes and practice records.

Quiz Bank should not be the primary generation surface.

Quiz generation stays in conversation, but Quiz should be treated as an
enabled agent capability rather than a hard "send means generate immediately"
mode.

```text
User discusses quiz goals in chat
  -> Agent clarifies scope, source material, difficulty, and question style
  -> User asks to generate or confirms the plan
  -> Agent calls create_quiz
  -> Quiz generator creates candidate questions
  -> Deterministic validation checks schema and citation shape
  -> Quiz verifier reviews answer/support/explanation consistency against sources
  -> Chat renders an interactive Quiz card
  -> User answers in chat
  -> Quiz record appears in Space / Quiz Bank
```

Notebook and Research report detail pages should not expose independent Quiz
generation buttons. Saved notes and reports are durable material; quiz
generation remains a Chat action so the active model, source selection,
attachments, references, learner memory, and conversation context are explicit.

The composer may still expose a Quiz-oriented capability selector, but selecting
it should mean "the agent may plan and create quizzes" rather than "every user
message immediately posts to `/api/quizzes`". This lets the user refine the
assessment before committing to generated questions.

Quiz planning should distinguish:

- instruction: the latest user request, such as "make these harder" or "focus on misconceptions",
- source material: selected knowledge base chunks, attached files, `@` Space
  references, Notebook material, or prior conversation content,
- personalization context: learner memory used only to choose focus,
  difficulty, tags, and explanation style.

Quiz quality should be guarded by a verifier stage inside the same product
workflow. This verifier is a controlled reviewer agent, not a second free-form
chat agent. It receives the candidate question JSON plus the cited source chunks
and returns structured review data. It should check:

- whether the correct answer is directly supported by the cited source chunks,
- whether the explanation agrees with the selected correct answer,
- whether the cited chunks are actually evidential rather than merely topical,
- whether the supporting quote is present in a cited chunk,
- whether any distractor is equally or more correct than the intended answer.

The verifier must not add external facts, broaden the source set, or rewrite the
quiz freely. Failed questions should be repaired and re-verified once where
practical, otherwise discarded before the Quiz is saved.

Quiz Bank responsibilities:

- list historical quizzes,
- show scores,
- show missed questions,
- show explanations and citations,
- support re-practice/review,
- filter by source later.

Quiz records and individual questions should be mentionable from Chat. This
supports follow-up explanation, targeted re-practice, and generating related
questions from a known mistake without re-opening a separate Quiz generation
page.

### Student Profile

Student Profile summarizes the learner's state inside the Space.

It should start as a readable projection of the Memory system, not as a
separate hidden profile database.

The durable source of truth should be Markdown memory documents that the user
can inspect and edit. Student Profile can render the relevant parts of those
documents and supplement them with lightweight stats.

Initial profile can be derived from:

- quiz scores,
- missed concepts,
- recent activity,
- saved notebook topics,
- repeated questions,
- knowledge base coverage.

The profile should help answer:

- What has the student studied?
- What do they understand well?
- What are weak points?
- What should they do next?

For agent usage, the important path is not the UI card. Quiz, Research, Chat,
and Deep Solve agents should get access to a `read_memory` tool and decide
when to read the learner profile before planning or generating.

## 3. Markdown Memory System

The learning memory design should follow a simple three-layer model inspired by
DeepTutor, but remain local and product-owned in this repo.

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
    knowledge.md

  L3/
    recent.md
    profile.md
    scope.md
    preferences.md
    teaching_strategy.md
```

### L1: Raw Events

L1 stores append-only evidence. It is not meant to be read directly by the
agent in normal turns.

Examples:

- user asked a question,
- agent produced a quiz,
- user answered a quiz question,
- a research report was saved,
- a notebook entry was created or edited.

### L2: Surface Summaries

L2 stores Markdown summaries per product surface:

- `chat.md`: recurring questions, unresolved topics, notable context.
- `quiz.md`: strengths, weaknesses, wrong-answer patterns, repeated concepts.
- `notebook.md`: organization habits, recurring note and report themes,
  preferred note/report formats, and unresolved questions.
- `knowledge.md`: document interests, frequent queries, and knowledge gaps.

Research remains available as L1 evidence, including activity that has not yet
been saved, but does not own a separate L2 document. Report bodies and external
findings remain Notebook artifacts rather than learner-memory copies.

### L3: Cross-Surface Memory

L3 is the primary memory that agents should read:

- `recent.md`: short recent learning summary.
- `profile.md`: learner state, strengths, weaknesses, misconceptions.
- `scope.md`: current subjects, projects, knowledge boundaries.
- `preferences.md`: explicit user preferences.
- `teaching_strategy.md`: how the tutor should adapt explanations and quizzes.

`teaching_strategy.md` is separate because it directly affects agent behavior.
For example, quiz generation may prioritize concept distinction questions or
avoid overly difficult applied questions when evidence suggests that is useful.

### Markdown Entry Format

Memory entries should be readable Markdown bullets with stable hidden ids and
source refs:

```md
# Student profile

## Weaknesses

- Often confuses lithography model and photoresist model. [^1] <!--m_01ABC-->

---

[^1]: quiz:session_123:q_2
```

This gives us:

- human-readable memory,
- manual editing,
- stable entry ids for edit/delete,
- source traceability,
- enough structure for future consolidation.

### Consolidation

Memory consolidation should live in the Memory module.

MVP behavior:

- User manually triggers consolidation from Memory.
- The UI shows which L1/L2 sources will be used.
- The result is written to Markdown files.
- The user can inspect and edit the Markdown.

Later behavior:

- Suggest consolidation after N turns.
- Suggest consolidation after quiz completion.
- Suggest consolidation after saving research reports.
- Allow automatic consolidation only after the manual workflow feels reliable.

### Agent Access

Agents should not receive all memory by default in every prompt.

Instead, tools should be mounted when memory exists:

```text
read_memory -> returns relevant L3 Markdown
write_memory -> writes only explicit user preferences or user-approved facts
```

Prompt guidance should say:

```text
For personalized teaching, quiz generation, review planning, or long-running
learning context, call read_memory before planning. Do not quote memory as
factual source material; use it to adapt teaching behavior.
```

This keeps the model responsible for deciding when memory is useful while
keeping the user's long-term profile visible and correctable.

## 4. Page Ownership Changes

### Quiz Page

The standalone Quiz page should be removed from primary navigation.

Quiz remains as:

- an agent capability/tool available from chat,
- an in-chat interactive Quiz card,
- historical records in Space / Quiz Bank.

This keeps planning and generation in the conversation and review in the Space.

### Books Page (Retired)

Books are no longer a product surface. Research reports and other generated
learning records remain in Notebook. The former Book UI, routes, stores,
actions, and source targets should be deleted without a compatibility or data
migration layer.

### Research Reports

Research reports should be saved to Notebook first:

```ts
NotebookEntry {
  type: 'research_report'
  title: string
  markdown: string
  metadata: {
    query?: string
    sources?: ResearchSource[]
    sessionId?: string
    toolTraceIds?: string[]
    generatedBy: 'research'
  }
}
```

## 5. Data Model

Start with one default Space. Add multiple Spaces only after the module model is stable.

```ts
Space {
  id: string
  name: string
  description?: string
  createdAt: string
  updatedAt: string
}
```

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

```ts
QuizSession {
  id: string
  spaceId?: string
  sourceType?: 'conversation' | 'knowledge_base' | 'notebook_entry' | 'mentioned_space_items'
  sourceId?: string
  sourceMentionIds?: string[]
  title: string
  questions: QuizQuestion[]
  answers: QuizAnswer[]
  score?: QuizScore
  createdAt: string
  updatedAt: string
}
```

```ts
SpaceMention {
  id: string
  type: 'notebook_entry' | 'quiz_session' | 'quiz_question'
  targetId: string
  questionId?: string
  title: string
  preview?: string
  metadata?: Record<string, unknown>
}
```

`SpaceMention` is a chat/session boundary object. It should store enough
information for the UI to render chips and for the backend to resolve the
artifact later, but it should not duplicate full Notebook or Quiz content.

```ts
StudentProfile {
  spaceId: string
  summary: string
  strengths: ProfileConcept[]
  weaknesses: ProfileConcept[]
  recentActivity: ProfileActivity[]
  quizStats: ProfileQuizStats
  recommendedTasks: ProfileRecommendation[]
  updatedAt: string
}
```

The persisted representation for Student Profile should come from Markdown
memory entries first. A structured `StudentProfile` object can be introduced
later as a cache or projection if the UI needs faster filtering.

```ts
MemoryEntry {
  id: string
  layer: 'L2' | 'L3'
  file: 'chat' | 'quiz' | 'notebook' | 'knowledge' | 'recent' | 'profile' | 'scope' | 'preferences' | 'teaching_strategy'
  section: string
  text: string
  refs: string[]
  createdAt?: string
  updatedAt?: string
}
```

## 6. Navigation Direction

Near-term navigation:

```text
Chat
Tutor
Writing
Knowledge
Space
Memory
Settings
```

Remove standalone Quiz from primary navigation after Quiz Bank exists in Space.

Longer-term navigation can be simplified:

```text
Chat
Knowledge
Space
Settings
```

Do not rush this simplification until Space is useful enough.

## 7. Implementation Plan

### Phase 1: Space Shell

- [x] Replace Space placeholder with a tabbed Space page.
- [x] Add tabs: Notebook, Quiz Bank, Student Profile.
- [x] Use a default Space record.
- [x] Keep layout consistent with current blue/white/gray product UI.

### Phase 2: Notebook Store

- [x] Add `NotebookEntry` store.
- [x] Add list/create/read/update/delete notebook entry APIs.
- [x] Add Notebook tab list/detail UI.
- [x] Save Research reports to Notebook as `type = research_report`.
- [x] Move current "Save to book" primary action to "Save to notebook".
- [ ] Remove any remaining "Send to book" action and Book compatibility path.

### Phase 3: Quiz Bank

- [x] Remove standalone Quiz nav entry.
- [x] Keep composer Quiz mode for the current V1 implementation.
- [x] Move Quiz history/review UI into Space / Quiz Bank.
- [x] Keep Quiz generation in chat only.
- [x] Expose Quiz sessions and questions as Chat `@` mention targets.
- [x] Add filters by source type later.
- [x] Redesign Quiz mode into an enabled Chat capability/tool instead of automatic generation on every send.
- [x] Add a `create_quiz` product tool that the agent can call after explicit user intent or plan confirmation.
- [x] Add a `propose_quiz_plan` product tool for scope discussion before generation.
- [x] Keep normal chat behavior while Quiz capability is enabled so the user can discuss quiz scope before generation.
- [x] Split latest user instruction from source material at the API/tool boundary with `kb_id`, `notebook_entry_id`, `source_text`, and `source_label`.
- [x] Add deterministic Quiz validation before saving final questions.
- [ ] Add a controlled LLM Quiz verifier stage after generation and before saving final questions.
- [ ] Add structured verifier output for pass/revise/reject, issue list, citation support, and explanation consistency.
- [ ] Add retry-or-discard behavior for failed generated questions.

### Phase 3A: Chat Mentions and Agent-Assisted Notebook Edits

- [x] Add a backend lookup endpoint for Space mention candidates.
- [x] Add a backend read endpoint for resolving mentioned Space artifacts.
- [x] Add structured mention storage to chat messages/sessions.
- [x] Add `read_space_item` product tool for Notebook entries, Quiz sessions, and Quiz questions.
- [x] Render selected mentions as compact chips in the chat composer.
- [x] Render sent mentions as compact references in the message body or metadata area.
- [x] Let Chat mode answer questions about mentioned Space artifacts.
- [x] Let Quiz mode generate questions from mentioned Space artifacts.
- [x] Let Chat propose Notebook edits for mentioned Notebook entries.
- [x] Require explicit user confirmation before applying an agent-produced Notebook edit.
- [x] Create Notebook memory events after confirmed agent edits.

### Phase 4: Student Profile

- [x] Add Memory module shell if it does not exist.
- [x] Add Markdown memory file viewer/editor.
- [x] Add manual consolidation action for L1/L2/L3 memory.
- [x] Add `read_memory` tool for agents.
- [x] Mount `read_memory` for Quiz, Research, Chat, and Deep Solve when memory exists.
- [x] Render Student Profile from `L3/profile.md`, `L3/recent.md`, and `L3/teaching_strategy.md`.
- [x] Derive basic stat cards from Quiz history as supplemental UI.
- [x] Keep profile explainable and user-editable through Markdown.

### Phase 5: Book Removal

- [ ] Remove Book navigation and page components.
- [ ] Remove Book routes, stores, source targets, and tests that only support
  the retired capability.
- [ ] Remove Book persistence without migrating old data.
- [ ] Remove stale Book requirements and user-facing documentation.

## 8. Open Questions

- Should there be exactly one default Space in v0.1, or should users create named Spaces immediately?
- Should Knowledge Bases belong to Spaces, or remain global with optional Space links?
- Should Student Profile remain pure Markdown for v0.1, or should we add a structured cache once the UI needs filtering?
- Should Notebook entries be indexed into RAG automatically?
- Should automatic memory consolidation be opt-in per trigger, or globally enabled after the user approves it once?
