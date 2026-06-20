import { useState } from 'react'
import { PanelRightClose, PanelRightOpen } from 'lucide-react'

export interface TraceEntry {
  kind: string
  payload: Record<string, unknown>
  timestamp: number
}

interface Props {
  entries: TraceEntry[]
  collapsed: boolean
  onToggleCollapsed: () => void
}

export function TracePanel({ entries, collapsed, onToggleCollapsed }: Props) {
  const [expanded, setExpanded] = useState<Set<number>>(new Set())

  const toggle = (i: number) =>
    setExpanded((prev) => {
      const next = new Set(prev)
      next.has(i) ? next.delete(i) : next.add(i)
      return next
    })

  if (collapsed) {
    return (
      <div className="flex h-full flex-col items-center py-3">
        <button
          className="rounded p-2 text-gray-500 hover:bg-gray-100"
          title="Expand trace"
          onClick={onToggleCollapsed}
        >
          <PanelRightOpen size={18} />
        </button>
        <div className="mt-3 [writing-mode:vertical-rl] text-xs font-medium uppercase tracking-wide text-gray-400">
          Trace
        </div>
      </div>
    )
  }

  return (
    <div className="h-full overflow-y-auto p-2 text-xs font-mono">
      <div className="mb-2 flex items-center justify-between">
        <div className="text-xs uppercase text-gray-500">Trace</div>
        <button
          className="rounded p-1.5 text-gray-500 hover:bg-gray-100"
          title="Collapse trace"
          onClick={onToggleCollapsed}
        >
          <PanelRightClose size={16} />
        </button>
      </div>
      {entries.map((entry, i) => (
        <div key={i} className="border-b border-gray-100 py-1">
          <button
            className="w-full text-left flex gap-2 items-center"
            onClick={() => toggle(i)}
          >
            <span className="text-gray-400">
              {expanded.has(i) ? '▼' : '▶'}
            </span>
            <span className="text-blue-600">{entry.kind}</span>
          </button>
          {expanded.has(i) && (
            <pre className="mt-1 text-gray-600 pl-4 text-xs overflow-x-auto">
              {JSON.stringify(entry.payload, null, 2)}
            </pre>
          )}
        </div>
      ))}
    </div>
  )
}
