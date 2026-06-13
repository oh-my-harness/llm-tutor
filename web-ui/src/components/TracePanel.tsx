import { useState } from 'react'

export interface TraceEntry {
  kind: string
  payload: Record<string, unknown>
  timestamp: number
}

interface Props {
  entries: TraceEntry[]
}

export function TracePanel({ entries }: Props) {
  const [expanded, setExpanded] = useState<Set<number>>(new Set())

  const toggle = (i: number) =>
    setExpanded((prev) => {
      const next = new Set(prev)
      next.has(i) ? next.delete(i) : next.add(i)
      return next
    })

  return (
    <div className="h-full overflow-y-auto p-2 text-xs font-mono">
      <div className="text-gray-500 text-xs uppercase mb-2">Trace</div>
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
