# llm-harness-runtime v0.2 Framework Feedback

> Written after implementing tutor_agent v0.1 (2026-06-13).
> This document records friction points, missing APIs, and positive validations
> to inform llm-harness-runtime v0.3 planning.

## Hooks Used

| Hook | Use Case | Verdict |
|------|---------|---------|
| `BeforeToolCallHook` | Human approval for sensitive tools | ✅ worked as designed |
| `PrepareNextTurnHook` | Historical PhaseManager active-tool filtering | ✅ worked; now replaced by workflow step tool scopes |
| `AfterProviderResponseHook` | BudgetControlAdapter cost accumulation | ✅ worked; now awaiting safer app-level budget policy |
| `ShouldStopHook` | BudgetControlAdapter loop stop policy | ⚠️ unsafe for ordinary one-turn chat with current semantics |

## Friction Points

- **Status update: runtime workflow support is now available and consumed**
  - `llm-tutor` now pins `llm-harness-runtime` to `eea964b`, which includes `workflow` and `spawn/subagent` modules. Current product flows consume `WorkflowEngine` plus the runtime JSONL session factory; no separate free-form subagent is needed for the migrated paths.
  - The old adapter pin conflict is resolved by aligning `llm-api-adapter` to the runtime-compatible revision.
  - First migration step: Deep Solve now defines its phase graph as an `llm_harness_runtime::workflow::model::Workflow` and validates it through `validate_workflow` before execution.
  - Second migration step: Quiz generation now defines its controlled product flow (`collect_sources -> generate_questions -> verify_questions -> publish_questions`) as a runtime `Workflow` and validates it through `validate_workflow` before generation.
  - Third migration step: ordinary Chat, Code Exec, and the existing Deep Solve phases now construct harnesses through runtime `HarnessBuilder` instead of manually assembling `AgentHarnessOptions`; product code only maps tools, prompts, and hooks into a thin builder config.
  - Fourth migration step: Deep Solve and Quiz workflow edges now use runtime-evaluable `EdgeCondition::Expr` predicates instead of legacy label strings, so the built-in `EdgeConditionJudge` can route them once execution moves to `WorkflowEngine`.
  - Fifth migration step: Quiz generation now performs a controlled verifier repair pass that mirrors the runtime workflow's `verify_questions -> generate_questions` repair edge while the full `WorkflowEngine` execution path is being adopted.
  - Sixth migration step: `llm-tutor` now has a thin `runtime_engine` adapter that builds `WorkflowEngineConfig` from the product `ExecutionEnv`, LLM client, model, and runtime JSONL session factory. A smoke test runs an executor workflow through runtime `WorkflowEngine`.
  - Seventh migration step: Quiz generation now has a product runtime workflow path. `collect_sources`, `generate_questions`, `verify_questions`, and `publish_questions` run as runtime executor steps, and the repair edge is driven by `WorkflowEngine` transitions. The web Quiz route and chat `create_quiz` tool now call this runtime workflow path when an LLM is configured.
  - Eighth migration step: Memory assist/update/check/dedupe now has a runtime `WorkflowEngine` path. The web memory route builds a runtime workflow config with a JSONL session root and runs the LLM-backed memory workflow through a registered runtime executor.
  - Ninth migration step: Deep Solve now runs through runtime `WorkflowEngine`. Product code registers a retrieve executor, product tools/hooks, and a thin event bridge; runtime owns the plan/solve/synthesize LLM step sessions, step history, `submit_step_result` routing, and workflow transitions.
  - Tenth migration step: Quiz generation and verification now run as runtime LLM steps. Product code only collects sources into workflow context, publishes the final validated questions, and enforces a bounded repair loop through a thin workflow judge; the model submits structured quiz and verifier results through runtime `submit_step_result`.
  - Eleventh migration step: Memory assist/update/check/dedupe now runs as a runtime LLM step. Product code prepares the memory prompt in workflow context and validates the submitted structured memory result; the model submits memory facts/edits through runtime `submit_step_result`.
  - Twelfth migration step: agent-side legacy direct structured-output helpers have been removed. Deep Solve, Quiz, and Memory now use runtime workflow/harness paths for LLM orchestration; product code keeps only domain validation, source repair, and runtime executor bridges.
  - Thirteenth migration step: app-side declarative edge evaluation has been removed. Deep Solve and Memory now pass a no-op marker into `WorkflowEngine::new`, allowing runtime's built-in declarative edge judge to own `EdgeCondition::Expr` routing.
  - Fourteenth migration step: legacy Deep Solve `PhaseManager`, `ReplanHook`, `ReplanTool`, and `SolveContext` have been removed. Replanning is now represented only as workflow structured output (`submit_step_result` with `route:"replan"`) and runtime edge transitions.
  - Fifteenth migration step: Quiz and Memory workflow APIs no longer accept duplicate client/model parameters; runtime client/model ownership now flows only through `WorkflowEngineConfig`.
  - Sixteenth migration step: upgraded all runtime crates to `eea964b` and verified `tutor-agent` / `tutor-web` compile against the latest runtime. The newest runtime still keeps `NoopJudge`, `EdgeConditionJudge`, and fixed-env helpers private, so the tiny product marker judge and env factory remain necessary thin adapters.
  - Remaining migration target: settings diagnostics still use a direct adapter probe because they are provider connectivity checks, not agent orchestration. Further cleanup depends on runtime/adapter support for provider-native structured LLM step options, public declarative/no-op judge helpers, typed validation/retry helpers, and normalized model metadata discovery.

- **Budget control still needs a safer runtime API**
  - Product code no longer constructs `BudgetControlAdapter` directly for ordinary harness setup; it now carries only the session budget limit in `GovernanceConfig`.
  - Attempting to wire `HarnessBuilder::budget(..., None)` into ordinary one-turn Chat/Code Exec harnesses still makes mock integration tests hang, matching the earlier `ShouldStopHook` semantic issue: `false` means "continue loop", not "allow this call and finish normally".
  - `WorkflowEngineConfig` exposes hooks and step cost aggregation, but does not yet expose a simple builder-style `budget(...)` / shared budget policy API for multi-step workflows.
  - Follow-up: add runtime budget helpers that distinguish per-call budget accounting from agent-loop stop decisions, and expose the same policy for ordinary harnesses and workflows.

- **`BeforeToolCallCtx` requires a live `AssistantMessage` reference, making unit tests noisy**
  - Expected: construct a minimal mock in test code to verify hook logic
  - Actual: `BeforeToolCallCtx` borrows `AssistantMessage`, preventing straightforward construction. Tests need `Box::leak` or an `Arc` with full field population just to test a simple allow/deny decision.
  - Suggestion: add a `BeforeToolCallCtx::new(name, args)` constructor for tests, or make `assistant_message` borrow `Option`al.

- **`AuditEntry` requires hash-chain fields (`hash`, `prev_hash`) that are internally managed by `JsonlAuditSink`**
  - Expected: callers should provide domain fields only, the sink fills in the chain
  - Actual: `AuditEntry` is public with required hash fields that callers must guess at. The sink then overwrites them. Unclear contract.
  - Suggestion: split into `AuditPayload` (caller-provided) and `AuditEntry` (sink-computed with hash chain), or make hash fields `Option` with `take()` semantics.

- **Resolved: repeated harness setup has been consolidated**
  - Earlier `SolveOrchestrator` phases repeated manual `AgentHarnessOptions` and harness setup code.
  - `llm-tutor` now uses runtime `HarnessBuilder` for ordinary capability harness setup and runtime `WorkflowEngine` for Deep Solve, Quiz generation, and Memory assist workflows.
  - Follow-up: reduce product-side bridge code as runtime exposes public declarative judges and typed structured-output helpers.

- **Session option/metadata types are not re-exported from the root facade**
  - Expected: common session types used with `JsonlSessionRepo` can be imported from `llm_harness_agent::{...}` or the prelude.
  - Actual: `JsonlSessionRepo`, `Session`, and `SessionRepo` are root exports, but `CreateSessionOptions`, `ListSessionOptions`, and `SessionMetadata` require `llm_harness_agent::session::{...}`.
  - Suggestion: re-export these common session types at the root/prelude for a smoother app-layer integration.

- **`SessionInfo` entries do not appear to update session metadata name via `Session::append`**
  - Expected: appending `SessionEntryPayload::SessionInfo { name }` updates `SessionMetadata.name`, matching the type comment that the most recent `SessionInfo` wins.
  - Actual: `Session::append` updates model metadata for `ModelChange`, but not name metadata for `SessionInfo`, so apps still need a separate title derivation path.
  - Suggestion: update metadata name in `Session::append` when a `SessionInfo` payload is appended, or expose a public high-level `Session::set_name`.

- **Resolved: runtime pin previously blocked downstream embedding usage**
  - Expected: once `llm-api-adapter` adds `EmbeddingProvider`, downstream apps can update and use it for RAG indexing.
  - Actual: `llm-harness-runtime` still depends on the older adapter revision, so `llm-tutor` cannot independently bump `llm_adapter` without ending up with two incompatible `Provider` traits in the dependency graph.
  - Suggestion: update `llm-harness-runtime` to the adapter revision that includes embedding support, then optionally re-export embedding traits/types from the runtime facade.

- **Resolved: latest runtime HEAD can now be consumed by Cargo**
  - Expected: pinning `llm-harness-runtime` to the latest commit should fetch cleanly as a git dependency.
  - Actual: commit `c6eba08` pulls submodule `examples/coding-agent` with URL `git@github.com:oh-my-harness/coding-agent.git`, which Cargo reports as an invalid relative URL.
  - Suggestion: use a valid absolute SSH URL such as `ssh://git@github.com/oh-my-harness/coding-agent.git`, or avoid requiring example submodules for library consumption.

- **Structured-output generation still needs app-level boilerplate**
  - Expected: product flows like quiz generation can ask the framework for typed JSON output with provider-aware schema support, retries, and validation error reporting.
  - Actual: runtime LLM steps can collect structured results through `submit_step_result`, but provider-native JSON schema response formats are not exposed at the workflow step level. The legacy direct helpers have been removed, so runtime workflow paths now place schema instructions in prompts and validate submitted JSON in product code.
  - Suggestion: add a runtime or agent helper such as `generate_structured<T>(prompt, schema/options)` that uses provider capabilities, validates typed output, and returns structured errors suitable for UI display.

- **Structured product flows cannot yet combine typed output with normal tool orchestration**
  - Expected: a product flow such as Quiz can ask the model to call tools like `read_memory`, then return validated typed JSON questions in one runtime-managed flow.
  - Actual: Chat, Research, Deep Solve, and Quiz runtime workflows can mount runtime tools, and Quiz now uses `submit_step_result` for structured quiz output. However, provider-native typed JSON schema, retries, and validation are still product-layer responsibilities.
  - Suggestion: add a runtime pattern for "tool-using structured generation", for example `AgentHarness::generate_structured_with_tools<T>()`, where tools, trace events, schema output, validation, and retries are all runtime-managed.

- **Default workflow judge helpers are still not public**
  - Expected: product code can construct a declarative `EdgeCondition::Expr` workflow through public runtime APIs without implementing any `StepTransitionJudge`.
  - Actual: runtime now auto-selects its built-in edge judge when the provided judge reports `is_noop()`, so `llm-tutor` no longer duplicates edge evaluation. However, runtime's `NoopJudge` / `EdgeConditionJudge` are still `pub(crate)`, so apps need a tiny marker judge solely to opt into the built-in behavior.
  - Suggestion: expose a `WorkflowEngine::new_declarative(workflow, config)` helper that selects the built-in edge judge automatically, or publish a small no-op marker constructor.

- **Model metadata discovery is still app-specific**
  - Expected: settings diagnostics and runtime context budgeting can ask the provider adapter for normalized model metadata such as context window, native embedding dimension, supported embedding dimensions, and detected source.
  - Actual: `llm-tutor` has to implement a thin `GET /models` probe, provider-specific auth headers, endpoint derivation, and recursive parsing of fields such as `context_window`, `max_context_tokens`, and `max_model_len`.
  - Suggestion: add an `llm-api-adapter` capability such as `list_models()` / `model_metadata(model)` and expose source labels (`metadata`, `known_model`, `default`) so apps do not duplicate provider quirks.

- **Resolved: compact summaries now read directly from runtime session entries**
  - Expected: UI-visible compaction summaries should come from runtime `SessionEntryPayload::Compaction` records.
  - Actual: `llm-tutor` previously wrote an additional custom `compact_summary` entry after each chat turn, which duplicated runtime-owned session/compaction state.
  - Change: removed the product-layer summary mirror. Session detail responses still expose `compact_summary`, but it is now derived only from the latest runtime compaction entry.

## Positive Validations

- **CompositeBeforeToolCallHook** can layer domain-specific + cross-cutting hooks; current product code only needs human approval hooks after moving replan into workflow routing
- **BudgetControlAdapter** dual-role as `AfterProviderResponseHook` + `ShouldStopHook` is elegant — one instance, two contracts
- **`active_tools` in `NextTurnDirective`** was useful for historical PhaseManager-style filtering; current workflow paths prefer runtime step `allowed_tools`
- **`HarnessHooks::none()`** pattern with struct update syntax (`..HarnessHooks::none()`) makes selective hook wiring readable
- **`AgentHarness::subscribe()` before `prompt()`** pattern allows reliable event collection without race conditions

## API Gaps

| Gap | Description | Severity |
|-----|-------------|----------|
| No test-helper constructors for hook context types | Building `BeforeToolCallCtx` in tests is unnecessarily hard | Medium |
| Session options/metadata missing from root/prelude exports | Apps need mixed import paths for common session operations | Low |
| SessionInfo does not update metadata name | Session titles need app-layer workaround | Medium |
| AuditEntry hash fields leak implementation detail | Callers must provide hash-chain fields that the sink overwrites | Low |
| WorkflowEngine migration bridge still needs product adapters | Existing product flows need event bridges, executor state mapping, and structured `submit_step_result` prompts before they can stop using direct phase loops | Medium |
| No typed structured-output helper | Product flows must duplicate JSON extraction, schema hints, validation, and retry policy | Medium |
| No tool-using structured-generation helper | Product flows such as Quiz cannot combine `read_memory` tool orchestration with typed JSON output without a parallel loop | Medium |
| No public declarative/no-op workflow judge helper | Product workflows need a tiny marker judge to opt into runtime's built-in declarative edge router | Low |
| No safe app-level budget policy helper | Ordinary one-turn harnesses and multi-step workflows cannot share budget accounting without app-layer loop-risk or hook boilerplate | Medium |
| No normalized model metadata API | Apps duplicate `/models` probing, auth headers, context-window parsing, and embedding dimension capability discovery | Medium |

## Proposed v0.3 Changes

1. Add `BeforeToolCallCtx::new_test(name, args)` that uses a dummy assistant message internally
2. Make `AuditEntry.hash` and `AuditEntry.prev_hash` optional with internal fill-in, or split into payload vs entry types
3. Continue documenting `HarnessBuilder` examples for app-layer harness factories and plugin-based hook injection
4. Re-export common session repo option and metadata types from the facade/prelude
5. Add `Session::set_name` or metadata updates for `SessionInfo`
6. Add a typed structured-output helper for provider-aware JSON/schema generation
7. Add a tool-using structured-generation helper for flows like memory-aware Quiz
8. Add normalized model metadata discovery in the adapter/runtime boundary
9. Expose a public no-op/declarative workflow judge helper or `WorkflowEngine::new_declarative` constructor
10. Continue hardening `WorkflowEngine` examples for app-level workflows that mix executor steps, LLM steps, and subagent reviewers
11. Add a safe app-level budget helper/policy API that separates cost accounting from loop continuation
