# Product Onboarding Plan

> Status: in progress | Date: 2026-07-17 | Last updated: 2026-07-17 | Scope: first-run onboarding, contextual empty states, one-time hints, and reusable in-app guidance.

## 1. Goal

Help a new user complete the first useful learning task with minimal setup and
without turning the product into a feature tour.

The onboarding experience should connect existing product capabilities:

```text
Model readiness -> Optional Tutor -> First real task
                                  -> Chat / Research / Notebook / Quiz
```

The normal product remains the destination. Onboarding does not create a
separate tutorial workspace or simulated data model.

## 2. Product Decisions

- First-run onboarding has at most three primary steps.
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

### Step 3: First Useful Task

Offer a small set of starter actions:

- explain a concept in Chat;
- research a topic through Research conversation and its explicit workflow;
- create or organize a Notebook note;
- generate a Quiz from a topic or selected material.

The selected action opens the real destination with an editable starting prompt
or action. Completion is recorded when the user enters the product, not after a
ceremonial completion page.

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

- [x] Build the three-step desktop onboarding surface.
- [x] Reuse the existing model configuration and connection-test boundaries.
- [x] Reuse the bounded Tutor chooser and Temporary Assistant behavior.
- [x] Route starter actions into real Chat, Research, Notebook, and Quiz flows.
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
- [x] Verify Chat, Research, Notebook, and Quiz starter actions route into their
  real product surfaces with editable prompts where applicable.
- [ ] Complete keyboard-only, English-copy, and installed-desktop QA. Light and
  dark themes plus the `1100 x 700` minimum desktop viewport have been visually
  verified in the local UI.

## 7. Acceptance Criteria

- A fresh user can configure or reuse a model and reach a useful task in no
  more than three primary steps.
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
