type Capability = 'chat' | 'deep_solve' | 'code_exec'

interface Props {
  value: Capability
  onChange: (c: Capability) => void
}

export function CapabilitySelector({ value, onChange }: Props) {
  const options: { key: Capability; label: string }[] = [
    { key: 'chat', label: 'Chat' },
    { key: 'deep_solve', label: 'Deep Solve' },
    { key: 'code_exec', label: 'Code Exec' },
  ]
  return (
    <div className="flex border rounded overflow-hidden text-sm">
      {options.map((opt) => (
        <button
          key={opt.key}
          className={`px-4 py-2 ${
            value === opt.key
              ? 'bg-blue-600 text-white'
              : 'bg-white text-gray-700 hover:bg-gray-50'
          }`}
          onClick={() => onChange(opt.key)}
        >
          {opt.label}
        </button>
      ))}
    </div>
  )
}
