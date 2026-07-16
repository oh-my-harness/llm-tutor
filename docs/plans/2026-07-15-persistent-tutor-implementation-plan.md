# Persistent Tutor Implementation Plan

> Status: in progress | Date: 2026-07-15 | Last updated: 2026-07-16 | Target: v0.3.1 stabilization and post-release development
>
> Product design: `../specs/2026-07-15-persistent-tutor-design.md`

Implementation progress (2026-07-16): the Phase 0/1 identity loop, bounded Soul
runtime context, tutor default-model resolution, and server-enforced resource
policy are implemented. Tutor CRUD, General Tutor seeding, immutable session
binding, the optional Chat chooser, session restoration, Tutor management, and
private Tutor Memory are released in `v0.3.1`. Private commitments, open
loops, lesson plans, reflections, and strategies are isolated per Tutor,
tool-readable, user-manageable in the continuity view, and routed separately
from shared Learner Memory by tool-aware runtime instructions. Recent-tutor
ranking, avatar presentation, handoff, stronger autonomous-write content
policy, and settings-deletion protection for a model referenced by a tutor
remain pending.

## 1. Objective

Implement persistent tutors as the identity layer that connects conversations,
capabilities, resources, Learner Memory, and tutor-private continuity memory.

The first complete slice must let a user:

1. create or select a tutor from the new-conversation screen;
2. start a normal runtime-backed conversation bound to that tutor;
3. restart the application and restore the same tutor identity;
4. receive answers shaped by the tutor Soul and authorized context;
5. inspect and manage the tutor from the Tutor workspace.

Chat, Research, Quiz, and Deep Solve remain capabilities. A tutor does not own a
parallel message engine, workflow engine, or session store.

## 2. Current Baseline

The implementation starts from these existing boundaries:

- `SessionPool` owns product metadata beside `JsonlSessionRepo` runtime
  sessions in `crates/tutor-web/src/session.rs`.
- `POST /api/sessions` creates the runtime session and records capability,
  resource, model, and search configuration in
  `crates/tutor-web/src/routes/sessions.rs`.
- `crates/tutor-web/src/routes/ws.rs` assembles the current capability router,
  product tools, and runtime execution path.
- `web-ui/src/App.tsx` owns session creation, restoration, switching, and the
  Chat empty state.
- `web-ui/src/components/Sidebar.tsx` renders conversation state and navigation.
- `TutorStore`, Tutor CRUD routes, the Chat chooser, and the initial `TutorPage`
  now provide the persistent identity surface.

This is a favorable boundary: tutor identity can remain product metadata while
`llm-harness-runtime` continues to own durable conversation history, context,
tool calls, traces, compaction, and workflow execution.

## 3. Fixed Architecture Decisions

### 3.1 Storage

Add a product-owned store rooted at:

```text
<data-dir>/tutors/
  tutors.json
  <tutor-id>/
    memory.json
```

`tutors.json` contains profile and permission configuration. `memory.json`
contains structured private Tutor Memory. Writes use temporary-file plus rename
semantics, matching the durability expectations of the other product stores.

Existing sessions are not rewritten. A missing `tutor_id` means Temporary
Assistant, preserving current Chat behavior.

### 3.2 Tutor Identity

The first schema is:

```text
TutorProfile
  id
  name
  soul_markdown
  avatar
  default_model_config_id?
  default_capability
  allowed_capabilities[]
  learner_memory_access
  resource_permissions
  autonomous_memory
  built_in
  created_at
  updated_at
```

The store seeds one built-in General Tutor on first use. It may be edited and
reset to defaults, but not deleted. User-created tutors are currently archived
through `DELETE /api/tutors/:id`; permanent deletion remains future work and
must report affected sessions before asking the UI for an explicit retention
choice.

### 3.3 Session Binding

Add `tutor_id: Option<String>` to `ProductSessionMetadata` and `SessionEntry`.
The value is set at session creation and is immutable. `PATCH /api/sessions/:id`
must reject attempts to change it.

Changing tutors always creates a new runtime session. It never swaps Soul or
private memory inside an existing history.

### 3.4 Configuration Precedence

Resolve settings in this order:

```text
explicit new-conversation choice
  > tutor default
  > global application default
```

The resolved model configuration is copied into session product metadata at
creation time. Later tutor setting changes do not silently alter an active
session.

### 3.5 Permissions

Permissions are enforced in product code, not only described in prompts:

- reject a session capability outside `allowed_capabilities`;
- mount only tools allowed by tutor resource permissions;
- filter Knowledge IDs before retrieval;
- expose Learner Memory tools only when `learner_memory_access` is true;
- expose Tutor Memory tools only for the session's own tutor;
- keep Temporary Assistant behavior explicit and separate.

### 3.6 Context

For tutor-bound turns, product code supplies a compact tutor context to the
existing runtime path:

```text
stable Tutor Soul
  + current permission summary
  + small active commitment/open-loop summary
  + authorized on-demand memory tools
  + runtime session history
```

Complete Learner Memory and Tutor Memory files are not injected into every
turn. Retrieval remains on demand.

## 4. Implementation Phases

### Phase 0: Contract and Store Foundation

Goal: establish a tested product domain before changing conversation behavior.

Backend tasks:

- [x] Add `crates/tutor-web/src/tutor_store.rs` with typed profile, permission,
  and validation models. Private memory models remain Phase 4 work.
- [x] Seed the built-in General Tutor idempotently.
- [x] Add atomic create, list, get, update, archive, and reset operations.
- [ ] Add explicit permanent deletion with affected-session preview and
  retention choice.
- [x] Generate stable unique IDs and reject blank Souls and unknown
  capabilities.
- [x] Reject dangling model configuration IDs on Tutor create/update and
  session resolution.
- [ ] Prevent Settings from deleting a model configuration still referenced by
  a tutor, or require an explicit replacement/clear operation.
- [x] Add `crates/tutor-web/src/routes/tutors.rs` and mount it from `main.rs`.
- [x] Expose `GET/POST /api/tutors` and
  `GET/PATCH/DELETE /api/tutors/:id`.
- [x] Expose `POST /api/tutors/:id/reset-profile` separately from memory reset.

Frontend contract tasks:

- [x] Add tutor DTOs and pure mapping helpers outside the main App component.
- [ ] Add localized validation and API error labels.

Tests:

- [x] Store restart round trip and built-in seed idempotence.
- [x] Validation and non-deletable built-in tutor tests.
- [x] Route tests for CRUD status codes and redacted configuration output.

Exit criteria:

- Tutor CRUD survives a process restart.
- The API cannot produce an invalid tutor profile.
- No session or WebSocket behavior has changed yet.

### Phase 1: Persistent Identity and Session Binding

Goal: complete the smallest end-to-end tutor-bound conversation path.

Backend tasks:

- [x] Add immutable `tutor_id` to `SessionEntry`, `ProductSessionMetadata`, and
  session list/detail responses.
- [x] Extend `POST /api/sessions` with optional `tutor_id` and validate it against
  `TutorStore`.
- [x] Resolve tutor default capability only during session creation.
- [x] Resolve tutor default model only during session creation.
- [x] Reject disallowed capabilities before creating a runtime session.
- [x] Restore `tutor_id` through `SessionPool::ensure_entry` after restart.
- [x] Return tutor summary data with session list items to avoid frontend N+1
  requests.
- [x] Keep sessions without `tutor_id` as Temporary Assistant sessions.

Frontend tasks:

- [x] Add a compact tutor chooser to the empty Chat state.
- [x] Show all tutors, a management/create action, and an implicit Temporary
  Assistant default; clicking the selected tutor again clears the selection.
- [ ] Add recent-tutor ordering and continuity summaries.
- [x] Carry the selected `tutor_id` through deferred session creation on first
  send.
- [x] Display tutor name on the active conversation header.
- [ ] Display tutor identity in session rows and add avatar presentation.
- [x] Restore the selected tutor when reopening a session.

Tests:

- [x] Session metadata round trip with and without `tutor_id`.
- [x] Unknown tutor request test.
- [x] Reject disallowed capability and resource requests during session create,
  session update, and runtime execution.
- [x] Frontend helper tests for tutor selection and create-session payloads.
- [ ] Regression test proving existing unbound sessions still open normally.

Exit criteria:

- A user can choose a tutor, send a message, restart the app, and reopen the
  session with the same tutor identity.
- Tutor identity cannot be changed in place.
- Temporary Assistant remains a one-click path.

### Phase 2: Tutor Runtime Context and Capability Policy

Goal: make tutor identity affect agent behavior through the existing runtime,
not only through UI labels.

Backend tasks:

- [x] Pass `TutorStore` into `ws_router` and load the bound tutor before each
  run so archived/deleted configuration cannot be used silently.
- [x] Add a small product-instruction adapter that maps profile Soul into
  the existing `CapabilityRouter`/runtime invocation.
- [x] Apply bounded Tutor Soul Markdown as stable product instructions.
- [x] Keep current learning goals and plans in Tutor Memory rather than the
  stable tutor profile.
- [x] Filter mounted product tools according to capability and resource policy,
  including Learner Memory, Notebook, Space, Quiz sources, and Deep Solve
  workflow declarations.
- [x] Validate runtime capability changes against the bound tutor policy.
- [x] Record `tutor_id` in relevant trace/run metadata for diagnosis without
  exposing private memory contents.
- [ ] Document any runtime API limitation in `docs/framework-feedback.md`
  instead of creating a parallel context builder.

Tests:

- [x] Boundary test that Soul reaches the runtime instruction layer once.
- [ ] Complete the tool-mount matrix for Tutor, Temporary Assistant, and denied
  access. Learner Memory, session-resource denial, and direct Quiz source
  bypass cases are covered.
- [x] Capability and resource changes on existing Tutor sessions are validated
  server-side.
- [ ] Research and Quiz workflow trigger regression tests under a tutor-bound
  session.

Exit criteria:

- Different tutors produce different stable Soul context while sharing the same
  Chat/Research/Quiz/Deep Solve implementation.
- A prompt cannot bypass capability or resource restrictions.

### Phase 3: Tutor Workspace and Conversation UX

Goal: replace the Tutor placeholder with a useful management surface.

Frontend tasks:

- [x] Add the initial `TutorPage` with a compact tutor rail and profile editor.
- [ ] Add the tutor conversation list and compact continuity panel.
- [x] Add create/edit forms for Markdown Soul, default capability, allowed
  capabilities, and memory policy.
- [x] Add default-model and resource-permission controls.
- [ ] Show active/background run state beside tutor conversations.
- [ ] Add quick actions for continuing a recent conversation and resetting the
  built-in profile. Starting a conversation and editing are implemented.
- [x] Preserve current desktop layout rules: pane-local scrolling, keyboard
  access, no nested cards, and no browser-like context behavior.
- [ ] Add empty, loading, error, archived, and deleted-tutor states.

Integration tasks:

- [ ] Move tutor fetching and mutation state out of `App.tsx` into a focused
  hook/service module.
- [ ] Reuse the same tutor card/identity primitives in Chat empty state,
  Sidebar, and Tutor workspace.
- [ ] Add Chinese and English UI strings; generated tutor content keeps the
  user's language rather than being retroactively translated.

Tests:

- [ ] Pure state tests for selection, recent ordering, and deleted tutor state.
- [ ] Component/browser checks at minimum desktop size and a smaller supported
  viewport.
- [ ] Keyboard navigation and focus-visible checks for tutor selection.

Exit criteria:

- The Tutor navigation item is no longer a placeholder.
- Tutor management and conversation entry use one shared data model.

### Phase 4: Private Tutor Memory

Goal: let each tutor continue commitments, open loops, plans, reflections, and
strategy across multiple sessions.

Backend tasks:

- [x] Implement typed entries for `commitment`, `open_loop`, `lesson_plan`,
  `reflection`, and `strategy` with status, provenance, timestamps, and optional
  due/next-action fields.
- [x] Add scoped operations to list, read, create, update, resolve, delete, and
  reset entries for one tutor.
- [x] Add product tools `read_tutor_memory`, `remember_for_later`, and
  `resolve_tutor_memory`; never expose a generic filesystem tool.
- [x] Mount these tools only for a tutor-bound session and hard-bind tool scope
  to that session's `tutor_id`.
- [ ] Allow autonomous writes only for the low-risk categories defined by the
  design; credentials, sensitive personal data, unsupported judgments, and
  external factual claims are rejected. Tool descriptions carry this contract,
  but hard content-policy validation remains pending.
- [x] Provide a compact active commitments/open-loops summary for turn start;
  full content remains tool-read on demand.
- [x] Build one runtime memory-routing policy from the memory tools actually
  mounted for Chat, Research, Quiz, Organize, and Deep Solve. Keep learner
  facts and tutor-owned continuity separate, forbid duplicate writes, and omit
  unavailable tool names from prompts.

Frontend tasks:

- [x] Show private memory in the Tutor continuity view with type and state.
- [x] Support inspect, edit, resolve/reopen, delete, and reset actions.
- [x] Clearly distinguish Tutor Memory from shared Learner Memory.
- [x] Show source session provenance when available.

Tests:

- [x] Cross-tutor isolation tests at store, route, and tool boundaries.
- [x] Restart persistence and Tutor-scoped reset tests.
- [ ] Autonomous-memory allow/deny policy tests.
- [ ] Two-session continuity test for one tutor and isolation test for another.

Exit criteria:

- The same tutor can resume an unresolved learning thread in a new session.
- Another tutor cannot read or mutate that private thread.
- Resetting Tutor Memory leaves Learner Memory and learning assets untouched.

### Phase 5: Resource Permissions and Handoff

Goal: finish controlled resource access and safe tutor switching.

Backend tasks:

- [x] Enforce Knowledge allowlists and Notebook, Space, and Learner Memory
  booleans at current product-tool boundaries.
- [ ] Add an API that previews effective permissions before saving a tutor.
- [ ] Add handoff preview and execute endpoints.
- [ ] Use runtime session summary/compaction APIs to prepare a bounded handoff;
  do not implement a second transcript summarizer in product code.
- [ ] Let the user select open loops and artifact references to transfer.
- [ ] Create a fresh destination runtime session with an immutable destination
  `tutor_id` and a single bounded handoff context entry.
- [ ] Keep source and destination Tutor Memory separate; selected transfer items
  are copied with provenance rather than shared by reference.

Frontend tasks:

- [x] Add permission controls to Tutor settings using existing model and
  resource selectors.
- [ ] Add handoff preview with destination tutor, bounded summary, open loops,
  and artifact selection.
- [ ] Navigate to the newly created destination conversation after success.
- [ ] Show recent tutor, open-loop count, and background-run indicators in the
  chooser and Tutor workspace.

Tests:

- [ ] Permission matrix tests for every product resource tool.
- [ ] Handoff size bound, provenance, cancellation, and failure atomicity tests.
- [ ] Regression test proving the source session and memory remain unchanged.

Exit criteria:

- Tutor permissions are effective enforcement, not advisory UI.
- Handoff creates a new session and never mutates tutor identity in place.
- Only user-selected context crosses the tutor boundary.

## 5. Delivery Order and Commit Boundaries

Use one reviewable commit per completed boundary:

1. `feat(tutor): add persistent tutor store and API`
2. `feat(tutor): bind tutor identity to runtime sessions`
3. `feat(tutor): apply tutor context and capability policy`
4. `feat(ui): add tutor chooser and management workspace`
5. `feat(tutor): add private continuity memory`
6. `feat(tutor): enforce resources and support handoff`

Do not combine the Tutor Memory implementation with the initial session schema
change. The identity loop must be stable before autonomous memory writes are
introduced.

## 6. Verification Matrix

Run after each backend phase:

```powershell
cargo fmt --all -- --check
cargo test -p tutor-web
cargo clippy -p tutor-web --all-targets -- -D warnings
```

Run after each frontend phase:

```powershell
Set-Location web-ui
npm test
npm run build
```

Before declaring the feature complete:

- run `cargo tauri dev` with a clean temporary data directory;
- create two tutors and one Temporary Assistant conversation;
- verify Chat streaming, Research workflow launch, Quiz card restoration, and
  Deep Solve under tutor-bound sessions;
- switch sessions during a background run and verify run markers settle;
- restart the desktop app and verify tutor/session/private-memory restoration;
- verify cross-tutor memory and resource denial in both API and UI;
- inspect desktop screenshots at the minimum supported window size;
- update `README.md`, `MANUAL.md`, PRD status, roadmap checkboxes, and desktop QA
  documentation before release.

## 7. Feature Completion Gates

The persistent Tutor roadmap is not complete until all of these hold. The
`v0.3.1` release contains the implemented core while the remaining gates stay
visible as post-release hardening work:

- no tutor path creates a product-owned replacement for runtime sessions;
- existing unbound sessions remain readable as Temporary Assistant sessions;
- tutor identity is immutable per conversation;
- permissions are checked server-side;
- private Tutor Memory is isolated by tutor ID and visible to the user;
- autonomous writes are bounded, inspectable, editable, and removable;
- deleting/resetting a tutor cannot delete Learner Memory, Notebook, Knowledge,
  Quiz, or Space data;
- background session behavior remains at least as reliable as current Chat;
- all automated tests pass and desktop restart behavior is manually verified.

## 8. Historical First Execution Slice

The initial implementation began with Phase 0 and Phase 1 only, delivering the
identity loop before Tutor Memory. This slice is now complete:

1. create `TutorStore` and CRUD routes;
2. seed General Tutor;
3. add optional immutable `tutor_id` to session metadata and APIs;
4. add the Chat empty-state tutor chooser;
5. display and restore tutor identity in conversation UI;
6. prove restart persistence and Temporary Assistant compatibility with tests.

This gives the user-visible product model a solid base before Soul injection,
tool permissions, autonomous memory, and handoff increase the behavioral risk.
