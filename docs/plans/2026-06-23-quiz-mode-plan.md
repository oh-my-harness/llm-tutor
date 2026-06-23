# Quiz Mode Plan

> Status: planned | Date: 2026-06-23 | Scope: add a knowledge-base driven quiz workspace with structured questions, answers, scoring, explanations, and citations.

## 1. Goal

Quiz mode should turn a knowledge base into an interactive assessment workflow.

The user should be able to:

- choose a knowledge base and quiz settings,
- generate a small set of questions from indexed material,
- answer questions one by one,
- get immediate scoring and explanations,
- see source chunks behind each question,
- review weak points after finishing.

## 2. First Version Scope

Start with a dedicated Quiz page, not chat-driven quiz.

V1 supports:

- one selected knowledge base,
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

Expected capability:

```text
generate_quiz(kb, topic, difficulty, count) -> QuizQuestion[]
```

For V1, quiz generation can be a small dedicated flow that:

- retrieves source chunks,
- asks the LLM to generate JSON questions grounded in those chunks,
- validates the JSON shape,
- emits trace events for retrieval and generation.

### `tutor-web` Layer

Owns product APIs and persistence:

- quiz session store,
- generate quiz endpoint,
- submit answer endpoint,
- read quiz session endpoint,
- optional trace events for quiz generation.

### `web-ui` Layer

Owns the Quiz page:

- quiz configuration panel,
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
POST   /api/quizzes/{quiz_id}/generate
POST   /api/quizzes/{quiz_id}/answers
POST   /api/quizzes/{quiz_id}/finish
DELETE /api/quizzes/{quiz_id}
```

Generation request:

```json
{
  "kb_id": "kb_x",
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

Add a `测验` item to the left navigation.

Quiz page layout:

```text
Quiz

Left / top config
  Knowledge base
  Topic
  Difficulty
  Question count
  Generate

Main
  Question n / total
  Stem
  Options
  Submit
  Explanation
  Citations

Summary
  Score
  Missed questions
  Weak tags
  Review suggestions
```

The page should keep the existing blue / white / gray visual style.

## 7. Implementation Phases

### Phase 1: Product Shell

- [ ] Add Quiz navigation entry.
- [ ] Add Quiz page route/view.
- [ ] Add configuration panel.
- [ ] Add empty state and generated-question mock view.

### Phase 2: Persistence

- [ ] Add quiz store JSON file.
- [ ] Add quiz session CRUD APIs.
- [ ] Add answer submission and score calculation.
- [ ] Add tests for store and scoring.

### Phase 3: RAG-backed Generation

- [ ] Retrieve source chunks from selected knowledge base.
- [ ] Generate single-choice questions as strict JSON.
- [ ] Validate generated questions.
- [ ] Attach citations to questions.
- [ ] Add non-real-LLM test with mock quiz generation.

### Phase 4: Review Experience

- [ ] Show per-question explanation.
- [ ] Show citations below explanations.
- [ ] Add final score summary.
- [ ] Add missed-question review list.

### Phase 5: Runtime Feedback

- [ ] If quiz generation needs better structured-output support, record feedback in `docs/framework-feedback.md`.
- [ ] If runtime has suitable session custom entries, use them for quiz generation trace instead of parallel trace storage.

## 8. Acceptance Criteria

V1 is complete when:

- a user can open Quiz from the sidebar,
- create a quiz from an existing knowledge base,
- answer every generated single-choice question,
- see whether each answer is correct,
- see explanation and citations,
- finish with a score summary,
- reload the app and see the quiz session restored.

