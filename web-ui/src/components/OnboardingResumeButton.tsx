import { CircleHelp } from 'lucide-react'
import { useI18n } from '../i18n'

interface Props {
  onClick: () => void
}

export function OnboardingResumeButton({ onClick }: Props) {
  const { language } = useI18n()
  const label = language === 'en-US' ? 'Continue guide' : '继续引导'

  return (
    <button
      type="button"
      className="fixed bottom-40 right-5 z-40 inline-flex h-10 max-w-[calc(100vw-2.5rem)] items-center gap-2 rounded-md border border-blue-200 bg-white px-3 text-sm font-medium text-gray-700 shadow-lg shadow-gray-950/10 hover:border-blue-300 hover:bg-blue-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
      aria-label={label}
      title={label}
      onClick={onClick}
    >
      <CircleHelp size={17} className="shrink-0 text-blue-600" />
      <span className="truncate">{label}</span>
    </button>
  )
}
