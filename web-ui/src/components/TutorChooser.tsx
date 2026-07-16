import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { Bot, Check, ChevronDown, Settings2, UserRound } from 'lucide-react'
import { useI18n } from '../i18n'
import { placeTutorChooser, type TutorChooserPlacement } from '../tutorChooserPlacement'
import { tutorSoulSummary, type TutorProfile } from '../tutorTypes'

interface Props {
  tutors: TutorProfile[]
  selectedTutorId: string | null | undefined
  onSelect: (tutorId: string | null) => void
  onManage: () => void
}

export function TutorChooser({ tutors, selectedTutorId, onSelect, onManage }: Props) {
  const { t } = useI18n()
  const [open, setOpen] = useState(false)
  const [placement, setPlacement] = useState<TutorChooserPlacement | null>(null)
  const chooserRef = useRef<HTMLDivElement>(null)
  const menuRef = useRef<HTMLDivElement>(null)
  const selectedTutor = selectedTutorId
    ? tutors.find((tutor) => tutor.id === selectedTutorId) ?? null
    : null
  const selectedName = selectedTutor?.name ?? t('chat.tutor.temporary')
  const selectedDescription = selectedTutor
    ? tutorSoulSummary(selectedTutor.soul_markdown)
    : t('chat.tutor.temporary.description')

  const updatePlacement = useCallback(() => {
    const anchor = chooserRef.current?.getBoundingClientRect()
    if (!anchor) return
    setPlacement(placeTutorChooser(anchor, {
      width: window.innerWidth,
      height: window.innerHeight,
    }))
  }, [])

  useLayoutEffect(() => {
    if (!open) {
      setPlacement(null)
      return
    }
    updatePlacement()
    window.addEventListener('resize', updatePlacement)
    window.addEventListener('scroll', updatePlacement, true)
    return () => {
      window.removeEventListener('resize', updatePlacement)
      window.removeEventListener('scroll', updatePlacement, true)
    }
  }, [open, updatePlacement])

  useEffect(() => {
    if (!open) return
    const closeOnOutsidePointer = (event: PointerEvent) => {
      const target = event.target as Node
      if (!chooserRef.current?.contains(target) && !menuRef.current?.contains(target)) setOpen(false)
    }
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setOpen(false)
    }
    document.addEventListener('pointerdown', closeOnOutsidePointer, true)
    document.addEventListener('keydown', closeOnEscape)
    return () => {
      document.removeEventListener('pointerdown', closeOnOutsidePointer, true)
      document.removeEventListener('keydown', closeOnEscape)
    }
  }, [open])

  const select = (tutorId: string | null) => {
    onSelect(tutorId)
    setOpen(false)
  }

  return (
    <div ref={chooserRef} className="relative flex min-h-11 items-stretch border-t border-blue-50">
      <button
        type="button"
        className="flex min-w-0 flex-1 items-center gap-3 rounded-bl-3xl px-4 py-2 text-left hover:bg-gray-50"
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen((value) => !value)}
      >
        <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-blue-50 text-blue-600">
          {selectedTutorId == null ? <UserRound size={16} /> : <Bot size={16} />}
        </span>
        <span className="min-w-0 flex-1">
          <span className="flex items-center gap-2">
            <span className="shrink-0 text-xs text-gray-500">{t('chat.tutor.label')}</span>
            <span className="truncate text-sm font-medium text-gray-900">{selectedName}</span>
          </span>
          <span className="block truncate text-[11px] text-gray-500">{selectedDescription}</span>
        </span>
        <ChevronDown
          size={17}
          className={`shrink-0 text-gray-400 transition-transform ${open ? 'rotate-180' : ''}`}
        />
      </button>

      <button
        type="button"
        className="flex w-12 shrink-0 items-center justify-center rounded-br-3xl border-l border-blue-50 text-gray-500 hover:bg-gray-50 hover:text-gray-900"
        title={t('chat.tutor.manage')}
        aria-label={t('chat.tutor.manage')}
        onClick={onManage}
      >
        <Settings2 size={17} />
      </button>

      {open && placement && createPortal(
        <div
          ref={menuRef}
          className="fixed z-[100] overflow-y-auto rounded-lg border border-gray-200 bg-white p-1.5 shadow-xl shadow-gray-950/10"
          style={placement}
          role="listbox"
          aria-label={t('chat.tutor.select')}
        >
          {tutors.map((tutor) => (
            <TutorOption
              key={tutor.id}
              selected={selectedTutorId === tutor.id}
              title={tutor.name}
              description={tutorSoulSummary(tutor.soul_markdown)}
              icon={<Bot size={17} />}
              onClick={() => select(selectedTutorId === tutor.id ? null : tutor.id)}
            />
          ))}
          {tutors.length === 0 && (
            <div className="px-3 py-3 text-sm text-gray-500">{t('chat.tutor.empty')}</div>
          )}
        </div>,
        document.body,
      )}
    </div>
  )
}

function TutorOption({
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
      role="option"
      aria-selected={selected}
      className={`flex w-full items-center gap-3 rounded-md px-3 py-2 text-left ${
        selected ? 'bg-blue-50 text-gray-900' : 'text-gray-800 hover:bg-gray-50'
      }`}
      onClick={onClick}
    >
      <span className={selected ? 'text-blue-600' : 'text-gray-500'}>{icon}</span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-sm font-medium">{title}</span>
        <span className="block truncate text-xs text-gray-500">{description}</span>
      </span>
      <span className="flex h-5 w-5 shrink-0 items-center justify-center text-blue-600">
        {selected && <Check size={16} />}
      </span>
    </button>
  )
}
