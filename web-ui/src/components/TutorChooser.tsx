import { useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { Bot, Check, ChevronDown, Search, Settings2, UserRound } from 'lucide-react'
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
  const [query, setQuery] = useState('')
  const [placement, setPlacement] = useState<TutorChooserPlacement | null>(null)
  const chooserRef = useRef<HTMLDivElement>(null)
  const menuRef = useRef<HTMLDivElement>(null)
  const searchRef = useRef<HTMLInputElement>(null)
  const selectedTutor = selectedTutorId
    ? tutors.find((tutor) => tutor.id === selectedTutorId) ?? null
    : null
  const selectedName = selectedTutor?.name ?? t('chat.tutor.temporary')
  const selectedDescription = selectedTutor
    ? tutorSoulSummary(selectedTutor.soul_markdown)
    : t('chat.tutor.temporary.description')
  const normalizedQuery = query.trim().toLocaleLowerCase()
  const filteredTutors = normalizedQuery
    ? tutors.filter((tutor) => `${tutor.name}\n${tutorSoulSummary(tutor.soul_markdown)}`.toLocaleLowerCase().includes(normalizedQuery))
    : tutors

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
    if (tutors.length > 6) window.requestAnimationFrame(() => searchRef.current?.focus())
    window.addEventListener('resize', updatePlacement)
    window.addEventListener('scroll', updatePlacement, true)
    return () => {
      window.removeEventListener('resize', updatePlacement)
      window.removeEventListener('scroll', updatePlacement, true)
    }
  }, [open, tutors.length, updatePlacement])

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
    setQuery('')
  }

  return (
    <div ref={chooserRef} className="relative flex min-h-11 items-stretch border-t border-blue-50">
      <button
        type="button"
        className="flex min-w-0 flex-1 items-center gap-3 rounded-bl-3xl px-4 py-2 text-left hover:bg-gray-50"
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen((value) => {
          if (!value) setQuery('')
          return !value
        })}
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
          className="fixed z-[100] flex flex-col overflow-hidden rounded-lg border border-gray-200 bg-white shadow-xl shadow-gray-950/10"
          style={placement}
          role="listbox"
          aria-label={t('chat.tutor.select')}
        >
          <div className="flex h-11 shrink-0 items-center justify-between border-b border-gray-100 px-3.5">
            <span className="text-sm font-semibold text-gray-900">{t('chat.tutor.select')}</span>
            <span className="text-xs text-gray-400">{tutors.length}</span>
          </div>
          {tutors.length > 6 && (
            <label className="relative mx-2.5 mt-2.5 block shrink-0">
              <Search size={15} className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
              <input
                ref={searchRef}
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                className="h-9 w-full rounded-md border border-gray-200 bg-gray-50 pl-9 pr-3 text-sm text-gray-900 outline-none placeholder:text-gray-400 focus:border-blue-300 focus:bg-white"
                placeholder={t('chat.tutor.search')}
                aria-label={t('chat.tutor.search')}
              />
            </label>
          )}
          <div className="min-h-0 flex-1 overflow-y-auto p-1.5">
            {filteredTutors.map((tutor) => (
              <TutorOption
                key={tutor.id}
                selected={selectedTutorId === tutor.id}
                title={tutor.name}
                description={tutorSoulSummary(tutor.soul_markdown)}
                onClick={() => select(selectedTutorId === tutor.id ? null : tutor.id)}
              />
            ))}
            {tutors.length === 0 && (
              <div className="px-3 py-4 text-center text-sm text-gray-500">{t('chat.tutor.empty')}</div>
            )}
            {tutors.length > 0 && filteredTutors.length === 0 && (
              <div className="px-3 py-4 text-center text-sm text-gray-500">{t('chat.tutor.noMatching')}</div>
            )}
          </div>
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
  onClick,
}: {
  selected: boolean
  title: string
  description: string
  onClick: () => void
}) {
  return (
    <button
      type="button"
      role="option"
      aria-selected={selected}
      className={`group flex w-full items-center gap-3 rounded-md border px-2.5 py-2 text-left ${
        selected ? 'border-blue-100 bg-blue-50 text-gray-900' : 'border-transparent text-gray-800 hover:bg-gray-50'
      }`}
      onClick={onClick}
    >
      <span className={`flex h-9 w-9 shrink-0 items-center justify-center rounded-md ${
        selected ? 'bg-white text-blue-600 shadow-sm' : 'bg-gray-100 text-gray-500'
      }`}>
        <Bot size={17} />
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-sm font-medium">{title}</span>
        <span className="mt-0.5 block truncate text-xs text-gray-500">{description}</span>
      </span>
      <span className="flex h-5 w-5 shrink-0 items-center justify-center text-blue-600">
        {selected && <Check size={16} />}
      </span>
    </button>
  )
}
