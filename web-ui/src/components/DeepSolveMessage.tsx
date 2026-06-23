import { Brain, CheckCircle2, CircleDot, ListChecks, Wrench } from 'lucide-react'
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
}

const stageLabels: Record<string, string> = {
  retrieve: 'Retrieve',
  plan: 'Plan',
  solve: 'Solve',
  verify: 'Verify',
  synthesize: 'Final',
}

export function DeepSolveMessage({ text, events, citations, citationList }: Props) {
  const stages = buildStages(events)
  const plan = events.find((event) => event.kind === 'deep_solve_plan')?.payload
  const steps = events.filter((event) => event.kind === 'deep_solve_step_done')
  const toolEvents = events.filter((event) => event.kind === 'tool_call' || event.kind === 'tool_result')

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
            {steps.map((event, index) => (
              <details key={`${String(event.payload.step_id ?? index)}`} className="rounded-md border border-gray-100 p-2">
                <summary className="cursor-pointer text-sm font-medium text-gray-900">
                  {String(event.payload.title ?? event.payload.step_id ?? `Step ${index + 1}`)}
                </summary>
                {typeof event.payload.summary === 'string' && (
                  <p className="mt-2 text-xs leading-5 text-gray-600">{event.payload.summary}</p>
                )}
              </details>
            ))}
          </div>
        </div>
      )}

      {toolEvents.length > 0 && (
        <div className="flex items-center gap-2 rounded-lg border border-gray-200 bg-white px-3 py-2 text-xs text-gray-600">
          <Wrench size={15} />
          {toolEvents.filter((event) => event.kind === 'tool_call').length} tool calls attached to solve steps
        </div>
      )}

      <div className="rounded-lg border border-blue-100 bg-white p-3">
        <div className="mb-2 text-xs font-semibold uppercase text-gray-500">Final Answer</div>
        <MarkdownMessage text={text} />
        {citations && citations.length > 0 && citationList(citations)}
      </div>
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
