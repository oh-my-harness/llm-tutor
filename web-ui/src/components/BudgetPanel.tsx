interface Props {
  spent: number
  limit: number
  warning: boolean
}

export function BudgetPanel({ spent, limit, warning }: Props) {
  const pct = Math.min((spent / limit) * 100, 100)
  return (
    <div className={`p-3 rounded border text-sm ${warning ? 'border-yellow-400 bg-yellow-50' : 'border-gray-200'}`}>
      <div className="flex justify-between mb-1">
        <span className="text-gray-600">Budget</span>
        <span className={warning ? 'text-yellow-600 font-medium' : 'text-gray-800'}>
          ${spent.toFixed(4)} / ${limit.toFixed(2)}
        </span>
      </div>
      <div className="h-1.5 bg-gray-200 rounded overflow-hidden">
        <div
          className={`h-full rounded transition-all ${pct > 80 ? 'bg-yellow-400' : 'bg-blue-500'}`}
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  )
}
