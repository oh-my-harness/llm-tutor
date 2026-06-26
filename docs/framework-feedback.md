# llm-harness-runtime v0.2 Framework Feedback

> Written after implementing tutor_agent v0.1 (2026-06-13).
> This document records friction points, missing APIs, and positive validations
> to inform llm-harness-runtime v0.3 planning.

## Hooks Used

| Hook | Use Case | Verdict |
|------|---------|---------|
| `BeforeToolCallHook` | ReplanHook intercepts `replan()` | ✅ worked as designed |
| `PrepareNextTurnHook` | PhaseManager sets active_tools per step | ✅ worked as designed |
| `AfterProviderResponseHook` | BudgetControlAdapter accumulates cost | ✅ worked as designed |
| `ShouldStopHook` | BudgetControlAdapter hard-stops loop | ✅ worked as designed |

## Friction Points

- **`BeforeToolCallCtx` requires a live `AssistantMessage` reference, making unit tests noisy**
  - Expected: construct a minimal mock in test code to verify hook logic
  - Actual: `BeforeToolCallCtx` borrows `AssistantMessage`, preventing straightforward construction. Tests need `Box::leak` or an `Arc` with full field population just to test a simple allow/deny decision.
  - Suggestion: add a `BeforeToolCallCtx::new(name, args)` constructor for tests, or make `assistant_message` borrow `Option`al.

- **`AuditEntry` requires hash-chain fields (`hash`, `prev_hash`) that are internally managed by `JsonlAuditSink`**
  - Expected: callers should provide domain fields only, the sink fills in the chain
  - Actual: `AuditEntry` is public with required hash fields that callers must guess at. The sink then overwrites them. Unclear contract.
  - Suggestion: split into `AuditPayload` (caller-provided) and `AuditEntry` (sink-computed with hash chain), or make hash fields `Option` with `take()` semantics.

- **`SolveOrchestrator` patterns repeat harness setup code across four phases**
  - Not a framework bug, but notable: every harness call repeats `AnthropicProvider::builder`, `AgentHarness::new_in_memory`, `subscribe`, event loop. A shared harness factory or builder pattern would reduce boilerplate.

- **Session option/metadata types are not re-exported from the root facade**
  - Expected: common session types used with `JsonlSessionRepo` can be imported from `llm_harness_agent::{...}` or the prelude.
  - Actual: `JsonlSessionRepo`, `Session`, and `SessionRepo` are root exports, but `CreateSessionOptions`, `ListSessionOptions`, and `SessionMetadata` require `llm_harness_agent::session::{...}`.
  - Suggestion: re-export these common session types at the root/prelude for a smoother app-layer integration.

- **`SessionInfo` entries do not appear to update session metadata name via `Session::append`**
  - Expected: appending `SessionEntryPayload::SessionInfo { name }` updates `SessionMetadata.name`, matching the type comment that the most recent `SessionInfo` wins.
  - Actual: `Session::append` updates model metadata for `ModelChange`, but not name metadata for `SessionInfo`, so apps still need a separate title derivation path.
  - Suggestion: update metadata name in `Session::append` when a `SessionInfo` payload is appended, or expose a public high-level `Session::set_name`.

- **Runtime pins an older `llm-api-adapter`, blocking downstream embedding usage**
  - Expected: once `llm-api-adapter` adds `EmbeddingProvider`, downstream apps can update and use it for RAG indexing.
  - Actual: `llm-harness-runtime` still depends on the older adapter revision, so `llm-tutor` cannot independently bump `llm_adapter` without ending up with two incompatible `Provider` traits in the dependency graph.
  - Suggestion: update `llm-harness-runtime` to the adapter revision that includes embedding support, then optionally re-export embedding traits/types from the runtime facade.

- **Latest runtime HEAD cannot be consumed by Cargo because of an invalid submodule URL**
  - Expected: pinning `llm-harness-runtime` to the latest commit should fetch cleanly as a git dependency.
  - Actual: commit `c6eba08` pulls submodule `examples/coding-agent` with URL `git@github.com:oh-my-harness/coding-agent.git`, which Cargo reports as an invalid relative URL.
  - Suggestion: use a valid absolute SSH URL such as `ssh://git@github.com/oh-my-harness/coding-agent.git`, or avoid requiring example submodules for library consumption.

- **Structured-output generation still needs app-level boilerplate**
  - Expected: product flows like quiz generation can ask the framework for typed JSON output with provider-aware schema support, retries, and validation error reporting.
  - Actual: `llm-tutor` has to call `llm_adapter::ResponseFormat` directly, extract JSON from text, deserialize it, and implement validation/retry policy in product code.
  - Suggestion: add a runtime or agent helper such as `generate_structured<T>(prompt, schema/options)` that uses provider capabilities, validates typed output, and returns structured errors suitable for UI display.

- **Structured product flows cannot yet combine typed output with normal tool orchestration**
  - Expected: a product flow such as Quiz can ask the model to call tools like `read_memory`, then return validated typed JSON questions in one runtime-managed flow.
  - Actual: Chat, Research, and Deep Solve can mount `read_memory` through the harness, but Quiz currently uses a direct structured-output API path. `llm-tutor` can pass L3 memory as product context for personalization, but it cannot let the model truly decide to call `read_memory` without migrating Quiz into a custom parallel agent loop.
  - Suggestion: add a runtime pattern for "tool-using structured generation", for example `AgentHarness::generate_structured_with_tools<T>()`, where tools, trace events, schema output, validation, and retries are all runtime-managed.

- **Model metadata discovery is still app-specific**
  - Expected: settings diagnostics and runtime context budgeting can ask the provider adapter for normalized model metadata such as context window, native embedding dimension, supported embedding dimensions, and detected source.
  - Actual: `llm-tutor` has to implement a thin `GET /models` probe, provider-specific auth headers, endpoint derivation, and recursive parsing of fields such as `context_window`, `max_context_tokens`, and `max_model_len`.
  - Suggestion: add an `llm-api-adapter` capability such as `list_models()` / `model_metadata(model)` and expose source labels (`metadata`, `known_model`, `default`) so apps do not duplicate provider quirks.

## Positive Validations

- **CompositeBeforeToolCallHook** chains ReplanHook and HumanApprovalWrapper cleanly — allows layering domain-specific + cross-cutting hooks
- **BudgetControlAdapter** dual-role as `AfterProviderResponseHook` + `ShouldStopHook` is elegant — one instance, two contracts
- **`active_tools` in `NextTurnDirective`** is exactly the right granularity for PhaseManager — not `tools` (replace entire set) but a subset filter
- **`HarnessHooks::none()`** pattern with struct update syntax (`..HarnessHooks::none()`) makes selective hook wiring readable
- **`AgentHarness::subscribe()` before `prompt()`** pattern allows reliable event collection without race conditions

## API Gaps

| Gap | Description | Severity |
|-----|-------------|----------|
| No test-helper constructors for hook context types | Building `BeforeToolCallCtx` in tests is unnecessarily hard | Medium |
| Session options/metadata missing from root/prelude exports | Apps need mixed import paths for common session operations | Low |
| SessionInfo does not update metadata name | Session titles need app-layer workaround | Medium |
| Runtime adapter pin lacks embedding support | Downstream RAG work cannot use new adapter embedding APIs while sharing harness provider traits | High |
| Latest runtime HEAD has an invalid example submodule URL | Cargo cannot consume `c6eba08` as a git dependency | High |
| AuditEntry hash fields leak implementation detail | Callers must provide hash-chain fields that the sink overwrites | Low |
| No shared harness builder in the public API | Every call site repeats `new_in_memory`, `subscribe`, event loop | Low |
| No typed structured-output helper | Product flows must duplicate JSON extraction, schema hints, validation, and retry policy | Medium |
| No tool-using structured-generation helper | Product flows such as Quiz cannot combine `read_memory` tool orchestration with typed JSON output without a parallel loop | Medium |
| No normalized model metadata API | Apps duplicate `/models` probing, auth headers, context-window parsing, and embedding dimension capability discovery | Medium |

## Proposed v0.3 Changes

1. Add `BeforeToolCallCtx::new_test(name, args)` that uses a dummy assistant message internally
2. Make `AuditEntry.hash` and `AuditEntry.prev_hash` optional with internal fill-in, or split into payload vs entry types
3. Consider adding `AgentHarnessBuilder` that caches provider/client construction and event subscription setup
4. Re-export common session repo option and metadata types from the facade/prelude
5. Add `Session::set_name` or metadata updates for `SessionInfo`
6. Align `llm-harness-runtime` with the adapter revision that includes `EmbeddingProvider`
7. Fix example submodule URLs so latest runtime commits can be consumed as git dependencies
8. Add a typed structured-output helper for provider-aware JSON/schema generation
9. Add a tool-using structured-generation helper for flows like memory-aware Quiz
10. Add normalized model metadata discovery in the adapter/runtime boundary
