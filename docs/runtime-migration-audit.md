# Runtime Migration Audit

Date: 2026-07-07

## Current Evidence

- `llm-harness-runtime` remote HEAD is `bea5374690192f2e32943073ced10f66c120db91`.
- `Cargo.toml` and `Cargo.lock` pin all `llm-harness-*` crates to the same
  runtime revision.
- `main` is synchronized with `origin/main` after the latest runtime migration
  commits.

## Runtime-Owned Capabilities In Use

- Durable chat history uses `JsonlSessionRepo` and runtime `Session` APIs.
- Chat, Research, Organize, Quiz-mode chat, and Code Exec harness setup goes
  through runtime `HarnessBuilder`.
- Final assistant text is restored from runtime `AssistantMessageKind::FinalAnswer`
  via `AgentEvent::as_final_answer()`.
- Progress messages are detected through runtime `AgentEvent::as_progress()` and
  are not restored as final chat bubbles.
- Automatic compaction calls runtime `AgentHarness::compact()` and reads compact
  summaries from runtime `SessionEntryPayload::Compaction`.
- Deep Solve, Quiz generation, and Memory workflows run through runtime
  `WorkflowEngine`.
- Deep Solve and Memory use runtime declarative edge routing through a thin
  no-op marker judge.
- Quiz uses runtime LLM workflow steps and `submit_step_result` for generated
  questions and verifier results.
- Workflow step progress is consumed from runtime `WorkflowEvent::StepProgress`
  and bridged to product trace events.

## Removed Or Avoided Product Reimplementations

Active source no longer contains the old Deep Solve `PhaseManager`,
`ReplanHook`, `ReplanTool`, `SolveContext`, or direct `AgentHarnessOptions`
construction paths.

Product session storage does not duplicate message history. It stores product
metadata and custom runtime session entries for UI concepts such as trace,
mentions, and citations.

## Remaining Product Adapters

- `RuntimeDeclarativeJudge` remains as a marker because runtime's no-op /
  declarative judge helpers are not public.
- `QuizWorkflowJudge` remains as a bounded semantic repair policy because
  runtime declarative edges cannot yet express "repair once, then fail" based on
  verifier output and step history.
- Settings diagnostics still probe providers directly because model metadata
  discovery is not normalized at the adapter/runtime boundary.
- Text streaming still emits raw `TextDelta` because runtime deltas do not carry
  final/progress classification; classification is only available on
  `MessageEnd`.
- Budget limits are stored in product config but direct runtime budget hook
  wiring is disabled until runtime exposes a safe app-level budget policy for
  ordinary one-turn harnesses and workflows.

## Next Runtime API Requests

1. Public declarative workflow constructor or no-op judge helper.
2. Declarative bounded semantic repair / step visit policies.
3. Provider-aware typed structured output helper.
4. Tool-using structured generation helper.
5. Safe budget policy helper that separates accounting from loop continuation.
6. Normalized model metadata discovery.
7. Per-delta final/progress classification for streaming UI.
