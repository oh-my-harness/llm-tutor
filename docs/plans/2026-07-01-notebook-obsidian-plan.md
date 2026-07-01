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

Notebook should not automatically become a knowledge base in the first slice.

Recommended order:

1. Notebook links, backlinks, tags, import/export.
2. Search and `@` references over Notebook content.
3. Optional Notebook-to-RAG indexing with explicit user action.

This keeps personal notes portable and reviewable before turning them into a
retrieval corpus.

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

- [ ] Add Markdown file import API.
- [ ] Add zip import API.
- [ ] Parse YAML frontmatter.
- [ ] Add import preview with duplicate/conflict detection.
- [ ] Add Notebook UI import action.
- [ ] Add tests for Markdown/frontmatter/link preservation.

### Phase 3: Export

- [ ] Add single-note Markdown export.
- [ ] Add selected-notes export.
- [ ] Add whole-Notebook zip export.
- [ ] Include frontmatter in exported notes.
- [ ] Add Notebook UI export action.
- [ ] Add tests for stable exported filenames and metadata.

### Phase 4: Agent-Assisted Organization

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
