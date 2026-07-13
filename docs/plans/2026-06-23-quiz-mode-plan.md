# Quiz Mode Plan

> Status: active / V1 implemented | Date: 2026-06-23 | Last updated: 2026-07-05 | Scope: chat-driven quiz generation plus durable Quiz Bank review with structured questions, answers, scoring, explanations, and citations.

## 1. Goal

Quiz mode should turn selected learning material into an interactive assessment workflow.

The user should be able to:

- discuss quiz scope with the agent before generation,
- generate a small set of questions from a knowledge base, conversation, attachment,
  Notebook entry, or explicit `@` Space reference,
- answer questions one by one,
- get immediate scoring and explanations,
- see source chunks behind each question,
- review weak points after finishing.

## 2. First Version Scope

The original V1 started with a dedicated Quiz page. The current product direction
has changed: quiz generation happens from Chat through the Quiz capability, while
historical quiz review happens in Space / Quiz Bank.

Quiz is different from ordinary chat:

- chat is open-ended conversation,
- quiz generation is a structured product tool flow,
- every question has answer options, scoring, explanation, and source citations,
- quiz sessions can be resumed and reviewed later.

Current V1 supports:

- selected knowledge bases,
- conversation/attachment/source text material,
- Notebook entries,
- `@` referenced Space items,
- single-choice questions,
- configurable topic, difficulty, and question count,
- generated explanations and citations,
- local durable quiz sessions,
- result summary with score and missed questions.

Out of scope for V1:

- multi-choice partial scoring,
- free-answer LLM judging,
- timed exams,
- spaced repetition,
- sharing/export,
- independent generation buttons in Notebook or Research pages,
- adaptive question generation.

## 3. Layering

### Runtime / Agent Layer

Use runtime and agent framework capabilities first:

- provider calls,
- tool orchestration,
- RAG retrieval through `rag_search`,
- trace/status events,
- runtime sessions where conversation context is needed.

Do not build a separate agent loop for quiz generation.

### `tutor-agent` Layer

Owns quiz generation prompts and structured output parsing.

Implemented capabilities:

```text
propose_quiz_plan(title, topic, source, difficulty, question_count, notes)
create_quiz(title?, kb_id?, notebook_entry_id?, source_text?, source_label?, topic?, difficulty?, question_count?)
```

For V1, quiz generation is a controlled product flow. The flow is now also
declared as a runtime `Workflow` and validated before generation:

```text
collect_sources -> generate_questions -> verify_questions -> publish_questions
                                      ^             |
                                      |             v
                                      +---------- repair
```

The current implementation still executes this flow in product code. The next
runtime migration step is to run it through `WorkflowEngine` so retries, repair
loops, trace, cancellation, and cost/budget policy are owned by the framework.

Today the flow:

- retrieves source chunks,
- asks the LLM to generate JSON questions grounded in those chunks,
- validates the JSON shape,
- runs a strict structured LLM verifier against generated questions and source
  chunks,
- stores the Quiz in `QuizStore`,
- returns quiz details to Chat so the UI can render an interactive card.

### `tutor-web` Layer

Owns product APIs and persistence:

- quiz session store,
- generate quiz endpoint and `create_quiz` product tool,
- `propose_quiz_plan` product tool,
- submit answer endpoint,
- read quiz session endpoint,
- optional trace events for quiz generation.

### `web-ui` Layer

Owns the Chat Quiz card and Space / Quiz Bank review UI:

- quiz planning and generation affordances in the composer,
- question player,
- answer submission,
- explanation and citation display,
- final report.

## 4. Data Model

```ts
QuizSession {
  id: string
  title: string
  kbId: string
  status: 'draft' | 'generating' | 'active' | 'finished' | 'error'
  config: QuizConfig
  questions: QuizQuestion[]
  answers: QuizAnswer[]
  score?: QuizScore
  createdAt: string
  updatedAt: string
}
```

```ts
QuizConfig {
  topic?: string
  difficulty: 'easy' | 'medium' | 'hard'
  questionCount: number
  questionType: 'single_choice'
}
```

```ts
QuizQuestion {
  id: string
  type: 'single_choice'
  stem: string
  options: Array<{ id: string; text: string }>
  correctOptionId: string
  explanation: string
  citations: Array<{ source: string; text: string; score?: number }>
  tags: string[]
  difficulty: 'easy' | 'medium' | 'hard'
}
```

```ts
QuizAnswer {
  questionId: string
  selectedOptionId: string
  correct: boolean
  answeredAt: string
}
```

## 5. API Shape

```text
GET    /api/quizzes
POST   /api/quizzes
GET    /api/quizzes/{quiz_id}
POST   /api/quizzes/{quiz_id}/answers
POST   /api/quizzes/{quiz_id}/finish
DELETE /api/quizzes/{quiz_id}
```

Generation request:

```json
{
  "kb_id": "kb_x",
  "notebook_entry_id": "optional_note_id",
  "source_text": "optional explicit source text",
  "source_label": "conversation / attachment / @ item label",
  "topic": "光刻模型",
  "difficulty": "medium",
  "question_count": 5
}
```

Answer request:

```json
{
  "question_id": "q1",
  "selected_option_id": "B"
}
```

## 6. UI Shape

Quiz generation is exposed through the Chat composer Quiz capability. The old
standalone Quiz navigation was removed after Space / Quiz Bank reached review
parity.

Chat generation flow:

```text
User discusses quiz goals in Chat
  -> agent may call propose_quiz_plan
  -> user confirms or gives unambiguous generation request
  -> agent calls create_quiz
  -> backend generates and validates questions
  -> Chat renders an interactive Quiz card
  -> Quiz record appears in Space / Quiz Bank
```

Quiz Bank layout:

```text
Space / Quiz Bank

Left
  historical quizzes
  source filters
  scores and status

Main
  Question n / total
  Stem
  Options
  Explanation
  Citations

Summary
  Score
  Missed questions
  Weak tags
  Review suggestions
```

The page should keep the existing blue / white / gray visual style.

V1 UI should stay simple:

- Chat: planning, generation request, and interactive answering,
- Space / Quiz Bank: historical review, explanation, citations, and final review,
- no separate marketing or landing page.

## 7. Current Verification Flow

The current implementation has deterministic validation plus a strict structured
LLM verifier. When a usable LLM config exists, generation and verification run
through runtime `WorkflowEngine`; otherwise the backend uses a deterministic
fallback generator.

1. `create_quiz` builds source material from a selected KB, Notebook entry,
   explicit `source_text`, or Chat/Space material resolved before tool call.
2. `tutor_agent::quiz::generate_quiz_questions_with_workflow` validates and
   runs the runtime workflow:
   `collect_sources -> generate_questions -> verify_questions -> publish_questions`.
3. `collect_sources` is a product executor step that prepares source chunks and
   generation instructions in workflow context.
4. `generate_questions` is a runtime LLM step. It must call
   `submit_step_result` with structured question JSON.
5. `verify_questions` is a runtime LLM step. It must call `submit_step_result`
   with `{"verdict":"pass","issues":[]}` or
   `{"verdict":"fail","action":"repair","issues":[...]}`.
6. The workflow judge routes a verifier repair back to `generate_questions`
   once. A second verifier failure aborts the workflow instead of publishing weak
   questions.
7. `publish_questions` is a product executor step that validates and returns the
   final structured questions.
8. Generated source indices are mapped to `QuizCitation` metadata. Citation text
   is derived from the cited chunk and supporting quote when available.
9. `validate_quiz_questions_for_storage` rejects questions with:
   - empty stems,
   - fewer than two options,
   - a missing/nonexistent correct option,
   - empty explanations,
   - missing citations,
   - empty citation text.
10. The quiz is stored with a `QuizVerificationReport`. Today this report records
   the validation method and issues at the storage boundary; the detailed
   verifier issues are surfaced as generation errors when verification fails.

The verifier should continue to judge only supplied sources and should not
browse, introduce external facts, or behave like a free-form chat agent.

### Intermediate Output Handling

Quiz generation is triggered by the outer Quiz Chat agent calling the
`create_quiz` product tool. The detailed generation workflow runs inside that
tool call.

- Ordinary Quiz mode conversation before `create_quiz` behaves like normal Chat:
  model text deltas stream through the final-answer `content` channel.
- `propose_quiz_plan` returns a structured tool result. The WebSocket bridge
  persists and streams it as a `tool_result` trace; the UI attaches the parsed
  plan to the next assistant message.
- `create_quiz` runs the runtime workflow internally. Its workflow step outputs
  (`collect_sources`, `generate_questions`, verifier repair/fail/pass, and
  `publish_questions`) are not currently emitted as separate WebSocket
  `progress_content` events or durable assistant bubbles.
- When `create_quiz` completes, the outer agent receives one tool result whose
  details include the saved `quiz`. The WebSocket bridge persists and streams
  that as a `tool_result` trace; the UI parses it and attaches the interactive
  Quiz card to the assistant message.
- The interactive Quiz card must not depend only on the live WebSocket event or
  transient React state. The persisted assistant message should carry a stable
  attachment reference to the saved Quiz session so navigating away, reloading,
  or returning to the session can hydrate the same answerable card.
- The outer agent may then produce a short final assistant response, which uses
  the normal `content` channel. The intermediate generated drafts and verifier
  outputs remain workflow-internal structured data unless the workflow fails and
  surfaces an error.

## 8. Implementation Phases

### Phase 1: Product Shell

- [x] Add initial Quiz navigation entry.
- [x] Add initial Quiz page route/view.
- [x] Add configuration panel.
- [x] Add empty state and generated-question mock view.
- [x] Remove standalone Quiz navigation after Quiz Bank review parity.

### Phase 2: Persistence

- [x] Add quiz store JSON file.
- [x] Add quiz session CRUD APIs.
- [x] Add answer submission and score calculation.
- [x] Add tests for store and scoring.

### Phase 3: RAG-backed Generation

- [x] Retrieve source chunks from selected knowledge base.
- [x] Generate from conversation/source text.
- [x] Generate from Notebook entries.
- [x] Generate from `@` referenced Space material.
- [x] Generate single-choice questions as strict JSON.
- [x] Validate generated questions.
- [x] Attach citations to questions.
- [x] Add non-real-LLM test with mock quiz generation.

### Phase 4: Review Experience

- [x] Show per-question explanation.
- [x] Show citations below explanations.
- [x] Add final score summary.
- [x] Add missed-question review list.

### Phase 5: Runtime Feedback

- [x] If quiz generation needs better structured-output support, record feedback in `docs/framework-feedback.md`.
- [x] If runtime has suitable session custom entries, use them for quiz generation trace instead of parallel trace storage.

Note: V1 does not introduce separate quiz trace persistence. Future quiz generation trace should reuse runtime session/custom entries instead of adding another product-specific trace store.

### Phase 6: Chat Tool Flow

- [x] Add `propose_quiz_plan` product tool.
- [x] Add `create_quiz` product tool.
- [x] Let Quiz capability keep normal chat behavior until the agent calls a tool.
- [x] Save `create_quiz` output into Quiz Bank.
- [x] Render generated quizzes as interactive Chat cards.
- [ ] Persist Chat Quiz card attachments as stable references to saved Quiz
  sessions and hydrate them when the user returns to the session.
- [ ] Add a regression test that generates a Quiz in Chat, reloads or reopens
  the session, and verifies the interactive Quiz card is restored.
- [ ] Preserve completed `create_quiz` tool results when the user switches
  sessions before the outer agent produces its final short response.
- [ ] Add stronger provider-behavior QA so agents consistently plan before
  generating when the request is ambiguous.

### Phase 7: Verification Hardening

- [x] Add deterministic validation before storage.
- [x] Store `QuizVerificationReport`.
- [x] Add controlled LLM verifier stage.
- [x] Add structured verifier output.
- [x] Declare and validate Quiz flow as a runtime workflow.
- [x] Repair once and re-verify before surfacing unresolved verifier failures.
- [x] Add tests for answer/explanation/citation contradiction cases.
- [x] Execute Quiz generation/verification orchestration through runtime
  `WorkflowEngine` executor steps.
- [x] Move verifier repair routing from the transitional direct-call path into
  `WorkflowEngine` transitions.
- [x] Move generation and verifier LLM calls from workflow executor internals
  into native runtime LLM steps using `submit_step_result`.

## 9. Acceptance Criteria

V1 is complete when:

- a user can select Quiz in Chat,
- discuss scope before generation,
- create a quiz from an existing knowledge base, conversation, source text, or
  Notebook entry,
- answer every generated single-choice question,
- see whether each answer is correct,
- see explanation and citations,
- finish with a score summary,
- leave and return to the Chat session without losing the interactive Quiz
  card,
- reload the app and see the quiz session restored in Space / Quiz Bank.
