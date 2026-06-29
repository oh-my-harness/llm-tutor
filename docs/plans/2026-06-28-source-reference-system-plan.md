# Source Reference System Plan

> Status: proposed | Date: 2026-06-28 | Scope: unify citations, memory footnotes, RAG sources, quiz references, and research sources into one product-level reference system.

## 1. Goal

Build a unified source reference system for `llm-tutor`.

The goal is to make generated learning content traceable:

- inline references are readable and clickable,
- bottom reference lists explain where evidence came from,
- reference items can navigate back to the original product surface,
- internal metadata such as memory entry markers never leaks into rendered UI.

This should turn references from raw Markdown syntax into a consistent product interaction.

## 2. Product Behavior

Inline behavior:

- Markdown footnotes such as `[^1]` render as compact source chips like `[1]`.
- Clicking `[1]` scrolls to the corresponding item in the bottom reference list.
- Inline references stay visually quiet and should not interrupt reading.

Bottom reference behavior:

- Render a `SourceReferences` section below the content.
- Each item shows type, title/summary, and compact metadata.
- Clicking a bottom item navigates to the source page when possible.
- If deep navigation is not available yet, the item should still display the raw source reference and a disabled/limited state.

Marker behavior:

- Internal memory markers such as `<!--m_xxx-->` are hidden from rendered output.
- The renderer should not enable arbitrary raw HTML only to hide markers.
- The Markdown source remains user-editable and durable.

## 3. SourceRef Model

Use a normalized frontend model:

```ts
type SourceSurface =
  | 'chat'
  | 'notebook'
  | 'quiz'
  | 'research'
  | 'book'
  | 'kb'
  | 'web'
  | 'unknown'

type SourceRef = {
  id: string
  label: string
  raw: string
  surface: SourceSurface
  title?: string
  description?: string
  target?: SourceTarget
}

type SourceTarget =
  | { type: 'chat'; sessionId: string; messageId?: string }
  | { type: 'notebook'; entryId: string }
  | { type: 'quiz'; quizId: string; questionId?: string }
  | { type: 'research'; notebookEntryId: string }
  | { type: 'book'; bookId: string; chapterId?: string }
  | { type: 'kb'; knowledgeBaseId: string; documentId: string; chunkId?: string }
  | { type: 'web'; url: string }
```

Supported raw reference patterns:

```text
chat:<session_id>[:message_id]
notebook:<entry_id>
quiz:<quiz_id>[:question_id]
research:<notebook_entry_id>
book:<book_id>[:chapter_id]
kb:<knowledge_base_id>:<doc_id>[:chunk_id]
web:<url>
```

## 4. Architecture

Frontend should own display and navigation:

```text
Markdown text
  -> strip internal memory markers
  -> parse footnote definitions
  -> render markdown content with inline source chips
  -> render SourceReferences list
  -> route bottom reference clicks to product surfaces
```

Backend should keep producing durable, parseable source references:

- Memory keeps Markdown footnotes.
- RAG keeps source chunks.
- Quiz keeps question citations.
- Research keeps source list metadata.

Do not move runtime/session concerns into this feature. Session opening and history should continue to use runtime-backed session APIs.

## 5. Implementation Phases

### Phase 1: Markdown Memory References

Status: completed for the first reusable renderer slice on 2026-06-28.

Tasks:

- [x] Add a source reference parser for Markdown footnote definitions.
- [x] Remove or hide internal memory markers before rendering.
- [x] Render inline `[^n]` references as source chips.
- [x] Render a bottom `SourceReferences` list.
- [x] Clicking inline source chips scrolls to the bottom item.
- [ ] Add tests for parser behavior and marker hiding.

Acceptance:

- Memory preview no longer shows `<!--m_xxx-->`.
- Memory footnotes are visible as clickable source chips.
- Clicking an inline chip scrolls to the matching reference item.

### Phase 2: Navigation Targets

Status: completed for first exact-focus slice on 2026-06-28.

Tasks:

- [x] Add source target parsing for `chat`, `notebook`, `quiz`, `research`, `book`, `kb`, and `web`.
- [x] Add a shared navigation callback or router helper.
- [x] Implement chat session navigation first.
- [x] Implement Notebook entry navigation with exact entry focus.
- [x] Implement Quiz Bank item/question navigation with exact question focus.
- [x] Implement Knowledge Base document/chunk navigation with exact document or chunk focus where current UI supports it.
- [x] Add graceful fallback for unsupported or stale references.

Acceptance:

- Bottom reference items can navigate to at least Chat, Notebook, Quiz, and Knowledge Base surfaces.
- Unsupported references remain readable and do not break rendering.

### Phase 3: Reuse Beyond Memory

Status: implemented.

Tasks:

- [x] Use `SourceReferences` for RAG answer citations.
- [x] Use `SourceReferences` for Quiz question citations.
- [x] Use `SourceReferences` for Research report source lists.
- [x] Use the same visual language in Student Profile references.
- [x] Keep source display compact in chat and richer in detail/review pages.

Notes:

- Chat RAG/web citations now map into `SourceReferences` instead of bespoke citation cards.
- Quiz citations in chat and Space quiz review use the same source reference pattern.
- Notebook entries, including saved research reports, render through `MarkdownMessage` so source footnotes can scroll and navigate.
- Student profile memory cards rely on `MarkdownMessage` source references instead of a separate memory-only reference parser.

Acceptance:

- RAG, Quiz, Research, Memory, and Student Profile references share the same UI pattern.
- The user can tell whether an answer is grounded and where the grounding came from.

### Phase 4: Metadata Enrichment

Status: planned.

Tasks:

- [ ] Add source titles where available.
- [ ] Add document names, chunk numbers, page numbers, or message snippets where available.
- [ ] Add web page title and URL display.
- [ ] Add stale/missing-source indicators.
- [ ] Add hover previews only after the basic click flow is reliable.

Acceptance:

- References are understandable without exposing only raw IDs.
- Source metadata improves trust without cluttering main answers.

## 6. Testing Plan

- Unit-test source ref parsing.
- Unit-test Markdown footnote extraction.
- Unit-test marker stripping.
- Component-test inline chip to bottom reference scrolling where practical.
- Add regression cases for:
  - duplicate refs,
  - missing footnote definitions,
  - unsupported source types,
  - malformed raw refs,
  - `<!--m_xxx-->` marker leakage.

## 7. Design Constraints

- Do not enable arbitrary raw HTML unless there is a separate security review.
- Keep Markdown as the durable storage format for memory.
- Keep UI source refs reusable across product modules.
- Keep navigation best-effort; stale references should degrade gracefully.
- Do not make backend invent citations that the agent/tool did not actually produce.

## 8. Recommended First Slice

Implement Phase 1 first in the Memory preview:

1. Hide `<!--m_xxx-->`.
2. Parse bottom footnotes.
3. Render inline chips.
4. Render bottom reference list.
5. Scroll inline chip clicks to bottom references.

This directly fixes the current rendering issue and creates the reusable base for RAG, Quiz, and Research references.
