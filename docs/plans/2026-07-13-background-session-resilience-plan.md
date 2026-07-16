# Background Session Resilience Plan

> Status: partially implemented | Date: 2026-07-13 | Last updated: 2026-07-16 | Scope: preserve long-running agent tasks, interactive chat cards, and workflow progress when the user leaves and later returns to a session.

## 1. Problem

Some product actions create UI that is attached to the current live chat stream
rather than restored from a durable session projection. For example, a generated
Quiz can be saved into Quiz Bank, but the interactive Quiz card in the Chat
message can disappear after the user navigates away and returns. Longer-running
flows such as Research and Quiz generation have the same structural
risk: the backend task may still be running, but the UI cannot reliably rejoin
the in-flight stream or reconstruct the current result surface.

The product should support users who start a slow agent task, switch to another
session or workspace task, and later come back without losing visible progress,
tool results, interactive cards, or final artifacts.

## 2. Product Requirements

- Chat message content, tool results, workflow progress, and interactive
  attachments shall be represented as durable session state, not only as
  in-memory React state created during the active WebSocket stream.
- Interactive Chat cards, including Quiz cards and Research report cards, shall
  be restorable from stored product records and message-to-record links after
  navigation, refresh, or app restart.
- Long-running agent turns shall continue to have a durable run identity after
  the user leaves the current session.
- Returning to a session with an in-flight run shall show the current run state:
  queued, running, waiting for user input, failed, cancelled, or completed.
- Returning to an in-flight session shall restore the assistant text generated
  so far, then continue streaming subsequent deltas without duplication or a
  blank-message reset.
- Each session shall retain its own last reading position. Returning to a
  session restores that position; sessions last viewed at the bottom continue
  following new output, while readers inspecting older messages are not pulled
  away by background streaming.
- WebSocket events and session-detail responses shall be applied only to the
  session that produced them. Rapid A/B/A switching must not let a stale socket
  event or slower HTTP response overwrite the currently selected conversation.
- The UI shall be able to rejoin or resubscribe to progress for an active run
  without starting a duplicate agent turn.
- Completed tool results shall remain attached to the originating assistant
  message even if the user switches sessions before the outer agent produces a
  final short response.
- Product records such as Quiz sessions and Notebook research reports remain
  the source of truth for domain artifacts. Chat messages should store stable
  references to those artifacts, not large duplicated copies unless a snapshot
  is needed for historical fidelity.
- The implementation shall prefer runtime session/run support from
  `llm-harness-runtime` / `llm-harness-agent`. If the framework lacks a needed
  durable run or rejoin primitive, record the gap in `docs/framework-feedback.md`
  before adding a product-side bridge.

## 3. Target Model

Each long-running assistant turn should have a durable run envelope:

```text
Session
  Message[]
    assistant message
      content parts
      attachment refs
      tool result refs
      workflow progress refs
  Run[]
    run id
    session id
    active assistant message id
    capability
    status
    started/updated/completed timestamps
    current stage
    cancellation state
```

Domain artifacts stay in their existing stores:

```text
Quiz card attachment -> quiz_session:<quiz_id>
Research report attachment before save -> runtime_trace:<run_id>
Research report attachment after save -> notebook_entry:<entry_id>
Historical Deep Solve attachment -> legacy trace or message snapshot
```

On session load, the UI should hydrate messages first, then resolve attachment
references through product APIs. If an attachment cannot be resolved, the
message should show a clear unavailable state instead of silently dropping the
card.

## 4. Workflow Rejoin Behavior

When a user returns to a session:

1. Load durable session messages and run envelopes.
2. Render any completed assistant text and attachment references.
3. For active runs, open or reuse a subscription keyed by `run_id`.
4. Append new progress and message deltas to the existing assistant message.
5. If the run already completed while the user was away, fetch final run output
   and attach completed artifacts without replaying the whole workflow.
6. If the app restarted, recover active or terminal run state from runtime
   session storage. A run that was active when the process stopped is restored
   as `interrupted` until the runtime provides execution resume/replay support;
   product code does not invent a separate scheduler.

## 5. Implementation Phases

### Phase 1: Audit Current Volatile UI State

- [x] Trace how Quiz cards are attached to Chat messages during live
  `create_quiz` tool results.
- [x] Confirm whether restored Chat messages include stable quiz attachment
  references or only text.
- [x] Trace Research report restore behavior for completed reports and active
  report generation.
- [x] Identify every UI attachment type that currently depends on transient
  WebSocket-only state.

### Phase 2: Durable Message Attachments

- [x] Add or confirm a normalized assistant message attachment shape with
  `type`, `artifact_id`, `artifact_store`, and optional display snapshot.
- [x] Persist Quiz card attachments as references to saved Quiz sessions.
- [x] Persist Research report attachments as references to durable runtime trace
  entries before the user optionally saves them to Notebook.
- [x] Hydrate attachments on session load and show fallback UI when referenced
  artifacts are missing.
- [x] Add regression tests that create a Quiz through Chat, reload the session,
  and verify the interactive card is restored.

### Phase 3: Durable Run Envelopes

Active runs are tracked in-process by session id and run id, exposed on session
load, block duplicate starts, and are not cancelled by WebSocket disconnect.
Run status and current stage are also written to runtime session custom entries.
After an app/sidecar restart, a previously active run is restored as
`interrupted`; execution resume remains blocked on a runtime rejoin primitive.

- [x] Map runtime run/session identifiers into product session state.
- [x] Persist run status transitions and current stage for long-running turns.
- [x] Expose an API for session load to return the active run.
- [x] Ensure switching sessions does not cancel active runs unless the user
  explicitly cancels them.
- [x] Add cancellation and failure surfaces for active in-process runs.

### Phase 4: Progress Rejoin

- [x] Let the WebSocket bridge subscribe by `session_id` and active `run_id`.
- [x] Avoid duplicate workflow starts when the UI reconnects.
- [x] Backfill completed Research report metadata from durable trace/session
  entries and restore the latest persisted run stage where
  available.
- [x] Show a compact "still running" state when progress cannot be replayed but
  the backend run is active.
- [x] Keep an in-process per-session stream snapshot and atomically pair snapshot
  capture with live subscription so generated assistant text can be restored
  after switching away and back.
- [x] Mark completed stream snapshots and use a one-shot canonical history
  resync across the narrow handoff window where a run settles after session
  history loading but before the UI resubscribes.
- [x] Reconcile durable history with live WebSocket messages received during
  hydration instead of allowing a slower history response to replace them.
- [x] Persist and restore per-session Chat reading positions without defeating
  intentional bottom-follow behavior for new messages.
- [x] Tag WebSocket callbacks with their source session and reject stale events
  after rapid session switching.
- [x] Guard session-detail hydration against out-of-order HTTP responses.
- [x] Reconcile sidebar running indicators against the backend while any
  background run remains active.
- [x] Let the Memory workspace discover active runs from the backend and restore
  the target document, flow progress, and pending review after page remount.

### Phase 5: Cross-Mode QA

- [ ] Quiz: start generation, switch sessions, return, verify the card appears
  and remains answerable.
- [ ] Research: start a detailed report, switch sessions, return, verify
  current stage and final report attachment.
- [x] Retire new Deep Solve runs while preserving old trace/message hydration.
- [ ] Desktop restart: verify completed artifacts are restored and active runs
  resolve to a clear resumed, failed, or unavailable state.

## 6. Acceptance Criteria

- A generated Quiz card remains visible and usable after leaving and returning
  to the Chat session.
- The same Quiz is visible in Space / Quiz Bank and the Chat card links to that
  durable quiz record.
- A long-running Research task can continue while the user works
  in another session.
- Returning to the original session shows a coherent run state without starting
  a duplicate run.
- App refresh or desktop restart never silently drops completed tool results or
  interactive cards from the visible conversation.
