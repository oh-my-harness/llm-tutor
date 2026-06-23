import { Brain, CheckCircle2, CircleDot, ListChecks, MessageSquareText, Wrench } from 'lucide-react'
import type { ReactNode } from 'react'
import { MarkdownMessage } from './MarkdownMessage'

export interface DeepSolveTraceEntry {
  kind: string
  payload: Record<string, unknown>
  timestamp: number
}

interface Citation {
  index: number
  source: string
  text: string
  score?: number | null
}

interface Props {
  text: string
  events: DeepSolveTraceEntry[]
  citations?: Citation[]
  citationList: (citations: Citation[]) => ReactNode
  onAskStep?: (step: { id: string; title: string; summary?: string }) => void
}

const stageLabels: Record<string, string> = {
  retrieve: 'Retrieve',
  plan: 'Plan',
  solve: 'Solve',
  verify: 'Verify',
  synthesize: 'Final',
}

export function DeepSolveMessage({ text, events, citations, citationList, onAskStep }: Props) {
  const stages = buildStages(events)
  const plan = events.find((event) => event.kind === 'deep_solve_plan')?.payload
  const steps = events.filter((event) => event.kind === 'deep_solve_step_done')
  const toolEvents = events.filter((event) => event.kind === 'tool_call' || event.kind === 'tool_result')
  const toolPairs = pairToolEvents(toolEvents)
  const citationsFromEvents = citationsFromDeepSolveEvents(events)
  const allCitations = citations && citations.length > 0 ? citations : citationsFromEvents

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2 text-sm font-semibold text-blue-800">
        <Brain size={18} />
        Deep Solve
      </div>

      {stages.length > 0 && (
        <div className="rounded-lg border border-blue-100 bg-white p-3">
          <div className="mb-3 flex items-center gap-2 text-xs font-semibold uppercase text-gray-500">
            <ListChecks size={15} />
            Timeline
          </div>
          <div className="space-y-2">
            {stages.map((stage) => (
              <details key={stage.stage} className="group rounded-md bg-blue-50/60 px-3 py-2" open={stage.stage === 'synthesize'}>
                <summary className="flex cursor-pointer list-none items-center gap-2 text-sm font-medium text-gray-900">
                  {stage.done ? (
                    <CheckCircle2 size={16} className="text-blue-600" />
                  ) : (
                    <CircleDot size={16} className="text-gray-400" />
                  )}
                  <span>{stage.title || stageLabels[stage.stage] || stage.stage}</span>
                  {stage.done && <span className="ml-auto text-xs text-blue-700">done</span>}
                </summary>
                {stage.summary && (
                  <p className="mt-2 pl-6 text-xs leading-5 text-gray-600">{stage.summary}</p>
                )}
              </details>
            ))}
          </div>
        </div>
      )}

      {plan && (
        <div className="rounded-lg border border-gray-200 bg-white p-3">
          <div className="text-xs font-semibold uppercase text-gray-500">Plan</div>
          {typeof plan.analysis === 'string' && (
            <p className="mt-2 text-sm text-gray-700">{plan.analysis}</p>
          )}
          {Array.isArray(plan.steps) && (
            <ol className="mt-3 space-y-2 text-sm text-gray-700">
              {plan.steps.map((step, index) => {
                const item = step as Record<string, unknown>
                return (
                  <li key={`${String(item.id ?? index)}`} className="flex gap-2">
                    <span className="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-blue-100 text-xs font-semibold text-blue-700">
                      {index + 1}
                    </span>
                    <span>{String(item.goal ?? item.id ?? 'Solve step')}</span>
                  </li>
                )
              })}
            </ol>
          )}
        </div>
      )}

      {steps.length > 0 && (
        <div className="rounded-lg border border-gray-200 bg-white p-3">
          <div className="text-xs font-semibold uppercase text-gray-500">Step Results</div>
          <div className="mt-3 space-y-2">
            {steps.map((event, index) => {
              const stepId = String(event.payload.step_id ?? '')
              const title = String(event.payload.title ?? event.payload.step_id ?? `Step ${index + 1}`)
              const summary = typeof event.payload.summary === 'string' ? event.payload.summary : undefined
              return (
              <details key={`${String(event.payload.step_id ?? index)}`} className="rounded-md border border-gray-100 p-2">
                <summary className="cursor-pointer text-sm font-medium text-gray-900">
                  {title}
                </summary>
                {summary && (
                  <p className="mt-2 text-xs leading-5 text-gray-600">{summary}</p>
                )}
                <StepTools stepId={stepId} toolPairs={toolPairs} />
                {onAskStep && stepId && (
                  <button
                    className="mt-3 inline-flex items-center gap-1.5 rounded-md border border-blue-100 px-2 py-1 text-xs font-medium text-blue-700 hover:bg-blue-50"
                    type="button"
                    onClick={() => onAskStep({ id: stepId, title, summary })}
                  >
                    <MessageSquareText size={14} />
                    Ask about this step
                  </button>
                )}
              </details>
              )
            })}
          </div>
        </div>
      )}

      {(toolPairs.length > 0 || citationsFromEvents.length > 0) && (
        <div className="rounded-lg border border-gray-200 bg-white p-3">
          <div className="mb-3 flex items-center gap-2 text-xs font-semibold uppercase text-gray-500">
            <Wrench size={15} />
            Evidence
          </div>
          {toolPairs.length > 0 && (
            <div className="space-y-2">
              {toolPairs.map((tool, index) => (
                <ToolEvidence key={`${tool.id}-${index}`} tool={tool} />
              ))}
            </div>
          )}
          {citationsFromEvents.length > 0 && (
            <div className="mt-3 border-t border-gray-100 pt-3">
              {citationList(citationsFromEvents)}
            </div>
          )}
        </div>
      )}

      <div className="rounded-lg border border-blue-100 bg-white p-3">
        <div className="mb-2 text-xs font-semibold uppercase text-gray-500">Final Answer</div>
        <MarkdownMessage text={text} />
        {allCitations.length > 0 && citationList(allCitations)}
      </div>
    </div>
  )
}

interface ToolPair {
  id: string
  stepId?: string
  name: string
  args?: unknown
  ok?: boolean
  details?: unknown
}

function StepTools({ stepId, toolPairs }: { stepId: string; toolPairs: ToolPair[] }) {
  const tools = toolPairs.filter((tool) => tool.stepId === stepId)
  if (tools.length === 0) return null

  return (
    <div className="mt-3 space-y-2 border-t border-gray-100 pt-2">
      {tools.map((tool, index) => (
        <ToolEvidence key={`${tool.id}-${index}`} tool={tool} compact />
      ))}
    </div>
  )
}

function ToolEvidence({ tool, compact = false }: { tool: ToolPair; compact?: boolean }) {
  return (
    <details className={`${compact ? 'bg-gray-50' : 'bg-blue-50/40'} rounded-md px-3 py-2`}>
      <summary className="flex cursor-pointer list-none items-center gap-2 text-xs font-medium text-gray-800">
        <Wrench size={14} className="text-blue-600" />
        <span>{tool.name}</span>
        {tool.stepId && <span className="text-gray-400">step {tool.stepId}</span>}
        {typeof tool.ok === 'boolean' && (
          <span className={`ml-auto ${tool.ok ? 'text-blue-700' : 'text-red-600'}`}>
            {tool.ok ? 'ok' : 'error'}
          </span>
        )}
      </summary>
      <div className="mt-2 space-y-2 pl-5 text-xs text-gray-600">
        {tool.args !== undefined && <JsonBlock label="Args" value={tool.args} />}
        {tool.details !== undefined && tool.details !== null && (
          <JsonBlock label="Result" value={tool.details} />
        )}
      </div>
    </details>
  )
}

function JsonBlock({ label, value }: { label: string; value: unknown }) {
  return (
    <div>
      <div className="mb-1 font-medium text-gray-500">{label}</div>
      <pre className="max-h-40 overflow-auto rounded bg-gray-900 p-2 text-[11px] leading-4 text-gray-100">
        {formatJson(value)}
      </pre>
    </div>
  )
}

function buildStages(events: DeepSolveTraceEntry[]) {
  const order = ['retrieve', 'plan', 'solve', 'verify', 'synthesize']
  const stages = new Map<string, { stage: string; title?: string; summary?: string; done: boolean }>()

  for (const event of events) {
    const stage = typeof event.payload.stage === 'string' ? event.payload.stage : undefined
    if (!stage) continue
    const existing = stages.get(stage) ?? { stage, done: false }
    if (event.kind === 'deep_solve_stage_start' && typeof event.payload.title === 'string') {
      existing.title = event.payload.title
    }
    if (event.kind === 'deep_solve_stage_done') {
      existing.done = true
      if (typeof event.payload.title === 'string') existing.title = event.payload.title
      if (typeof event.payload.summary === 'string') existing.summary = event.payload.summary
    }
    stages.set(stage, existing)
  }

  return Array.from(stages.values()).sort((a, b) => {
    const ai = order.indexOf(a.stage)
    const bi = order.indexOf(b.stage)
    return (ai === -1 ? 99 : ai) - (bi === -1 ? 99 : bi)
  })
}

function pairToolEvents(events: DeepSolveTraceEntry[]): ToolPair[] {
  const pairs = new Map<string, ToolPair>()

  for (const event of events) {
    const id = String(event.payload.tool_use_id ?? `${event.kind}-${event.timestamp}`)
    const existing = pairs.get(id) ?? {
      id,
      name: String(event.payload.tool ?? event.payload.tool_name ?? 'tool'),
      stepId: typeof event.payload.step_id === 'string' ? event.payload.step_id : undefined,
    }

    if (event.kind === 'tool_call') {
      existing.name = String(event.payload.tool ?? event.payload.tool_name ?? existing.name)
      existing.args = event.payload.args
      existing.stepId = typeof event.payload.step_id === 'string' ? event.payload.step_id : existing.stepId
    }

    if (event.kind === 'tool_result') {
      existing.ok = typeof event.payload.ok === 'boolean' ? event.payload.ok : undefined
      existing.details = event.payload.details
      existing.stepId = typeof event.payload.step_id === 'string' ? event.payload.step_id : existing.stepId
    }

    pairs.set(id, existing)
  }

  return Array.from(pairs.values())
}

function citationsFromDeepSolveEvents(events: DeepSolveTraceEntry[]): Citation[] {
  return events.flatMap((event) => {
    if (event.kind !== 'rag_citations' && !(event.kind === 'tool_result' && event.payload.tool === 'rag_search')) {
      return []
    }
    const details = event.payload.details
    if (!details || typeof details !== 'object') return []
    const sources = (details as { sources?: unknown }).sources
    if (!Array.isArray(sources)) return []
    return sources
      .map((source): Citation | null => {
        if (!source || typeof source !== 'object') return null
        const item = source as Record<string, unknown>
        return {
          index: typeof item.index === 'number' ? item.index : 0,
          source: typeof item.source === 'string' ? item.source : 'source',
          text: typeof item.text === 'string' ? item.text : '',
          score: typeof item.score === 'number' ? item.score : null,
        }
      })
      .filter((source): source is Citation => Boolean(source && source.text))
  })
}

function formatJson(value: unknown) {
  if (typeof value === 'string') return value
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}
