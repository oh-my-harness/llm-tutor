# Persistent Tutor Design

> Status: planned | Date: 2026-07-15 | Product surface: 辅导机器人

## 1. Product Decision

The Tutor surface shall represent persistent tutor entities, not another Chat
mode. A tutor owns a role, goal, capability policy, resource permissions,
conversation collection, and private continuity memory. New conversations
begin by choosing who the user wants to learn with; that tutor may then use
Chat, Research, Quiz, Deep Solve, Notebook, Knowledge, Space, and Memory as
parts of one learning relationship.

The product model is:

```text
Tutor = who accompanies the learner
Capability = what the tutor is doing now
Model = which LLM executes the work
Resources = which user-owned material may be used
```

## 2. Memory Ownership

Persistent tutors do not each copy the complete learner profile. Memory is
split by ownership:

| Memory | Owner | Purpose | Sharing |
| --- | --- | --- | --- |
| L1 evidence | Product workspace | What happened in Chat, Quiz, Notebook, and Knowledge | Shared product evidence |
| Learner Memory | Learner | Profile, scope, preferences, strengths, weaknesses, and recent learning state | Readable by authorized tutors |
| Tutor configuration | User | Tutor identity, role, specialty, goals, defaults, and permissions | Belongs to one tutor |
| Tutor Memory | Tutor relationship | Commitments, open loops, lesson plans, reflections, and next actions | Private to one tutor by default |

Learner Memory answers “what is known about the learner.” Tutor Memory answers
“what this tutor promised, where this learning relationship stopped, and what
the tutor should do next.”

Example:

```text
Learner Memory: The learner understands formulas more easily after an example.
Tutor Memory: Begin the next positional-encoding lesson with a two-dimensional example.
```

Tutor Memory must not become a hidden duplicate user profile. New observations
about the learner continue to enter L1 and the normal Learner Memory
consolidation path.

## 3. Tutor Memory

Each tutor may maintain:

```text
tutors/<tutor-id>/memory/
  commitments.md
  open_loops.md
  lesson_plan.md
  reflections.md
  strategy.md
```

Supported entry kinds:

- `commitment`: something the tutor promised to do.
- `open_loop`: a question, exercise, or follow-up that remains unresolved.
- `lesson_plan`: the agreed or inferred next teaching sequence.
- `reflection`: evidence about whether a teaching approach worked.
- `strategy`: concrete behavior the tutor should use in future sessions.

Entries require stable IDs, timestamps, source references, lifecycle state, and
optional expiry. Completed commitments and resolved open loops are closed
rather than kept forever as active context.

Tutors may autonomously write low-risk operational memory when it directly
improves continuity. They may not store credentials, sensitive personal data,
external factual claims, or unsupported personality judgments. Tutor Memory is
visible, editable, removable, and resettable by the user.

## 4. Tutor Entity

A tutor profile contains at least:

```json
{
  "id": "transformer-tutor",
  "name": "Transformer 导师",
  "role": "帮助用户系统掌握 Transformer 架构",
  "goal": "从注意力机制推进到可独立阅读论文",
  "default_model_config_id": "...",
  "default_capability": "chat",
  "allowed_capabilities": ["chat", "research", "quiz", "deep_solve"],
  "learner_memory_access": true,
  "resource_permissions": {
    "knowledge_base_ids": [],
    "notebook": true,
    "space": true
  },
  "autonomous_memory": true
}
```

The role and permissions are explicit product configuration. They are not
silently rewritten by the model.

## 5. New Conversation Entry

The empty Chat state becomes a tutor chooser centered on the question:

> 这次想和哪位导师交流？

It shows:

- recently used tutors;
- all configured tutors;
- each tutor's role, current goal, last progress, and open-loop count;
- create-tutor action;
- a Temporary Assistant entry for one-off work.

Selecting a tutor immediately creates or opens a conversation. A configured
default or last-used tutor may support a one-click quick start so the chooser
does not become repeated friction.

Temporary Assistant preserves today's lightweight Chat behavior. It may read
authorized Learner Memory but has no persistent role, private Tutor Memory, or
long-term tutor plan.

## 6. Session Binding and Handoff

Every persistent conversation stores a stable `tutor_id` beside its runtime
session mapping. The binding does not change in place because changing tutor
identity would replace role instructions and private memory inside an existing
runtime history.

To change tutors, the product creates a new conversation through a handoff:

1. choose the destination tutor;
2. prepare a bounded conversation summary;
3. let the user choose which artifacts and open loops to share;
4. create a new runtime session owned by the destination tutor.

Deleting or resetting a tutor must not delete global Learner Memory, Notebook,
Knowledge, Quiz, or Space assets. Session retention is an explicit user choice.

## 7. Context Assembly

For a tutor-bound turn, product code supplies thin, explicit context mappings
to the runtime:

```text
tutor role and permissions
  + relevant Learner Memory
  + relevant private Tutor Memory
  + runtime session history
  + current capability and selected resources
  -> AgentHarness / WorkflowEngine
```

The runtime continues to own sessions, context construction, tools, traces,
compaction, and workflow execution. `llm-tutor` owns tutor records, permission
mappings, memory files, UI, and runtime-session IDs.

Memory should be read on demand. Only small high-priority commitments and open
loops may be included at turn start; complete tutor files must not be injected
into every prompt.

## 8. Product Integration

- Chat is the tutor's primary interaction surface.
- Research is a detailed workflow the tutor can start after confirming scope.
- Quiz checks learning progress and feeds shared Learner Memory evidence.
- Deep Solve supports difficult explanations and derivations.
- Notebook stores durable reports and learning material.
- Knowledge supplies grounded source documents.
- Space exposes quizzes, notes, profile, and other learning assets.
- Learner Memory is shared user context.
- Tutor Memory preserves relationship-specific plans and commitments.

The user chooses a tutor before choosing tools. The tutor chooses or proposes
the appropriate capability as the task develops.

## 9. UI Shape

The Tutor page is an operational workspace:

- left rail: tutor list, selection, and run state;
- main area: the selected tutor's conversations;
- compact side area: current goal, next plan, commitments, and open loops;
- settings: role, model defaults, capabilities, resource permissions, and
  autonomous-memory policy;
- memory management: inspect, edit, close, delete, or reset private entries.

Conversation rows display their tutor identity. Tutor selection is also the
first step of the new-conversation empty state.

## 10. MVP Scope

### Phase 1: Persistent Identity

- Add tutor store and CRUD API.
- Add a built-in General Tutor.
- Add tutor selection to the new-conversation screen.
- Persist immutable `tutor_id` on sessions.
- Apply tutor role, default model, and capability permissions.

### Phase 2: Private Continuity Memory

- Add tutor memory files and read/write tools.
- Support commitments, open loops, plans, reflections, and strategy.
- Show tutor memory in the Tutor workspace.
- Allow reset without changing Learner Memory.

### Phase 3: Resources and Handoff

- Add per-tutor Notebook, Knowledge, Space, and Learner Memory permissions.
- Add bounded handoff into a new tutor-bound session.
- Add recent tutor, open-loop, and background-run indicators.

## 11. Acceptance Criteria

- A user can create or select a tutor before starting a conversation.
- A tutor-bound session restores the same role and private memory after restart.
- Two tutors can share Learner Memory while keeping commitments and plans
  private from one another.
- A tutor can continue an unresolved learning thread across sessions.
- Chat, Research, Quiz, and Deep Solve remain capabilities rather than becoming
  duplicate tutor types.
- Temporary Assistant remains available for one-off conversations.
- Resetting a tutor does not destroy global learning assets or Learner Memory.

