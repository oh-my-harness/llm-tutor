# Notebook Obsidian-Like Plan

> Status: proposed | Date: 2026-07-01 | Scope: evolve Space / Notebook from a saved-record list into a Markdown-first connected knowledge workspace.

## 1. Product Positioning

Notebook should become the user's durable knowledge workspace inside Space.

It should borrow the useful parts of Obsidian:

- Markdown-first notes,
- wiki-style links,
- backlinks,
- tags,
- local import/export,
- graph-style navigation later.

It should not become a full Obsidian clone in the first slice. The goal is to
make learning artifacts connected, portable, and agent-readable while keeping
Chat as the agent interaction surface.

The product split remains:

```text
Chat
  Generate, explain, research, quiz, and propose edits.

Notebook
  Store and connect durable Markdown notes.

Books
  Publish polished outputs from selected notes.

Memory
  Summarize learner state and behavior.
```

Notebook is user-authored knowledge. Memory is agent-maintained learner state.
They can reference each other, but they should not collapse into one store.

## 2. Required Capabilities

### Markdown Notes

- Each Notebook entry is a Markdown note.
- Notes keep title, body, type, tags, source metadata, created time, and updated time.
- Existing `NotebookEntry` storage remains the first implementation base.
- Research reports remain `NotebookEntry(type = research_report)`.

### Wiki Links

Notebook Markdown should support wiki-style links:

```md
[[Photolithography]]
[[note-id|display title]]
[[OPC]]
```

Initial behavior:

- Parse links from Markdown.
- Resolve by stable note id first when available.
- Resolve by normalized title when id is not present.
- Show unresolved links as creatable notes.
- Keep normal Markdown links unchanged.

### Backlinks

Each note detail view should show notes that link to the current note.

Backlinks should include:

- source note title,
- short snippet around the link,
- link target,
- updated time.

### Tags

Notebook Markdown and metadata should support tags:

```md
#lithography
#weak-point
#research
```

Tags should support:

- parsing from note body,
- manual metadata tags,
- filtering note list,
- future use by Quiz and Student Profile.

### Import

Import is required, not optional.

Supported first formats:

- one or more `.md` files,
- a folder of Markdown files,
- a `.zip` containing Markdown files and assets.

Import behavior:

- Preserve filenames as initial titles when frontmatter is absent.
- Parse YAML frontmatter if present.
- Preserve Markdown content without destructive cleanup.
- Convert Obsidian-style `[[links]]` into internal link index entries without rewriting content by default.
- Store unknown frontmatter fields in metadata.
- Detect duplicate titles and either keep both with suffixes or ask the user before merge.
- Record import source metadata.

Later import formats:

- plain text files,
- exported chat Markdown,
- exported Book chapters,
- selected Knowledge Base snippets.

### Export

Export is required, not optional.

Supported first formats:

- export one note as Markdown,
- export selected notes as Markdown files,
- export the whole Notebook as a folder or zip.

Export behavior:

- Preserve Markdown body.
- Preserve YAML frontmatter for title, id, type, tags, source metadata, created time, and updated time.
- Preserve `[[links]]` where possible.
- Include assets once asset support exists.
- Provide a predictable file naming strategy.

Later export formats:

- PDF,
- HTML,
- Book-ready Markdown bundle,
- Obsidian-compatible vault export.

## 3. Data Model Direction

Keep the existing `NotebookEntry` as the durable record for the next slice:

```ts
NotebookEntry {
  id: string
  spaceId?: string
  type: 'note' | 'research_report' | 'chat_excerpt' | 'quiz_summary' | 'source_snippet' | 'deep_solve_result'
  title: string
  markdown: string
  tags: string[]
  metadata: Record<string, unknown>
  sourceSessionId?: string
  sourceMessageId?: string
  createdAt: string
  updatedAt: string
}
```

Add a derived index:

```ts
NotebookLinkIndex {
  sourceEntryId: string
  targetEntryId?: string
  targetTitle?: string
  raw: string
  alias?: string
  resolved: boolean
}
```

The link index can be rebuilt from note Markdown. It should not become the
primary source of truth.

For v0.1, keep JSON storage. When notebook relationships, assets, and import
history grow, consider SQLite or a file-backed vault mode.

## 4. UI Direction

Notebook should feel like a compact knowledge workspace:

- left pane: notes, filters, tags, import/export actions,
- center pane: Markdown editor/preview,
- right pane: backlinks, outgoing links, source metadata, agent edit proposal.

Near-term views:

- list,
- detail,
- edit,
- backlinks,
- tags,
- import/export dialogs.

Later views:

- local graph for current note,
- full graph for Space,
- unresolved links list,
- duplicate notes review.

## 5. Agent Behavior

Agents should interact with Notebook through explicit product tools and user
confirmation.

Allowed agent behaviors:

- read a mentioned note,
- propose edits to a note,
- propose new links,
- propose tags,
- propose merging duplicate notes,
- summarize a group of notes,
- generate a quiz from explicitly referenced notes.

Write behavior:

- Agent writes are proposals first.
- User applies or rejects proposals.
- Applied writes create Notebook memory events.
- Direct silent writes are out of scope.

Potential future tools:

```text
read_notebook_note
search_notebook
propose_notebook_edit
propose_notebook_links
propose_notebook_tags
propose_notebook_merge
```

`read_space_item` can remain the initial boundary. Add more specific Notebook
tools only when the generic Space tool becomes awkward.

### Chat-Triggered Organization

Notebook organization should primarily be triggered from Chat, not from a
separate "AI maintenance" surface inside Notebook.

Supported user intents:

- "帮 @OPC 这篇笔记补一些 wiki links。"
- "给 @光刻模型 这篇笔记整理标签。"
- "看看 @OPC 和 @光学邻近校正 是不是重复。"
- "把这篇笔记改得更适合后续出题。"

The product flow is:

```text
User asks in Chat
  -> Agent reads mentioned Notebook entries through read_space_item
  -> Agent produces a proposal tool result
  -> Chat renders a review card
  -> User applies or rejects
  -> Product updates Notebook through normal Notebook APIs
  -> Product records a Notebook memory event
```

The agent must not directly write Notebook content. Even if the user asks the
agent to "修改这篇笔记", the agent should produce a proposal first. The product
UI owns the final write after explicit user confirmation.

Proposal types:

- `notebook_edit`: complete Markdown replacement or diff for one entry.
- `notebook_tags`: suggested tags to add/remove.
- `notebook_links`: suggested `[[wiki links]]` to add or normalize.
- `notebook_merge`: suggested canonical note, merged Markdown, and affected
  source entries.

For the first implementation, all organization proposal types can be serialized
as a structured proposal plus a complete Markdown replacement. More specialized
apply APIs can come later if complete Markdown replacement becomes too blunt.

### Notebook Lookup Without Explicit `@`

Users should not always have to `@` a note. If the user asks a question that
appears to involve saved notes, the agent may search Notebook before answering.

Examples:

- "我之前关于 OPC 记了什么？"
- "根据我的笔记解释一下光刻胶模型。"
- "帮我从已有笔记里找一下和 EUV 有关的内容。"
- "这和我之前研究的张仕林有什么关系？"

This requires a separate Notebook lookup capability from `read_space_item`:

```text
search_notebook(query, limit, filters?)
  -> returns candidate entries with id, title, type, tags, snippet, score

read_notebook_note(entry_id)
  -> returns exact Markdown for a selected candidate
```

Behavior rules:

- If the user explicitly references an artifact with `@`, use `read_space_item`
  for that exact artifact.
- If the user asks about "my notes", "Notebook", "previously saved", or a topic
  likely stored in Notebook, call `search_notebook` before answering.
- If search returns one or a few high-confidence candidates, read the relevant
  note(s) and answer with Notebook citations.
- If search returns ambiguous candidates, ask the user to choose or present the
  candidate list before making strong claims.
- If no candidate is found, say that no relevant Notebook entry was found, then
  optionally answer from general knowledge only if the user asked for that.
- For edit/merge requests without `@`, search first, then ask the user to
  confirm the target notes before creating a proposal.

This keeps Chat natural while avoiding two bad extremes: forcing users to always
pick notes manually, or letting the agent silently invent Notebook context.

### Chat Source Association

Notebook association in the chat composer should share the existing knowledge
source button with Knowledge Base selection. Do not add a second top-level
Notebook toggle in the composer.

The shared selector should represent "source association" rather than only
"knowledge base association":

```text
No source
Notebook
Knowledge Base: <kb name>
```

Product semantics:

- `No source`: the agent uses conversation context, attachments, explicit `@`
  mentions, memory, and mode-specific tools only.
- `Notebook`: the agent may use Notebook as a local plain-text knowledge source.
- `Knowledge Base`: the agent may use the selected LanceDB-backed RAG knowledge
  base.

Notebook association is intentionally plain-text only. It must not use
embeddings, LanceDB, or any future vector index. Notebook remains a Markdown
workspace, not a RAG corpus.

When Notebook is associated:

- Chat mode may search Notebook when answering questions that could benefit from
  saved notes.
- Organize mode should actively use Notebook search and read tools.
- Quiz mode may use Notebook search as source material when no more specific
  `@` item or attachment is provided.
- Research mode may use Notebook as private prior context, but external factual
  claims still require web search/fetch when appropriate.

The backend should model this separately from `kb_id`:

```ts
SessionSourceAssociation =
  | { type: 'none' }
  | { type: 'notebook'; scope: 'default_space' }
  | { type: 'knowledge_base'; kbId: string }
```

For compatibility with the current session model, the first implementation may
store this as:

```ts
kb?: string
notebook_enabled: boolean
```

Rules:

- `kb` must only mean a real Knowledge Base id.
- `notebook_enabled` must only mean plain-text Notebook search is available.
- The UI should prevent selecting both Notebook and Knowledge Base from the same
  shared source button until a future multi-source design is introduced.
- Notebook search results should cite navigable Notebook sources such as
  `notebook:<entry_id>`.

### Chat Modes And Tools

Chat modes should represent task intent and default workflow, not a hard-coded
tool set.

Recommended composer modes:

- `chat`: ordinary tutoring, explanation, and lightweight Q&A.
- `deep_solve`: multi-step problem solving.
- `quiz`: quiz generation from conversation, attachments, associated source, or
  explicit `@` material.
- `research`: web/source exploration and report generation.
- `organize`: Notebook/Space organization, including search, tags, links,
  deduplication, and edit proposals.

`code_exec` should be demoted from a user-facing mode to a tool available to
other modes. The model should use it when computation, verification,
simulation, parsing, or code execution is needed. This prevents a confusing
"code mode" that is really just one tool, while keeping code execution useful in
Chat, Deep Solve, Research, Quiz, and Organize workflows.

Organize mode defaults:

- Notebook association should be enabled by default when entering Organize mode.
- The agent should search Notebook before making claims about saved notes.
- The agent may propose edits, tags, links, or merges.
- All writes must remain proposal-first and user-confirmed.

## 6. Import / Export Architecture

Backend responsibilities:

- parse uploaded Markdown files or zip bundles,
- extract frontmatter,
- create or update Notebook entries,
- rebuild link/tag index,
- generate Markdown or zip exports.

Frontend responsibilities:

- file/folder/zip picker,
- import preview,
- conflict resolution UI,
- export scope selection,
- download generated archive.

Safety rules:

- Import preview before destructive merge.
- Export should never require network access.
- Export should be readable outside this app.
- Import should preserve original Markdown as much as possible.

## 7. Relationship With RAG

Notebook is not a Knowledge Base and should not become a vectorized RAG corpus.
It remains a Markdown/plain-text workspace.

Recommended order:

1. Notebook links, backlinks, tags, import/export.
2. Search and `@` references over Notebook content.
3. Shared chat source association that can select either Notebook plain-text
   lookup or one LanceDB-backed Knowledge Base.

This keeps personal notes portable, inspectable, and editable while Knowledge
Base remains the place for embeddings, chunking, LanceDB storage, and RAG
retrieval.

## 8. Implementation Phases

### Phase 1: Link and Tag Index

- [x] Parse `[[wiki links]]` from Notebook Markdown.
- [x] Parse tags from Markdown.
- [x] Add derived link/tag/backlink view helpers without changing durable Notebook storage.
- [x] Add backend tests for link parsing, tag parsing, metadata tag merge, link resolution, and backlinks.
- [x] Show outgoing links and backlinks in note detail.
- [x] Add unresolved-link click-to-create flow.
- [x] Parse tags from note metadata and merge them with Markdown tags.

### Phase 2: Import

- [x] Add Markdown file import API.
- [x] Add zip import API.
- [x] Parse YAML frontmatter.
- [x] Add import preview with duplicate/conflict detection.
- [x] Add Notebook UI import action.
- [x] Add tests for Markdown/frontmatter/link preservation.

### Phase 3: Export

- [x] Add single-note Markdown export.
- [x] Add selected-notes export.
- [x] Add whole-Notebook zip export.
- [x] Include frontmatter in exported notes.
- [x] Add Notebook UI export action.
- [x] Add tests for stable exported filenames and metadata.

### Phase 4: Agent-Assisted Organization

- [x] Add Chat composer source association for No source / Notebook / Knowledge Base.
- [x] Keep Notebook association plain-text only and separate from Knowledge Base ids.
- [x] Add Organize mode as a Chat workflow.
- [x] Add `search_notebook` product tool for plain-text Notebook lookup.
- [x] Demote code execution from visible Chat mode to a tool available inside modes.
- [ ] Add "suggest links" proposal workflow.
- [ ] Add "suggest tags" proposal workflow.
- [ ] Add "merge duplicate notes" proposal workflow.
- [ ] Keep apply/reject explicit.
- [ ] Record applied organization changes as Notebook memory events.

### Phase 5: Graph and Advanced Portability

- [ ] Add local graph for current note.
- [ ] Add unresolved link review.
- [ ] Add Obsidian-compatible vault export.
- [ ] Evaluate file-backed vault mode.

## 9. Open Questions

- Should note titles be globally unique inside one Space?
- Should unresolved `[[links]]` create placeholder notes automatically or only on click?
- Should imported folders become tag prefixes, source metadata, or both?
- Should attachments/assets live inside Notebook storage or a shared Space asset store?
- Should Notebook export include Memory references, or only note content?
