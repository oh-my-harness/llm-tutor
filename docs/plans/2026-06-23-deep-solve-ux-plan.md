# Deep Solve UX Plan

> Status: complete | Date: 2026-06-23 | Scope: turn Deep Solve from a long chat answer into a structured guided solving workflow.

## 1. Goal

Deep Solve should feel like a solving workspace, not a delayed chat response.

The user should be able to see:

- what stage the agent is in,
- what plan it made,
- which tools or knowledge sources it used,
- what each solve step concluded,
- where the final answer came from,
- and how to ask follow-up questions about a specific step later.

## 2. Target Shape

For a Deep Solve turn, the UI should render one structured assistant result:

```text
Problem
  user question

Stage Timeline
  Understanding
  Retrieval
  Planning
  Step 1
  Step 2
  Verification
  Final Answer

Evidence
  RAG chunks
  tool calls
  code results

Final Answer Card
  direct answer
  explanation
  key formulas / concepts
  citations
```

Intermediate status and reasoning summaries should not become normal assistant bubbles. They should be small, collapsible, and attached to the Deep Solve result.

## 3. Layering

### Runtime Layer

Use existing runtime capabilities first:

- runtime sessions for durable messages,
- custom session entries for trace events,
- compaction entries when runtime compaction is available,
- tool call / tool result events,
- assistant message kind semantics where available.

Runtime should not know Tutor-specific stages such as `understand`, `plan`, or `verify`.

Possible runtime feedback:

- Add first-class trace event storage APIs such as `append_trace_event` and `read_trace_events`.
- Strengthen product-facing conventions for assistant progress vs final answer.

### `tutor-agent` Layer

Owns the Deep Solve workflow.

It emits structured events through `EventSink`:

```json
{
  "kind": "deep_solve_stage_start",
  "capability": "deep_solve",
  "stage": "plan",
  "title": "Create solve plan"
}
```

```json
{
  "kind": "deep_solve_stage_done",
  "capability": "deep_solve",
  "stage": "verify",
  "summary": "Verified result with code"
}
```

### `tutor-web` Layer

Responsibilities:

- stream Deep Solve events over WebSocket,
- persist events into runtime session custom trace entries,
- return restored events from `GET /api/sessions/{id}`,
- keep product metadata thin: selected capability, knowledge base, model config.

### `web-ui` Layer

Responsibilities:

- render Deep Solve events as a structured timeline,
- attach evidence and citations to stages,
- render final answer as the primary visible assistant result,
- keep trace/debug details available but visually secondary.

## 4. Event Schema

Start with a small stable event set:

| Event | Purpose |
|---|---|
| `deep_solve_stage_start` | A stage begins |
| `deep_solve_stage_delta` | Optional visible progress text for a stage |
| `deep_solve_stage_done` | A stage completes with summary |
| `deep_solve_plan` | Structured solve plan |
| `deep_solve_step_start` | A solve step begins |
| `deep_solve_step_done` | A solve step completes |
| `tool_call` | Existing tool call event, with `stage` when available |
| `tool_result` | Existing tool result event, with `stage` when available |
| `rag_citations` | Sources used for final answer |
| `deep_solve_final` | Final answer metadata, not necessarily the answer text itself |

Minimum common fields:

```ts
type DeepSolveEvent = {
  kind: string
  capability: 'deep_solve'
  stage?: 'understand' | 'retrieve' | 'plan' | 'solve' | 'verify' | 'synthesize'
  step_id?: string
  title?: string
  summary?: string
  details?: unknown
}
```

## 5. Implementation Plan

### Phase 1: Standardize Agent Events

- [x] Add Deep Solve event helper functions in `tutor-agent`.
- [x] Emit stage start/done for existing Deep Solve phases.
- [x] Include `stage` and `step_id` on tool trace events where possible.
- [x] Add mock/unit tests for event emission.

Acceptance:

- A Deep Solve run emits a predictable stage timeline.
- Events are visible in the existing Trace panel.

### Phase 2: Persist And Restore

- [x] Reuse `tutor-web` trace persistence for Deep Solve events.
- [x] Ensure restored session detail returns the Deep Solve event sequence.
- [x] Add backend test: run/store fake Deep Solve events -> reopen session -> restore events.

Acceptance:

- Refreshing or reopening a Deep Solve session preserves the timeline.

### Phase 3: Build Deep Solve UI

- [x] Add `DeepSolvePanel` or `DeepSolveMessage` component.
- [x] Render stage timeline with collapsed details.
- [x] Render plan and step summaries.
- [x] Attach tool results and citations to relevant stages.
- [x] Keep final answer as the main readable result.

Acceptance:

- Deep Solve no longer appears as multiple unrelated chat bubbles.
- User can scan the solve process without reading raw JSON trace.

### Phase 4: Evidence And Citations

- [x] Group RAG citations by stage or final answer.
- [x] Show source snippets below the final answer and inside evidence details.
- [x] Show code execution results in the verification stage.

Acceptance:

- User can tell which sources or tool results support the answer.

### Phase 5: Step Follow-Up

- [x] Add UI affordance to ask about a specific step.
- [x] Send follow-up metadata with `step_id` and session ID.
- [x] Add agent prompt context for step-specific follow-up.

Acceptance:

- User can ask why a step works without manually copying context.

## 6. Non-Goals For First Pass

- Do not implement a full visual proof editor.
- Do not add Tutor-specific stages to runtime.
- Do not make a separate Deep Solve session system.
- Do not require true LLM summarization for every stage before the basic UX works.

## 7. Follow-Up Enhancements

- Consider a dedicated backend follow-up payload if natural-language step context proves too weak.
- Consider first-class runtime trace APIs as framework feedback.
- Consider compaction rules for long-running stage details after real usage data exists.
- Consider answer-tool enforcement if prompt-only final answer structure is not reliable enough.

## 8. Completion Evidence

Completed in implementation:

1. Agent emits structured Deep Solve UX events.
2. Web session trace persistence preserves and restores Deep Solve event sequences.
3. UI renders Deep Solve as a structured assistant message with timeline, plan, step summaries, evidence, citations, final answer, and step follow-up affordances.
4. Checks run: `cargo check -p tutor-web -j 1`, targeted backend trace persistence tests, `npm run build`, and a local browser load check against Vite preview.
