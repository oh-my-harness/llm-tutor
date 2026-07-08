# Runtime Migration Audit

Date: 2026-07-08

## Current Evidence

- `llm-harness-runtime` main HEAD is `bea5374690192f2e32943073ced10f66c120db91`; the project is pinned to issue #43 fix branch commit `e200c12a69b896a0d9ab70d2752f9dafcbfc07ad`.
- The fix branch is behind open PR #44 and is not yet merged, so this is a deliberate temporary pin to validate OpenAI-compatible tool-call adjacency.
- `Cargo.toml` and `Cargo.lock` pin all `llm-harness-*` crates to the same
  runtime revision.
- `main` is synchronized with `origin/main` after the latest runtime migration
  commits.

## Runtime-Owned Capabilities In Use

- Durable chat history uses `JsonlSessionRepo` and runtime `Session` APIs.
- Chat, Research, Organize, Quiz-mode chat, and Code Exec harness setup uses
  runtime `HarnessBuilder`. Product hooks are injected through a minimal plugin,
  while runtime owns provider resolution, final-answer mode, model metadata, and
  cost hook injection.
- Final assistant text is restored from runtime `AssistantMessageKind::FinalAnswer`
  via `AgentEvent::as_final_answer()`.
- Progress messages are detected through runtime `AgentEvent::as_progress()` and
  are not restored as final chat bubbles.
  - Test evidence: `chat_returns_runtime_final_answer_not_progress_text` covers
    a progress `MessageEnd` followed by a final answer and asserts that the
    product return value uses only the runtime final-answer event while progress
    is emitted as trace.
  - Test evidence: `code_exec_returns_runtime_final_answer_not_progress_text`
    covers the same contract for the Code Exec harness path.
- Automatic compaction calls runtime `AgentHarness::compact()` and reads compact
  summaries from runtime `SessionEntryPayload::Compaction`.
- Deep Solve, Quiz generation, and Memory workflows run through runtime
  `WorkflowEngine`.
- Deep Solve and Memory use runtime declarative edge routing through a thin
  no-op marker judge.
- Quiz uses runtime LLM workflow steps and `submit_step_result` for generated
  questions and verifier results.
- Workflow step progress was consumed from runtime `WorkflowEvent::StepProgress`
  on `bea5374`; `cc0b737` temporarily removed that event; `e200c12` exposes it
  again. The product bridge currently still emits workflow step
  start/finish/failure only.
- Ordinary Chat and Code Exec turns emit `runtime_usage` trace events from
  runtime `AgentHarness::usage()`. This reuses the `CostAccumulatorHook`
  injected by `HarnessBuilder` instead of duplicating token accounting in
  product code.
- Deep Solve workflow emits `runtime_usage` from runtime `TaskResult.cost`,
  so multi-step workflow usage follows the same product trace/UI/session
  restore path as ordinary harness turns.
- Quiz generation and Memory workflows now return runtime `TaskResult.cost`
  alongside their domain output, so callers no longer need to reconstruct
  workflow usage from product-side state.
- The web UI consumes `runtime_usage` as the live context-usage fallback and
  budget spent source when provider message usage is unavailable.
- Session restore also derives `latest_usage` from persisted `runtime_usage`
  trace entries, so archived conversations keep runtime token usage even when
  provider message usage is absent.

## Removed Or Avoided Product Reimplementations

Active source no longer contains the old Deep Solve `PhaseManager`,
`ReplanHook`, `ReplanTool`, `SolveContext`, or ordinary direct
`AgentHarnessOptions` construction paths.

Product session storage does not duplicate message history. It stores product
metadata and custom runtime session entries for UI concepts such as trace,
mentions, and citations.

## Remaining Product Adapters

- `RuntimeDeclarativeJudge` remains as a marker because runtime's no-op /
  declarative judge helpers are not public.
- `QuizWorkflowJudge` remains as a bounded semantic repair policy because
  runtime declarative edges cannot yet express "repair once, then fail" based on
  verifier output and step history.
  - Audit note: runtime `WorkflowEngine::with_max_steps` was checked as a
    possible replacement. It is a global step-history guard, not a transition
    visit policy. With the current Quiz graph, `max_steps = 5` blocks the
    successful repair path before `publish_questions`; `max_steps = 6` allows a
    second verifier failure to enter a third generation attempt. Therefore it
    cannot replace the product judge without changing Quiz semantics.
- Settings diagnostics still probe providers directly because model metadata
  discovery is not normalized at the adapter/runtime boundary.
- Text streaming still emits raw `TextDelta` because runtime deltas do not carry
  final/progress classification; classification is only available on
  `MessageEnd`.
- Budget limits are stored in product config but direct runtime budget hook
  wiring is disabled until runtime exposes a safe app-level budget policy for
  ordinary one-turn harnesses and workflows.

## Latest Runtime API Recheck

Checked against local runtime checkout
`llm-harness-runtime-6a63eaf83d5f868e/e200c12` on 2026-07-08.

| Area | Runtime evidence | Product decision |
| --- | --- | --- |
| Declarative workflow routing | `workflow::judge::EdgeConditionJudge` and `NoopJudge` exist, but both are still `pub(crate)`. `WorkflowEngine` can auto-select the edge judge only when the provided judge reports `is_noop()`. | Keep the tiny `RuntimeDeclarativeJudge` marker until runtime exposes a public constructor/helper. |
| Bounded verifier repair | `WorkflowEngine::with_max_steps` is a global step-count guard. Runtime docs recommend loop counters in structured state for custom routing; no transition-level visit cap is public. | Keep `QuizWorkflowJudge` for the current "repair once, then fail" semantic verifier loop. |
| Harness setup | `HarnessBuilder` exposes `system_prompt`, `model_info`, `final_answer_mode`, provider registration, tools, and plugin hook registration. | Chat and Code Exec use `HarnessBuilder`; product hook vectors are injected through a tiny plugin. |
| Final answer contract | Runtime exposes `FinalAnswerMode`, `AgentEvent::as_final_answer()`, `AgentEvent::as_progress()`, and final/progress assistant message kinds. | Chat and Code Exec consume these APIs; tests cover both paths. |
| Streaming deltas | Runtime still emits raw `TextDelta` without final/progress classification; classification is available at terminal message events. | Keep live streaming as raw text for now, while durable bubbles use final-answer events. |
| Model metadata | Runtime accepts `ModelInfo` for context budgeting and compaction, but does not provide provider-normalized metadata discovery. | Keep product settings diagnostics for `/models` probing and inference until adapter/runtime owns discovery. |
| Budget policy | Runtime still exposes `BudgetControlAdapter` as a `ShouldStopHook`, and `HarnessBuilder::budget` wires it into loop stop behavior. `HarnessBuilder` does inject `CostAccumulatorHook`, and the harness exposes `usage()`. | Emit and consume runtime usage traces from `AgentHarness::usage()` for observability, but keep budget limits as product config only until runtime separates accounting from loop continuation. |
| Workflow usage | `WorkflowEngine::run()` returns `TaskResult.cost`, aggregated from step results. `WorkflowEngine::total_cost()` is also available for an active engine. | Deep Solve emits `runtime_usage` from `TaskResult.cost`. Quiz and Memory workflow helpers return their domain output plus runtime cost; Memory traces also preserve the cost payload. A future UI pass can decide how to summarize non-chat workflow costs. |
| Tool-call adjacency | PR #44 keeps provider-neutral runtime conversion from inserting assistant messages between consecutive tool results. | `llm-tutor` pins to the PR commit to validate Research/multi-tool paths until the fix is merged upstream. |

## Next Runtime API Requests

1. Public declarative workflow constructor or no-op judge helper.
2. Declarative bounded semantic repair / step visit policies.
3. Provider-aware typed structured output helper.
4. Tool-using structured generation helper.
5. Safe budget policy helper that separates accounting from loop continuation.
6. Normalized model metadata discovery.
7. Per-delta final/progress classification for streaming UI.

## Verification Coverage

- `cargo test -p tutor-agent --test mock_integration` covers ordinary harness
  setup, runtime final/progress splitting for Chat and Code Exec, tool routing,
  runtime usage traces for Chat, Code Exec, and Deep Solve, Deep Solve workflow
  events, and Code Exec sandbox execution.
- `cargo test -p tutor-agent quiz --lib` covers Quiz runtime workflow generation,
  verifier repair, publish behavior, and returned workflow cost.
- `cargo test -p tutor-agent memory --lib` covers Memory runtime workflow output
  validation and returned workflow cost.
- `cargo test -p tutor-web routes::quiz --lib` and
  `cargo test -p tutor-web routes::memory --lib` cover web route compatibility
  after exposing runtime workflow cost through the agent boundary.
- `cargo test -p tutor-web session --lib` covers runtime-backed session
  persistence, custom UI entries, citations, mentions, trace entries, and
  compaction summaries, including restored runtime usage traces.
