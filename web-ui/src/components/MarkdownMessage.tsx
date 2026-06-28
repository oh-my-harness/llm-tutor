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
}

export function MarkdownMessage({ text }: Props) {
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
            className="mx-0.5 inline-flex h-5 min-w-5 items-center justify-center rounded-full border border-blue-200 bg-blue-50 px-1.5 align-baseline text-[0.72em] font-semibold leading-none text-blue-700 no-underline hover:border-blue-300 hover:bg-blue-100"
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
        <SourceReferences id={sourceListId} references={prepared.references} />
      )}
    </div>
  )
}

type SourceSurface = 'chat' | 'notebook' | 'quiz' | 'research' | 'book' | 'kb' | 'web' | 'unknown'

interface SourceReference {
  id: string
  label: string
  raw: string
  surface: SourceSurface
}

function SourceReferences({ id, references }: { id: string; references: SourceReference[] }) {
  return (
    <section id={id} className="mt-5 border-t border-gray-200 pt-4">
      <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-gray-500">Sources</div>
      <ol className="m-0 list-none space-y-2 p-0">
        {references.map((reference) => (
          <li
            key={reference.id}
            id={`${id}-item-${safeReferenceId(reference.label)}`}
            className="scroll-mt-6 rounded-lg border border-blue-100 bg-white px-3 py-2 text-sm text-gray-700"
          >
            <div className="flex items-start gap-2">
              <span className="mt-0.5 inline-flex h-5 min-w-5 items-center justify-center rounded-full bg-blue-600 px-1.5 text-xs font-semibold text-white">
                {reference.label}
              </span>
              <span className="min-w-0 flex-1">
                <span className="font-medium text-gray-900">{sourceSurfaceLabel(reference.surface)}</span>
                <span className="mx-1 text-gray-300">·</span>
                <code className="break-all rounded bg-gray-100 px-1.5 py-0.5 text-xs text-gray-600">{reference.raw}</code>
              </span>
            </div>
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

function sourceSurfaceLabel(surface: SourceSurface) {
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
