import { Bot, Check, UserRound } from 'lucide-react'
import type { TutorProfile } from '../tutorTypes'

interface Props {
  tutors: TutorProfile[]
  selectedTutorId: string | null | undefined
  onSelect: (tutorId: string | null) => void
  onManage: () => void
}

export function TutorChooser({ tutors, selectedTutorId, onSelect, onManage }: Props) {
  return (
    <section className="mb-6" aria-labelledby="tutor-chooser-title">
      <div className="mb-3 flex items-end justify-between gap-4">
        <div>
          <h2 id="tutor-chooser-title" className="text-base font-semibold text-gray-900">
            这次想和哪位导师交流？
          </h2>
          <p className="mt-1 text-xs text-gray-500">导师身份会随会话保存，之后可以继续同一段学习过程。</p>
        </div>
        <button type="button" className="shrink-0 text-xs text-blue-600 hover:text-blue-700" onClick={onManage}>
          管理导师
        </button>
      </div>

      <div className="grid gap-2 sm:grid-cols-2 lg:grid-cols-3">
        {tutors.map((tutor) => (
          <TutorChoice
            key={tutor.id}
            selected={selectedTutorId === tutor.id}
            title={tutor.name}
            description={tutor.goal || tutor.role}
            icon={<Bot size={18} />}
            onClick={() => onSelect(tutor.id)}
          />
        ))}
        <TutorChoice
          selected={selectedTutorId === null}
          title="临时助手"
          description="适合一次性问题，不保留独立导师身份和私有计划。"
          icon={<UserRound size={18} />}
          onClick={() => onSelect(null)}
        />
      </div>
    </section>
  )
}

function TutorChoice({
  selected,
  title,
  description,
  icon,
  onClick,
}: {
  selected: boolean
  title: string
  description: string
  icon: React.ReactNode
  onClick: () => void
}) {
  return (
    <button
      type="button"
      className={`relative min-h-24 rounded-lg border p-3 text-left transition-colors ${
        selected
          ? 'border-blue-500 bg-blue-50/70 text-gray-900'
          : 'border-gray-200 bg-white text-gray-800 hover:border-gray-300 hover:bg-gray-50'
      }`}
      onClick={onClick}
      aria-pressed={selected}
    >
      <span className="flex items-center gap-2">
        <span className={selected ? 'text-blue-600' : 'text-gray-500'}>{icon}</span>
        <span className="min-w-0 truncate text-sm font-semibold">{title}</span>
      </span>
      <span className="mt-2 line-clamp-2 block text-xs leading-5 text-gray-500">{description}</span>
      {selected && <Check size={15} className="absolute right-3 top-3 text-blue-600" />}
    </button>
  )
}
