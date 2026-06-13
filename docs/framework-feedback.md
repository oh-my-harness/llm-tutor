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
| AuditEntry hash fields leak implementation detail | Callers must provide hash-chain fields that the sink overwrites | Low |
| No shared harness builder in the public API | Every call site repeats `new_in_memory`, `subscribe`, event loop | Low |

## Proposed v0.3 Changes

1. Add `BeforeToolCallCtx::new_test(name, args)` that uses a dummy assistant message internally
2. Make `AuditEntry.hash` and `AuditEntry.prev_hash` optional with internal fill-in, or split into payload vs entry types
3. Consider adding `AgentHarnessBuilder` that caches provider/client construction and event subscription setup
