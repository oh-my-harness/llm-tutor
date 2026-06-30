# Space Workspace Plan

> Status: proposed | Date: 2026-06-26 | Scope: define Space as the project-level learning workspace that contains Notebook, Quiz Bank, and Student Profile.

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

### Quiz Bank

Quiz Bank shows historical quizzes and practice records.

Quiz Bank should not be the primary generation surface.

Quiz generation stays in conversation through the composer `Quiz` mode:

```text
User asks in chat -> Agent generates Quiz card -> User answers in chat -> Quiz record appears in Space / Quiz Bank
```

Notebook and Research report detail pages should not expose independent Quiz
generation buttons. Saved notes and reports are durable material; quiz
generation remains a Chat action so the active mode, model, source selection,
attachments, and conversation context are explicit.

Quiz Bank responsibilities:

- list historical quizzes,
- show scores,
- show missed questions,
- show explanations and citations,
- support re-practice/review,
- filter by source later.

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
    research.md

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
- `notebook.md`: saved topics, important notes, research artifacts.
- `research.md`: researched themes, durable findings, source patterns.

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

- a composer capability,
- an in-chat interactive Quiz card,
- historical records in Space / Quiz Bank.

This keeps generation in the conversation and review in the Space.

### Books Page

Books remain a durable output surface, but they are not the first destination for raw research.

Preferred flow:

```text
Research report -> Notebook entry -> optional Book chapter
```

Books are polished outputs. Notebook is the working memory.

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
  sourceType?: 'conversation' | 'knowledge_base' | 'notebook_entry'
  sourceId?: string
  title: string
  questions: QuizQuestion[]
  answers: QuizAnswer[]
  score?: QuizScore
  createdAt: string
  updatedAt: string
}
```

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
  file: 'chat' | 'quiz' | 'notebook' | 'research' | 'recent' | 'profile' | 'scope' | 'preferences' | 'teaching_strategy'
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
Books
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

- [ ] Replace Space placeholder with a tabbed Space page.
- [ ] Add tabs: Notebook, Quiz Bank, Student Profile.
- [ ] Use a default Space record.
- [ ] Keep layout consistent with current blue/white/gray product UI.

### Phase 2: Notebook Store

- [ ] Add `NotebookEntry` store.
- [ ] Add list/create/read/update/delete notebook entry APIs.
- [ ] Add Notebook tab list/detail UI.
- [ ] Save Research reports to Notebook as `type = research_report`.
- [ ] Move current "Save to book" primary action to "Save to notebook".
- [ ] Add secondary "Send to book" action from Notebook entry.

### Phase 3: Quiz Bank

- [ ] Remove standalone Quiz nav entry.
- [ ] Keep composer Quiz mode.
- [ ] Move Quiz history/review UI into Space / Quiz Bank.
- [ ] Keep Quiz generation in chat only.
- [ ] Add filters by source type later.

### Phase 4: Student Profile

- [ ] Add Memory module shell if it does not exist.
- [ ] Add Markdown memory file viewer/editor.
- [ ] Add manual consolidation action for L1/L2/L3 memory.
- [ ] Add `read_memory` tool for agents.
- [ ] Mount `read_memory` for Quiz, Research, Chat, and Deep Solve when memory exists.
- [ ] Render Student Profile from `L3/profile.md`, `L3/recent.md`, and `L3/teaching_strategy.md`.
- [ ] Derive basic stat cards from Quiz history as supplemental UI.
- [ ] Keep profile explainable and user-editable through Markdown.

### Phase 5: Book Integration

- [ ] Allow Notebook entries to become Book chapters.
- [ ] Replace `sourceReportId` with `sourceNotebookEntryId` in future book chapter metadata.
- [ ] Keep Books as polished outputs, not raw research storage.

## 8. Open Questions

- Should there be exactly one default Space in v0.1, or should users create named Spaces immediately?
- Should Knowledge Bases belong to Spaces, or remain global with optional Space links?
- Should Student Profile remain pure Markdown for v0.1, or should we add a structured cache once the UI needs filtering?
- Should Notebook entries be indexed into RAG automatically?
- Should Book chapters remain separate from Notebook entries or become a published view of them?
- Should automatic memory consolidation be opt-in per trigger, or globally enabled after the user approves it once?
