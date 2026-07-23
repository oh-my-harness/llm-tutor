# Runtime Migration Audit

Date: 2026-07-08

> Historical note (2026-07-16): the standalone Deep Solve capability has been
> retired. Its references in this audit record the migration state on the audit
> date and do not describe a capability available for new runs.
>
> Update (2026-07-23): the repository is now pinned to runtime branch
> `codex/session-projection` at `8ab2a377` and adapter `16a22ad`. The A1/A2
> Tool, workflow, run-context, and Session Projection baseline has been
> migrated. Knowledge source and product-path work remains staged in
> `docs/plans/2026-07-23-runtime-knowledge-a6-migration-plan.md`.

## Current Evidence

- The project pins all `llm-harness-*` crates to
  `8ab2a3770bee0e7a1731b8074552ef9b6d70653a`.
- The aligned `llm-api-adapter` revision is
  `16a22ad284b8deb8c3a77664a0876f565f4a6eb9`.
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
- Quiz generation, Memory maintenance, and detailed Research run through
  runtime `WorkflowEngine`.
- Memory and Research use runtime declarative edge routing through a thin no-op
  marker judge.
- Quiz, Memory, and Research LLM steps declare structured output and finish
  with JSON assistant text; runtime populates `StepResult.structured`.
- Ordinary Chat, Research conversation, Organize, Quiz conversation, and Code
  Exec now enter the harness through `RunRequest`. Product integration coverage
  proves a typed extension reaches a Chat product Tool through
  `ToolContext.run`.
- All 26 production Tools use explicit `Projected` or `Ephemeral` Session
  projection. The checked inventory lives in
  `docs/runtime-tool-projections.json`.
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

Checked against runtime `codex/session-projection` commit `8ab2a377` on
2026-07-23.

| Area | Runtime evidence | Product decision |
| --- | --- | --- |
| Typed run context | `AgentHarness::run(RunRequest)` constructs one immutable `RunContext`, and Tools receive it through `ToolContext.run`. | Ordinary product capabilities now use `RunRequest`; integration coverage proves typed extensions reach Chat Tools. |
| Workflow run context | `WorkflowEngine::run_llm_step` still starts each step through `harness.prompt(&prompt_text)`, with no extension-bearing request. | Keep Knowledge out of workflow steps until runtime can propagate trusted extensions; do not add product-owned run state. |
| Tool Session projection | `ToolResult::projected` and `ToolResult::ephemeral` control durable model-visible Tool content. | All production Tools have an audited explicit projection; Full and struct-literal results fail the release audit. |
| Structured workflow output | `Step::with_structured(Some(true))` extracts final assistant JSON and supports provider response-format escalation. | Quiz, Memory, and Research use structured final output; product code retains domain deserialization and validation. |
| Knowledge citation validation | `CitationValidator` reads run-local evidence, but the plugin does not validate the candidate final answer before persistence. | Treat Knowledge product cutover as blocked until runtime owns the final-answer validation boundary. |
| Declarative workflow routing | `workflow::judge::EdgeConditionJudge` and `NoopJudge` exist, but both are still `pub(crate)`. `WorkflowEngine` can auto-select the edge judge only when the provided judge reports `is_noop()`. | Keep the tiny `RuntimeDeclarativeJudge` marker until runtime exposes a public constructor/helper. |
| Bounded verifier repair | `WorkflowEngine::with_max_steps` is a global step-count guard. Runtime docs recommend loop counters in structured state for custom routing; no transition-level visit cap is public. | Keep `QuizWorkflowJudge` for the current "repair once, then fail" semantic verifier loop. |
| Harness setup | `HarnessBuilder` exposes `system_prompt`, `model_info`, `final_answer_mode`, provider registration, tools, and plugin hook registration. | Chat and Code Exec use `HarnessBuilder`; product hook vectors are injected through a tiny plugin. |
| Final answer contract | Runtime exposes `FinalAnswerMode`, `AgentEvent::as_final_answer()`, `AgentEvent::as_progress()`, and final/progress assistant message kinds. | Chat and Code Exec consume these APIs; tests cover both paths. |
| Streaming deltas | Runtime still emits raw `TextDelta` without final/progress classification; classification is available at terminal message events. | Keep live streaming as raw text for now, while durable bubbles use final-answer events. |
| Model metadata | Runtime accepts `ModelInfo` for context budgeting and compaction, but does not provide provider-normalized metadata discovery. | Keep product settings diagnostics for `/models` probing and inference until adapter/runtime owns discovery. |
| Budget policy | Runtime still exposes `BudgetControlAdapter` as a `ShouldStopHook`, and `HarnessBuilder::budget` wires it into loop stop behavior. `HarnessBuilder` does inject `CostAccumulatorHook`, and the harness exposes `usage()`. | Emit and consume runtime usage traces from `AgentHarness::usage()` for observability, but keep budget limits as product config only until runtime separates accounting from loop continuation. |
| Workflow usage | `WorkflowEngine::run()` returns `TaskResult.cost`, aggregated from step results. `WorkflowEngine::total_cost()` is also available for an active engine. | Quiz, Memory, and Research workflow helpers return runtime cost with their domain output. |
| Tool-call adjacency | The reviewed baseline retains provider-neutral ordering for consecutive tool results. | Keep provider-specific normalization in runtime/adapter code and cover product multi-tool paths through integration tests. |

## Next Runtime API Requests

1. Workflow-level propagation of trusted `RunRequest` extensions.
2. Final-answer Knowledge citation validation with the current `RunContext`.
3. Public declarative workflow constructor or no-op judge helper.
4. Declarative bounded semantic repair / step visit policies.
5. Provider-aware typed structured domain output helper.
6. Safe budget policy helper that separates accounting from loop continuation.
7. Normalized model metadata discovery.
8. Per-delta final/progress classification for streaming UI.

## Verification Coverage

- `cargo test -p tutor-agent --test mock_integration` covers ordinary harness
  setup, runtime final/progress splitting for Chat and Code Exec, tool routing,
  typed RunRequest extension propagation, runtime usage traces, Research
  workflow behavior, and Code Exec sandbox execution.
- `scripts/check-tool-projections.ps1` compares every production Tool name with
  the reviewed projection inventory and rejects Full/struct-literal results.
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
