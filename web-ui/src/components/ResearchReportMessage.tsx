import { BookOpen, FileText, RefreshCw, SearchCheck } from 'lucide-react'
import { MarkdownMessage, SourceReferences, sourceTargetFromRaw } from './MarkdownMessage'
import type { SourceReference, SourceTarget } from './MarkdownMessage'

interface Props {
  text: string
  sources?: SourceReference[]
  onSaveToNotebook?: (markdown: string) => Promise<void>
  onRegenerate?: (markdown: string) => void
  onIngestSources?: (sources: SourceReference[], markdown: string) => Promise<void>
  onSourceNavigate?: (target: SourceTarget, reference: SourceReference) => void
}

export function ResearchReportMessage({
  text,
  sources = [],
  onSaveToNotebook,
  onRegenerate,
  onIngestSources,
  onSourceNavigate,
}: Props) {
  const title = researchReportTitle(text)
  const sourceReferences = sources.length > 0 ? sources : sourceReferencesFromMarkdown(text)
  const sourceStats = sourceSummary(sourceReferences)

  return (
    <article className="overflow-hidden rounded-lg border border-blue-100 bg-white">
      <header className="border-b border-blue-50 bg-blue-50/60 px-4 py-3">
        <div className="flex flex-wrap items-start justify-between gap-3">
          <div className="min-w-0">
            <div className="flex items-center gap-2 text-xs font-semibold uppercase text-blue-700">
              <SearchCheck size={15} />
              Research report
            </div>
            <h2 className="mt-1 truncate text-base font-semibold text-gray-950">{title}</h2>
            <div className="mt-1 flex flex-wrap gap-2 text-xs text-gray-500">
              <span>{sourceReferences.length} sources</span>
              {sourceStats.web > 0 && <span>{sourceStats.web} web</span>}
              {sourceStats.kb > 0 && <span>{sourceStats.kb} knowledge</span>}
            </div>
          </div>
          {(onRegenerate || onSaveToNotebook || onIngestSources) && (
            <div className="flex flex-wrap gap-2">
          {onRegenerate && (
            <button
              className="inline-flex h-8 items-center gap-2 rounded-md border border-blue-100 bg-white px-3 text-xs font-medium text-blue-700 hover:bg-blue-50"
              type="button"
              onClick={() => onRegenerate(text)}
            >
              <RefreshCw size={15} />
              Regenerate
            </button>
          )}
              {onSaveToNotebook && (
                <button
              className="inline-flex h-8 items-center gap-2 rounded-md border border-blue-100 bg-white px-3 text-xs font-medium text-blue-700 hover:bg-blue-50"
              type="button"
              onClick={() => {
                void onSaveToNotebook(text)
              }}
            >
              <FileText size={15} />
              保存到笔记本
                </button>
              )}
              {onIngestSources && sourceReferences.length > 0 && (
                <button
                  className="inline-flex h-8 items-center gap-2 rounded-md border border-blue-100 bg-white px-3 text-xs font-medium text-blue-700 hover:bg-blue-50"
                  type="button"
                  onClick={() => {
                    void onIngestSources(sourceReferences, text)
                  }}
                >
                  <BookOpen size={15} />
                  Add sources to KB
                </button>
              )}
            </div>
          )}
        </div>
      </header>
      <div className="space-y-4 px-4 py-4">
        <MarkdownMessage text={text} onSourceNavigate={onSourceNavigate} />
        <section className="rounded-lg border border-gray-100 bg-gray-50 px-3 py-3">
          <div className="mb-2 flex items-center gap-2 text-xs font-semibold uppercase text-gray-500">
            <BookOpen size={14} />
            Research sources
          </div>
          {sourceReferences.length > 0 ? (
            <SourceReferences
              id={`research-sources-${stableId(title)}`}
              references={sourceReferences}
              onNavigate={onSourceNavigate}
            />
          ) : (
            <p className="text-sm text-amber-700">
              No structured sources were attached to this report yet.
            </p>
          )}
        </section>
      </div>
    </article>
  )
}

export function looksLikeResearchReport(text: string) {
  const normalized = text.toLowerCase()
  const hasSources = /(^|\n)#{1,3}\s*sources\b/i.test(text)
  const hasSummary = /(^|\n)#{1,3}\s*summary\b/i.test(text)
  const hasFindings = /(^|\n)#{1,3}\s*(key findings|findings)\b/i.test(text)
  return hasSources && (hasSummary || hasFindings || normalized.includes('## analysis'))
}

function researchReportTitle(text: string) {
  const heading = text
    .split('\n')
    .map((line) => line.trim())
    .find((line) => /^#\s+/.test(line))
  if (heading) return heading.replace(/^#\s+/, '').trim()
  return 'Research Report'
}

function sourceReferencesFromMarkdown(text: string): SourceReference[] {
  const sourcesSection = text.split(/\n#{1,3}\s+sources\b/i)[1]
  if (!sourcesSection) return []
  const lines = sourcesSection
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean)
    .filter((line) => !/^#{1,3}\s+/.test(line))

  const references: SourceReference[] = []
  for (const line of lines) {
    const url = line.match(/https?:\/\/[^\s)>\]]+/)?.[0]
    const label = String(references.length + 1)
    const title = line
      .replace(/^[-*]\s*/, '')
      .replace(/^\d+[.)]\s*/, '')
      .replace(/^\[\d+\]\s*/, '')
      .replace(url ?? '', '')
      .replace(/[-–—]\s*$/, '')
      .trim()
    const raw = url || line
    const target = sourceTargetFromRaw(raw)
    references.push({
      id: `${label}:${raw}`,
      label,
      raw,
      surface: target?.type === 'web' ? 'web' : 'unknown',
      title: title || url || `Source ${label}`,
      description: line,
      metadata: {
        url,
        missingReason: target ? undefined : 'No navigable URL was found in this source line.',
      },
      target,
    })
  }
  return references
}

function sourceSummary(references: SourceReference[]) {
  return references.reduce(
    (summary, reference) => {
      if (reference.surface === 'web' || reference.target?.type === 'web') summary.web += 1
      if (reference.surface === 'kb' || reference.target?.type === 'kb') summary.kb += 1
      return summary
    },
    { web: 0, kb: 0 },
  )
}

function stableId(value: string) {
  return value.toLowerCase().replace(/[^a-z0-9_-]+/g, '-').replace(/^-+|-+$/g, '') || 'report'
}
