# Product Onboarding Plan

> Status: in progress | Date: 2026-07-17 | Last updated: 2026-07-17 | Scope: first-run onboarding, contextual empty states, one-time hints, and reusable in-app guidance.

## 1. Goal

Help a new user complete the first useful learning task with minimal setup and
without turning the product into a feature tour.

The onboarding experience should connect existing product capabilities:

```text
Model readiness -> Optional Tutor -> Knowledge Base -> Notebook -> Memory
                                                        -> Conversation modes
                                                        -> Chat / Research / Quiz / Organize
```

The normal product remains the destination. Onboarding does not create a
separate tutorial workspace or simulated data model.

## 2. Product Decisions

- First-run onboarding has six concise steps: model readiness, optional Tutor,
  Knowledge Base, Notebook, Memory, and conversation-mode selection.
- The flow is nonblocking except when an LLM action genuinely requires a model.
- An existing valid model configuration is detected and reused.
- Tutor selection is optional; skipping it uses Temporary Assistant.
- Starter actions launch real product workflows with editable prompts.
- Feature pages use compact contextual empty states instead of repeated modal
  walkthroughs.
- Complex controls may show one concise, dismissible hint on first use.
- Settings or Help can reopen onboarding without changing existing data.
- Closing an incomplete flow pauses it instead of marking it complete. A compact
  floating action resumes the current step without taking workspace layout.
- Completion requires either launching a real starter task or explicitly
  choosing `Complete` on the final step.
- State is local, versioned, and independent from runtime conversation history.
- MVP does not add hosted onboarding analytics.

## 3. First-Run Flow

### Step 1: Model Readiness

The app checks existing LLM settings before showing configuration UI.

- When a usable model exists, show it as ready and allow immediate continuation.
- When none exists, explain that an LLM is required for agent actions and link
  directly to the relevant provider configuration.
- Reuse the existing connection test and model capability information.
- Preserve credentials and provider settings if the user moves backward or
  closes onboarding.

### Step 2: Tutor Choice

- Show the existing Tutor chooser in a bounded, compact form.
- Allow creating or selecting a Tutor.
- Allow continuing without a Tutor; this means Temporary Assistant and does
  not add a duplicate Tutor item.
- Keep Tutor identity separate from Chat, Research, and Quiz capability modes.

### Step 3: Knowledge Base

- Explain that RAG requires a configured embedding model, while users who do
  not need RAG may skip this step.
- Show whether an embedding configuration and one or more Knowledge Bases
  already exist.
- Link directly to Embedding settings and the real Knowledge Base workspace.
- Explain the complete usage loop: configure embedding, create a Knowledge
  Base, add documents, then select that Knowledge Base from the Chat source
  selector so the Agent can retrieve and cite it.

### Step 4: Notebook

- Explain that the built-in local Notebook works without external setup.
- Explain that desktop users may bind an existing Markdown Vault from Notebook
  settings, and that generated content can then use the native save dialog.
- Show whether the current Notebook uses the app-local directory or an external
  Vault, and link to both Notebook settings and the real Notebook workspace.

### Step 5: Memory

- Explain L1 workspace evidence, L2 per-module summaries, and L3 cross-module
  learner context without presenting Memory as an external fact store.
- Explain that users open an L2 or L3 document, choose update, check, or
  deduplicate, select a model, run the workflow, review the proposed diff, and
  apply accepted changes.
- Link directly to the real Memory workspace; this step does not create or
  modify memory by itself.

### Step 6: Conversation Modes

The final step uses a dedicated child surface inside the onboarding dialog. It
does not add four more top-level steps or a separate application route.

- A stable horizontal mode control switches between Chat, Research, Quiz, and
  Organize without changing the dialog size.
- Each mode explains its suitable scenarios, runtime behavior, usable material,
  expected output, and editable starter prompt.
- Chat is ordinary streaming conversation with optional tool use and no forced
  workflow.
- Research remains ordinary conversation while scope is clarified, then starts
  the explicit detailed-research workflow and produces a cited report.
- Quiz confirms topic or source material before starting its workflow and
  produces a durable interactive quiz card.
- Organize reads Notebook material and proposes reviewable edits; it requires
  Notebook permission and never applies changes before user review.
- Code Exec remains an internal tool rather than a user-facing mode. Retired
  Deep Solve is not reintroduced.

The selected mode's start action opens a new conversation, selects that real
mode, pre-fills an editable prompt without sending it, and completes onboarding.
A user who does not want to launch a conversation may instead choose the
explicit `Complete` action on this final step.

## 4. Contextual Guidance

Empty states should focus on one next action and disappear when relevant data
exists:

| Surface | Empty-state guidance |
| --- | --- |
| Notebook | Create a note or bind a local Vault folder. |
| Quiz Bank | Generate a quiz from Chat, Notebook, Knowledge, or a topic. |
| Memory | Explain that memory grows from usage and offer the first consolidation run. |
| Tutor | Create a Tutor and define its teaching behavior in Soul Markdown. |
| Research | Ask for a research topic; the agent confirms scope before starting the detailed workflow. |

Guidance must remain compact, use existing product commands, and avoid permanent
instruction panels once the surface contains content.

## 5. State Model

Persist guidance state in the existing local settings boundary, conceptually:

```json
{
  "onboarding_version": 1,
  "onboarding_completed": true,
  "dismissed_context_hints": ["notebook.empty", "tutor.soul.first-edit"]
}
```

Rules:

- Increasing `onboarding_version` may introduce a targeted new-feature step,
  but must not replay unrelated completed steps.
- Reopening onboarding does not clear completion state, credentials, sessions,
  Tutors, Notebook data, or Memory.
- Pausing an incomplete flow preserves its current step for the active app run
  and keeps a compact resume action visible until completion.
- Hint identifiers are stable product keys rather than translated labels.
- Generated content and stored user data are not onboarding state.

## 6. Implementation Phases

### Phase 1: State and Entry Conditions

- [x] Add versioned onboarding state to local settings.
- [x] Detect model readiness from existing provider configuration and health
  checks.
- [x] Decide first-run entry without delaying normal app startup.
- [x] Add Settings or Help action to reopen onboarding.
- [x] Keep a non-layout floating resume action visible while onboarding remains
  incomplete.

### Phase 2: First-Run Experience

- [x] Build the six-step desktop onboarding surface.
- [x] Expand the flow to cover Knowledge Base setup and use, Notebook setup,
  and Memory viewing and maintenance before conversation-mode selection.
- [x] Reuse the existing model configuration and connection-test boundaries.
- [x] Reuse the bounded Tutor chooser and Temporary Assistant behavior.
- [x] Replace the mixed starter-task cards with a dedicated four-mode child
  surface for Chat, Research, Quiz, and Organize.
- [x] Route each mode action into a real new conversation with an editable
  starter prompt and enforce Tutor capability and Notebook permissions.
- [x] Support back, skip, dismiss, keyboard navigation, active language, and
  light/dark themes.
- [x] Distinguish pause from completion and provide an explicit final `Complete`
  action.

### Phase 3: Contextual Empty States

- [x] Audit current Notebook, Quiz Bank, Memory, Tutor, and Research empty
  states.
- [x] Add one primary action to each empty state.
- [x] Hide guidance automatically when relevant content exists.
- [ ] Add stable dismissible hint keys only for interactions that remain hard to
  discover after the empty state is gone.

### Phase 4: Verification

- [x] Test fresh profiles, dismissed flows, and reopened onboarding; a real
  configured desktop profile remains to be included in release QA.
- [ ] Test model connection failure and recovery without losing entered state.
- [ ] Test both Temporary Assistant and Tutor-selected entry paths.
- [x] Verify Chat, Research, Quiz, and Organize introductions, disabled Tutor
  permissions, real mode selection, and editable starter prompts.
- [x] Verify Knowledge Base, Notebook configuration, and Memory guidance render
  at desktop size and route to their intended real settings or workspaces.
- [ ] Complete keyboard-only, English-copy, and installed-desktop QA. Light and
  dark themes plus the `1100 x 700` minimum desktop viewport have been visually
  verified in the local UI.

## 7. Acceptance Criteria

- A fresh user can configure or reuse a model, understand the three persistent
  knowledge surfaces, and reach a useful task through six concise steps.
- A configured returning user is not forced through credential setup.
- Tutor choice remains optional and skipping it has clear Temporary Assistant
  semantics.
- Dismissing onboarding never traps the user outside the normal application.
- Dismissing an incomplete flow does not silently mark it complete; the user can
  resume it from the compact floating action.
- Empty-state guidance disappears when its surface has content.
- Reopening onboarding leaves all existing product data unchanged.
- Future releases can add targeted guidance through the versioned state without
  replaying the complete first-run flow.

## 8. Deferred Work

- Hosted onboarding analytics or funnels.
- Interactive demo data or simulated conversations.
- Long multi-page spotlight tours.
- A separate tutorial mode or tutorial-only Tutor.
- Visual-input onboarding until multimodal product requirements are approved.
