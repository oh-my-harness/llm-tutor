# Memory Consolidation Design

> Status: draft | Date: 2026-06-26 | Scope: summarize the DeepTutor-inspired
> memory consolidation workflow and define the target prompt/input/output
> contract for `llm-tutor`.

## 1. Goal

The memory system should turn product activity into durable, inspectable learner
memory without letting the model freely rewrite hidden profile state.

Target flow:

```text
Product event / workspace record
  -> normalized consolidation input
  -> LLM returns structured JSON operations
  -> product code validates refs and applies operations
  -> Markdown memory documents are serialized deterministically
  -> agents read L3 memory through read_memory when useful
```

The key lesson from DeepTutor is that consolidation prompts should not ask the
model to write final Markdown. They should ask the model to extract or edit
small facts against a normalized input contract. The application owns Markdown
formatting, entry ids, reference validation, deduplication, and file writes.

## 2. Design Principles

- Keep raw product data and final memory documents separate.
- Give the LLM a uniform, compact, evidence-rich input shape.
- Require JSON-only output from the LLM.
- Validate every source reference before writing memory.
- Use Markdown as the durable user-editable memory surface.
- Treat memory as personalization evidence, not factual source evidence.
- Allow agents to write only explicit user preferences by default.
- Route profile, scope, recent, and teaching-strategy updates through a
  user-visible memory workbench.

## 3. Layers

### L1: Product Event Ledger

L1 is not the primary UI. It is the raw, append-only or derived event layer used
as consolidation evidence.

Recommended sources:

- chat messages and assistant answers,
- quiz generation events,
- quiz answers and scores,
- notebook entries,
- knowledge-base interactions,
- research reports,
- future task/workflow outputs.

Recommended event shape:

```ts
MemoryEvent {
  id: string
  ts: string
  surface: 'chat' | 'quiz' | 'notebook' | 'knowledge' | 'research'
  kind: string
  title?: string
  content: string
  metadata?: Record<string, unknown>
  sessionId?: string
  turnId?: string
}
```

The important part is that each event has a stable `surface:id` reference that
can be cited from L2 memory.

### L2: Surface Memory

L2 summarizes one product surface at a time. These files describe durable facts
from a specific activity stream.

Recommended files:

```text
memory/L2/chat.md
memory/L2/quiz.md
memory/L2/notebook.md
memory/L2/knowledge.md
memory/L2/research.md
```

### L3: Cross-Surface Learner Memory

L3 synthesizes across L2 files. It is the primary agent-readable memory layer.

Recommended files:

```text
memory/L3/recent.md
memory/L3/profile.md
memory/L3/scope.md
memory/L3/preferences.md
memory/L3/teaching_strategy.md
```

Only `preferences.md` should be directly writable from chat, and only when the
user explicitly states a long-term preference or approves a fact. Other L3 files
should be updated through the memory workbench.

## 4. Normalized Consolidation Input

DeepTutor's strongest idea is that all consolidation jobs are fed a normalized
view instead of arbitrary app-specific payloads.

The input should contain:

- job metadata,
- existing target memory,
- allowed sections,
- focus instructions,
- a chunk-local reference pool,
- source chunks in a stable format.

Target input envelope:

```ts
ConsolidationInput {
  job: {
    mode: 'update' | 'audit' | 'dedup'
    layer: 'L2' | 'L3'
    key: string
    language: 'zh' | 'en'
    today: string
  }
  target: {
    title: string
    existingMarkdown: string
    allowedSections: string[]
    focus: string
  }
  chunk: {
    index: number
    total: number
    start?: number
    end?: number
    citeableRefs: string[]
    text: string
  }
}
```

The LLM-facing source chunk should be rendered like this:

```md
# Chunk-local citeable refs
- chat:session_123
- chat:session_456

@entity chat:session_123
title: 2 的 pi 次方计算方法
ts: 2026-06-26T10:30:00Z
content:
User asked how to calculate 2^pi. Assistant answered with log/exponential
calculation and numeric result.

@entity chat:session_456
title: 光刻模型与光刻胶模型
ts: 2026-06-26T11:12:00Z
content:
User confused lithography model with photoresist model during a Q&A turn.
```

This format gives the model enough information while making reference checking
straightforward.

## 5. Section Catalog

Each surface and slot should have a fixed focus and section list. The model may
only emit sections from the list.

Recommended L2 catalog:

```yaml
surfaces:
  chat:
    focus: Stable misconceptions, demonstrated mastery, and recurring topics.
    sections: [Misconceptions, Mastery, Topics]
  quiz:
    focus: Error patterns, strong topics, weak topics, and question types.
    sections: [Error patterns, Strong topics, Weak topics]
  notebook:
    focus: Recurring note themes, preferred formats, and open questions.
    sections: [Themes, Formats, Open questions]
  knowledge:
    focus: Document interests, frequent queries, and knowledge gaps.
    sections: [Interests, Frequent queries, Gaps]
  research:
    focus: Research topics, preferred report shape, and unresolved questions.
    sections: [Topics, Report preferences, Open questions]
```

Recommended L3 catalog:

```yaml
slots:
  recent:
    focus: Rolling timeline of recent learning activity.
    sections: [This week, Earlier]
  profile:
    focus: Durable learner identity, learning style, strengths, and weaknesses.
    sections: [Identity, Learning style, Strengths, Weaknesses]
  scope:
    focus: Concepts the learner has engaged with and confidence labels.
    sections: [Familiar, Practicing, Unsure]
  teaching_strategy:
    focus: How the tutor should adapt examples, difficulty, hints, and reviews.
    sections: [Explanation style, Practice strategy, Review strategy]
  preferences:
    focus: Explicit user-stated long-term preferences.
    sections: [Preferences]
```

Chinese UI can localize section labels, but internal tests are easier if the
stored section keys are stable. A display-name map can translate them.

## 6. Update Prompt Contract

Update mode extracts new memory facts from source chunks.

### L2 Update Prompt

System prompt template:

```text
You are the memory curator for llm-tutor user {user_label}.

ROLE:
Read a chunk of the user's recent {surface} activity. Extract durable facts
about the learner. Prefer learning-relevant facts: misconceptions, strengths,
recurring topics, preferences, and review needs. Drop one-off chatter.

OUTPUT:
Return exactly one JSON object. No prose. No Markdown fences.

{
  "facts": [
    {
      "text": "<one concise fact, <= 240 chars>",
      "section": "<one of: {sections}>",
      "refs": ["<surface>:<entity_id>", "..."]
    }
  ]
}

HARD RULES:
- Every fact must cite at least one ref.
- Refs must come from the chunk-local citeable refs list or @entity markers.
- Do not invent ids.
- Do not cite refs outside this chunk.
- Do not duplicate facts already captured in existing memory.
- Use cautious language. Avoid absolute claims unless quoting the user.
- If nothing durable appears, return {"facts": []}.
- Today is {today}.

Surface focus:
{focus}
```

User prompt template:

```text
# Existing {surface} memory
{existing_memory}

# Source chunk {chunk_index}/{chunk_total}
{chunk}

Return JSON. Cite only refs visible in the source chunk.
```

### L3 Update Prompt

L3 update differs from L2 update in one important way: it synthesizes across L2
surface memories and should hedge every learner-level claim.

System prompt template:

```text
You are the cross-surface memory curator for llm-tutor user {user_label}.

ROLE:
Read a chunk of L2 memory from one or more surfaces. Synthesize durable, hedged
claims about the learner.

OUTPUT:
Return exactly one JSON object.

{
  "facts": [
    {
      "text": "<one hedged learner claim, <= 240 chars>",
      "section": "<one of: {sections}>",
      "refs": ["<surface>", "..."]
    }
  ]
}

HARD RULES:
- Refs are bare surface names from the chunk-local citeable refs list.
- Do not emit entry ids in L3 update.
- Prefer claims like "Across quiz and chat entries..." or
  "Quiz entries show...".
- Do not overgeneralize from a single weak signal.
- Do not duplicate existing memory.
- If nothing durable appears, return {"facts": []}.
- Today is {today}.

Slot focus:
{focus}
```

## 7. Audit Prompt Contract

Audit mode checks existing memory against source evidence. It should produce
line-level edit operations, not rewritten documents.

Input for audit should render the target memory as a numbered view and annotate
each bullet with its evidence.

Example L2 audit input:

```md
# Line-numbered view
 1: # chat memory
 2:
 3: ## Misconceptions
 4: - User confuses lithography model and photoresist model [^1] <!--m_abc-->

# Evidence for line 4
refs:
- chat:session_123
source:
User asked "what is lithography model, what is photoresist model" after an
assistant answer that mixed the terms.
```

System prompt template:

```text
You are the memory auditor for llm-tutor user {user_label}.

ROLE:
Read a chunk of {key} memory with each entry annotated by original evidence.
Find factual errors, unsupported claims, stale claims, and overgeneralizations.

OUTPUT:
Return exactly one JSON object.

{
  "edits": [
    {
      "op": "replace",
      "line": 4,
      "new_text": "<corrected fact, <= 240 chars>",
      "refs": ["<allowed ref>", "..."],
      "reason": "<short reason>"
    },
    {
      "op": "delete",
      "line_start": 4,
      "line_end": 4,
      "reason": "<short reason>"
    },
    {
      "op": "insert",
      "after_line": 4,
      "text": "<new fact, <= 240 chars>",
      "section": "<optional allowed section>",
      "refs": ["<allowed ref>", "..."],
      "reason": "<short reason>"
    }
  ]
}

HARD RULES:
- Edit only visible lines.
- Do not edit titles, blanks, or section headers.
- Replace/insert must cite visible evidence.
- Keep wording cautious and evidence-bound.
- If nothing needs fixing, return {"edits": []}.
- Today is {today}.
```

For L3 audit, evidence should be the L2 entries that support the L3 claim. L3
audit may cite L2 entry ids because it is checking an existing L3 claim against
specific L2 evidence.

## 8. Dedup Prompt Contract

Dedup mode should not add facts. It only merges, rewrites, or deletes existing
memory lines.

System prompt template:

```text
You are the memory dedup pass for llm-tutor user {user_label}.

ROLE:
Read the full memory document as a line-numbered view. Merge duplicates,
collapse near-duplicates, and delete low-signal repeated entries.

OUTPUT:
Return exactly one JSON object.

{
  "edits": [
    {
      "op": "replace",
      "line": 4,
      "new_text": "<merged fact, <= 240 chars>",
      "refs": ["<existing-or-unioned-ref>", "..."],
      "reason": "<short reason>"
    },
    {
      "op": "delete",
      "line_start": 5,
      "line_end": 5,
      "reason": "duplicate of line 4"
    }
  ]
}

HARD RULES:
- Use only replace and delete.
- Do not insert new facts.
- Preserve or union refs when merging.
- Delete the lower-quality duplicate.
- If nothing needs deduping, return {"edits": []}.
- Today is {today}.
```

## 9. Markdown Output Contract

The LLM never writes this directly. Product code serializes accepted facts and
edits into Markdown.

Target format:

```md
# chat memory

## Misconceptions

- User often confuses lithography model and photoresist model. [^1] <!--m_01ABC-->

## Mastery

- User correctly explains OPC and SMO as different computational lithography techniques. [^2] <!--m_01DEF-->

---

[^1]: chat:session_123
[^2]: quiz:session_456:q_2
```

Required invariants:

- Every bullet has a stable entry id marker: `<!--m_xxx-->`.
- Every cited source appears in the footnote block.
- Footnote labels are generated by the serializer, not the LLM.
- Repeated refs should share one footnote label.
- Parser and serializer should round-trip idempotently.
- Deleting one entry should remove unused footnotes on the next serialize.

## 10. Validation Rules

Before writing memory:

- Parse LLM output as JSON.
- Reject non-JSON prose.
- Reject facts with empty `text`.
- Reject sections not in the allowed section list.
- Reject refs outside the current ref pool.
- Reject L2 facts with no refs.
- Reject L3 update facts whose refs are not bare allowed surface names.
- Truncate or reject facts above the max length.
- Apply operations through a single parser/serializer path.

Audit/dedup operations should also validate line numbers and operation type.
Apply edits in reverse line order so line numbers remain stable.

## 11. Workbench Behavior

The Memory workbench should expose three actions for L2 and L3:

- Update memory: read new inputs and append validated facts.
- Check memory: audit current facts against their sources.
- Remove duplicates: merge duplicate or near-duplicate entries.

Recommended UX:

- Show model progress by chunk.
- Show LLM input/output in an expandable trace.
- Show proposed facts or edits before applying when practical.
- Allow undo of the latest run when a run writes files.
- Keep L1 hidden from the primary memory overview, but allow source refs to
  resolve back to source records.

## 12. Agent Tool Boundaries

`read_memory`:

- Available to Chat, Research, Deep Solve, and Quiz planning.
- Should be called when personalized teaching, review, quiz targeting, or
  long-running context matters.
- Should not be called for pure factual questions that do not need learner
  personalization.

`write_memory`:

- Should only write explicit preferences or user-approved facts.
- Should default to `L3/preferences.md`.
- Should not update `profile.md`, `scope.md`, `recent.md`, or
  `teaching_strategy.md` during ordinary chat.

Memory content should guide explanation style and planning. It should not be
presented as factual proof about external domains.

## 13. Implementation Notes for `llm-tutor`

Current `llm-tutor` already has Markdown memory files, L1 event recording, a
Memory UI, and `read_memory`. The next quality improvements should be:

- Replace ad hoc memory assist prompts with the unified input envelope above.
- Move toward JSON-only update/audit/dedup operations.
- Add strict ref-pool validation before writes.
- Add line-numbered audit/dedup views.
- Add serializer-level footnote normalization.
- Keep L3 updates hedged and source-attributed.
- Add tests for malformed JSON, invalid refs, duplicate refs, unknown sections,
  and idempotent parse/serialize.

## 14. Why This Shape Is Better

This design gives us:

- inspectable learner memory,
- controllable model output,
- fewer hallucinated profile claims,
- source-linked memory entries,
- user-editable Markdown,
- room to add automatic consolidation later,
- a clean boundary between product data and runtime agent behavior.

The core rule is simple: the LLM proposes small evidence-bound operations; the
application owns the memory document.
