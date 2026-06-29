import { useId } from 'react'
import ReactMarkdown from 'react-markdown'
import type { Components } from 'react-markdown'
import remarkBreaks from 'remark-breaks'
import remarkGfm from 'remark-gfm'
import rehypeAutolinkHeadings from 'rehype-autolink-headings'
import rehypeExternalLinks from 'rehype-external-links'
import rehypeKatex from 'rehype-katex'
import rehypeSlug from 'rehype-slug'
import remarkMath from 'remark-math'

interface Props {
  text: string
  onSourceNavigate?: (target: SourceTarget, reference: SourceReference) => void
}

export function MarkdownMessage({ text, onSourceNavigate }: Props) {
  const rawId = useId()
  const sourceListId = `source-refs-${rawId.replace(/[^a-zA-Z0-9_-]/g, '')}`
  const prepared = prepareMarkdownWithSourceReferences(text, sourceListId)
  const components: Components = {
    a({ href, children, ...props }) {
      if (href?.startsWith(`#${sourceListId}-item-`)) {
        const label = String(children)
        return (
          <a
            {...props}
            href={href}
            className="mx-0.5 align-baseline text-[0.82em] font-semibold text-blue-700 no-underline hover:text-blue-800 hover:underline"
            onClick={(event) => {
              event.preventDefault()
              document.getElementById(href.slice(1))?.scrollIntoView({ behavior: 'smooth', block: 'center' })
            }}
          >
            {label}
          </a>
        )
      }
      return (
        <a {...props} href={href}>
          {children}
        </a>
      )
    },
  }

  return (
    <div className="markdown-message text-sm">
      <ReactMarkdown
        skipHtml
        remarkPlugins={[remarkGfm, remarkBreaks, remarkMath]}
        rehypePlugins={[
          rehypeKatex,
          rehypeSlug,
          [rehypeAutolinkHeadings, { behavior: 'wrap' }],
          [rehypeExternalLinks, { target: '_blank', rel: ['nofollow', 'noopener', 'noreferrer'] }],
        ]}
        components={components}
      >
        {prepared.markdown}
      </ReactMarkdown>
      {prepared.references.length > 0 && (
        <SourceReferences id={sourceListId} references={prepared.references} onNavigate={onSourceNavigate} />
      )}
    </div>
  )
}

type SourceSurface = 'chat' | 'notebook' | 'quiz' | 'research' | 'book' | 'kb' | 'web' | 'unknown'

export type SourceTarget =
  | { type: 'chat'; sessionId: string; messageId?: string }
  | { type: 'notebook'; entryId: string }
  | { type: 'quiz'; quizId: string; questionId?: string }
  | { type: 'research'; notebookEntryId: string }
  | { type: 'book'; bookId: string; chapterId?: string }
  | { type: 'kb'; knowledgeBaseId: string; documentId: string; chunkId?: string }
  | { type: 'web'; url: string }

export interface SourceReference {
  id: string
  label: string
  raw: string
  surface: SourceSurface
  title?: string
  description?: string
  score?: number | null
  target?: SourceTarget
}

export function SourceReferences({
  id,
  references,
  onNavigate,
}: {
  id: string
  references: SourceReference[]
  onNavigate?: (target: SourceTarget, reference: SourceReference) => void
}) {
  return (
    <section id={id} className="mt-5 border-t border-gray-200 pt-4">
      <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-gray-500">Sources</div>
      <ol className="m-0 list-none space-y-2 p-0">
        {references.map((reference) => (
          <li
            key={reference.id}
            id={`${id}-item-${safeReferenceId(reference.label)}`}
            className="scroll-mt-6"
          >
            <button
              className={`flex w-full items-start gap-2 rounded-lg border border-blue-100 bg-white px-3 py-2 text-left text-sm text-gray-700 ${
                reference.target ? 'hover:border-blue-200 hover:bg-blue-50/50' : 'cursor-default'
              }`}
              type="button"
              disabled={!reference.target}
              onClick={() => {
                if (reference.target) onNavigate?.(reference.target, reference)
              }}
            >
              <span className="mt-0.5 inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-blue-600 px-1.5 text-xs font-semibold text-white">
                [{reference.label}]
              </span>
              <span className="min-w-0 flex-1">
                <span className="font-medium text-gray-900">{reference.title || sourceSurfaceLabel(reference.surface)}</span>
                {typeof reference.score === 'number' && (
                  <span className="ml-1 text-xs text-gray-400">{reference.score.toFixed(4)}</span>
                )}
                <span className="mx-1 text-gray-300">·</span>
                <code className="break-all rounded bg-gray-100 px-1.5 py-0.5 text-xs text-gray-600">{reference.raw}</code>
                {reference.description && (
                  <span className="mt-1 block text-xs leading-5 text-gray-600">{reference.description}</span>
                )}
              </span>
            </button>
          </li>
        ))}
      </ol>
    </section>
  )
}

function prepareMarkdownWithSourceReferences(text: string, sourceListId: string) {
  const withoutMarkers = stripInternalMemoryMarkers(text)
  const { markdown, references } = extractFootnoteReferences(withoutMarkers)
  const labels = new Set(references.map((reference) => reference.label))
  const linkedMarkdown = markdown.replace(/\[\^([^\]\s]+)\]/g, (match, label: string) => {
    if (!labels.has(label)) return match
    return `[${label}](#${sourceListId}-item-${safeReferenceId(label)})`
  })
  return { markdown: linkedMarkdown, references }
}

function stripInternalMemoryMarkers(text: string) {
  return text.replace(/<!--\s*m_[a-zA-Z0-9_-]+\s*-->/g, '')
}

function extractFootnoteReferences(text: string) {
  const references: SourceReference[] = []
  const lines = text.split(/\r?\n/)
  const keptLines: string[] = []

  for (const line of lines) {
    const match = line.match(/^\s*\[\^([^\]\s]+)\]:\s*(.+?)\s*$/)
    if (match) {
      const label = match[1]
      const raw = match[2]
      if (label && raw) {
        references.push({
          id: `${label}:${raw}`,
          label,
          raw,
          surface: sourceSurfaceFromRaw(raw),
          target: sourceTargetFromRaw(raw),
        })
      }
      continue
    }
    keptLines.push(line)
  }

  return {
    markdown: stripTrailingFootnoteSeparator(keptLines).join('\n').trimEnd(),
    references,
  }
}

function stripTrailingFootnoteSeparator(lines: string[]) {
  const next = [...lines]
  while (next.length > 0 && next[next.length - 1]?.trim() === '') {
    next.pop()
  }
  if (/^\s*-{3,}\s*$/.test(next[next.length - 1] ?? '')) {
    next.pop()
  }
  return next
}

function sourceSurfaceFromRaw(raw: string): SourceSurface {
  const prefix = raw.split(':', 1)[0]?.toLowerCase()
  if (prefix === 'chat') return 'chat'
  if (prefix === 'notebook') return 'notebook'
  if (prefix === 'quiz') return 'quiz'
  if (prefix === 'research') return 'research'
  if (prefix === 'book') return 'book'
  if (prefix === 'kb') return 'kb'
  if (prefix === 'web' || raw.startsWith('http://') || raw.startsWith('https://')) return 'web'
  return 'unknown'
}

export function sourceTargetFromRaw(raw: string): SourceTarget | undefined {
  if (raw.startsWith('http://') || raw.startsWith('https://')) {
    return { type: 'web', url: raw }
  }

  const [prefix, ...parts] = raw.split(':')
  if (!prefix) return undefined
  const type = prefix.toLowerCase()

  if (type === 'web') {
    const url = parts.join(':')
    return url ? { type: 'web', url } : undefined
  }
  if (type === 'chat') {
    const [sessionId, messageId] = parts
    return sessionId ? { type: 'chat', sessionId, messageId } : undefined
  }
  if (type === 'notebook') {
    const [entryId] = parts
    return entryId ? { type: 'notebook', entryId } : undefined
  }
  if (type === 'quiz') {
    const [quizId, questionId] = parts
    return quizId ? { type: 'quiz', quizId, questionId } : undefined
  }
  if (type === 'research') {
    const [notebookEntryId] = parts
    return notebookEntryId ? { type: 'research', notebookEntryId } : undefined
  }
  if (type === 'book') {
    const [bookId, chapterId] = parts
    return bookId ? { type: 'book', bookId, chapterId } : undefined
  }
  if (type === 'kb') {
    const [knowledgeBaseId, documentId, chunkId] = parts
    return knowledgeBaseId && documentId ? { type: 'kb', knowledgeBaseId, documentId, chunkId } : undefined
  }

  return undefined
}

export function sourceSurfaceLabel(surface: SourceSurface) {
  if (surface === 'chat') return 'Chat'
  if (surface === 'notebook') return 'Notebook'
  if (surface === 'quiz') return 'Quiz'
  if (surface === 'research') return 'Research'
  if (surface === 'book') return 'Book'
  if (surface === 'kb') return 'Knowledge Base'
  if (surface === 'web') return 'Web'
  return 'Source'
}

function safeReferenceId(label: string) {
  return label.replace(/[^a-zA-Z0-9_-]/g, '-')
}
