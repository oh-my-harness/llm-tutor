import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  BookMarked,
  CheckCircle2,
  ChevronLeft,
  ChevronRight,
  Edit3,
  FileQuestion,
  FileText,
  Link2,
  NotebookPen,
  Plus,
  RefreshCw,
  Save,
  Send,
  Tags,
  Target,
  Trash2,
  UserRound,
  X,
} from 'lucide-react'
import type { QuizQuestion, QuizSession } from '../quizTypes'
import { MarkdownMessage, SourceReferences, sourceTargetFromRaw } from './MarkdownMessage'
import type { SourceReference, SourceTarget } from './MarkdownMessage'

type SpaceTab = 'notebook' | 'quiz_bank' | 'student_profile'
type QuizSourceFilter = 'all' | 'knowledge_base' | 'conversation' | 'space' | 'notebook'

interface NotebookEntry {
  id: string
  space_id: string
  entry_type: 'note' | 'research_report' | 'chat_answer' | 'source_snippet' | 'quiz_summary' | 'deep_solve_result'
  title: string
  markdown: string
  metadata?: Record<string, unknown> | null
  source_session_id?: string | null
  source_message_id?: string | null
  created_at: string
  updated_at: string
  tags?: string[]
  links?: NotebookLink[]
  backlinks?: NotebookBacklink[]
}

interface NotebookLink {
  raw: string
  target: string
  alias?: string | null
  target_id?: string | null
  target_title?: string | null
  resolved: boolean
}

interface NotebookBacklink {
  source_entry_id: string
  source_title: string
  raw: string
  alias?: string | null
  snippet: string
}

interface Book {
  id: string
  title: string
}

interface MemoryFile {
  path: string
  level: string
  name: string
  markdown: string
}

const tabs: Array<{ key: SpaceTab; label: string; icon: typeof NotebookPen }> = [
  { key: 'notebook', label: 'Notebook', icon: NotebookPen },
  { key: 'quiz_bank', label: 'Quiz Bank', icon: FileQuestion },
  { key: 'student_profile', label: 'Student Profile', icon: UserRound },
]

const profileMemoryPaths = ['L3/profile.md', 'L3/recent.md', 'L3/teaching_strategy.md']

const quizSourceFilters: Array<{ key: QuizSourceFilter; label: string }> = [
  { key: 'all', label: 'All' },
  { key: 'knowledge_base', label: 'Knowledge' },
  { key: 'conversation', label: 'Conversation' },
  { key: 'space', label: 'Space refs' },
  { key: 'notebook', label: 'Notebook' },
]

type SpaceFocusTarget = Extract<SourceTarget, { type: 'notebook' | 'quiz' | 'research' }>

export function SpacePage({
  focusTarget,
  onSourceNavigate,
}: {
  focusTarget?: SpaceFocusTarget | null
  onSourceNavigate?: (target: SourceTarget, reference: SourceReference) => void
}) {
  const [activeTab, setActiveTab] = useState<SpaceTab>('notebook')
  const [quizzes, setQuizzes] = useState<QuizSession[]>([])
  const [activeQuizId, setActiveQuizId] = useState<string | null>(null)
  const [quizSourceFilter, setQuizSourceFilter] = useState<QuizSourceFilter>('all')
  const [notebookEntries, setNotebookEntries] = useState<NotebookEntry[]>([])
  const [activeNotebookId, setActiveNotebookId] = useState<string | null>(null)
  const [memoryFiles, setMemoryFiles] = useState<MemoryFile[]>([])
  const [draftTitle, setDraftTitle] = useState('')
  const [draftMarkdown, setDraftMarkdown] = useState('')
  const [editingNotebookId, setEditingNotebookId] = useState<string | null>(null)
  const [editTitle, setEditTitle] = useState('')
  const [editMarkdown, setEditMarkdown] = useState('')
  const [editingMemoryPath, setEditingMemoryPath] = useState<string | null>(null)
  const [memoryDraft, setMemoryDraft] = useState('')
  const [questionIndex, setQuestionIndex] = useState(0)
  const [status, setStatus] = useState('Ready')
  const [loading, setLoading] = useState(false)

  const filteredQuizzes = useMemo(
    () => quizzes.filter((quiz) => quizSourceFilter === 'all' || quizSourceType(quiz) === quizSourceFilter),
    [quizzes, quizSourceFilter],
  )
  const activeQuiz = filteredQuizzes.find((quiz) => quiz.id === activeQuizId) ?? null
  const activeQuestion = activeQuiz?.questions[questionIndex] ?? null
  const activeNotebookEntry = notebookEntries.find((entry) => entry.id === activeNotebookId) ?? null
  const profile = useMemo(() => buildProfile(quizzes), [quizzes])

  useEffect(() => {
    setActiveQuizId((current) =>
      current && filteredQuizzes.some((quiz) => quiz.id === current)
        ? current
        : filteredQuizzes[0]?.id ?? null,
    )
  }, [filteredQuizzes])

  const refreshQuizzes = useCallback(async () => {
    setLoading(true)
    try {
      const res = await fetch('/api/quizzes')
      const data = await safeJson(res)
      if (!res.ok) throw new Error(errorMessage(data, res.status))
      const items = (data.quizzes ?? []) as QuizSession[]
      setQuizzes(items)
      setActiveQuizId((current) => current && items.some((quiz) => quiz.id === current) ? current : items[0]?.id ?? null)
      setStatus(items.length ? 'Quiz records loaded' : 'No quiz records yet')
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }, [])

  const refreshNotebook = useCallback(async () => {
    setLoading(true)
    try {
      const res = await fetch('/api/notebook/entries?space_id=default')
      const data = await safeJson(res)
      if (!res.ok) throw new Error(errorMessage(data, res.status))
      const entries = (data.entries ?? []) as NotebookEntry[]
      setNotebookEntries(entries)
      setActiveNotebookId((current) => current && entries.some((entry) => entry.id === current) ? current : entries[0]?.id ?? null)
      setStatus(entries.length ? 'Notebook entries loaded' : 'No notebook entries yet')
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }, [])

  const refreshMemory = useCallback(async () => {
    setLoading(true)
    try {
      const res = await fetch('/api/memory/files')
      const data = await safeJson(res)
      if (!res.ok) throw new Error(errorMessage(data, res.status))
      setMemoryFiles((data.files ?? []) as MemoryFile[])
      setStatus('Memory files loaded')
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void refreshQuizzes()
    void refreshNotebook()
    void refreshMemory()
  }, [refreshNotebook, refreshQuizzes, refreshMemory])

  useEffect(() => {
    if (focusTarget?.type === 'quiz' && focusTarget.quizId === activeQuizId) return
    setQuestionIndex(0)
  }, [activeQuizId, focusTarget])

  useEffect(() => {
    if (!focusTarget) return
    if (focusTarget.type === 'notebook' || focusTarget.type === 'research') {
      const entryId = focusTarget.type === 'notebook' ? focusTarget.entryId : focusTarget.notebookEntryId
      setActiveTab('notebook')
      if (notebookEntries.some((entry) => entry.id === entryId)) {
        setActiveNotebookId(entryId)
        setStatus(`Opened notebook source: ${entryId}`)
      } else if (notebookEntries.length > 0) {
        setStatus(`Notebook source not found: ${entryId}`)
      }
      return
    }

    setActiveTab('quiz_bank')
    if (quizzes.some((quiz) => quiz.id === focusTarget.quizId)) {
      setActiveQuizId(focusTarget.quizId)
      const quiz = quizzes.find((item) => item.id === focusTarget.quizId)
      const nextQuestionIndex = quiz?.questions.findIndex((question) => question.id === focusTarget.questionId) ?? -1
      if (nextQuestionIndex >= 0) {
        setQuestionIndex(nextQuestionIndex)
        setStatus(`Opened quiz source: ${focusTarget.quizId} / ${focusTarget.questionId}`)
      } else {
        setQuestionIndex(0)
        setStatus(`Opened quiz source: ${focusTarget.quizId}`)
      }
    } else if (quizzes.length > 0) {
      setStatus(`Quiz source not found: ${focusTarget.quizId}`)
    }
  }, [focusTarget, notebookEntries, quizzes])

  const refreshActiveTab = () => {
    if (activeTab === 'notebook') void refreshNotebook()
    else if (activeTab === 'quiz_bank') void refreshQuizzes()
    else void refreshMemory()
  }

  const createNotebookEntry = async () => {
    const title = draftTitle.trim() || 'Untitled note'
    const markdown = draftMarkdown.trim()
    if (!markdown) {
      setStatus('Notebook markdown is empty')
      return
    }
    setLoading(true)
    try {
      const res = await fetch('/api/notebook/entries', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          space_id: 'default',
          entry_type: 'note',
          title,
          markdown,
        }),
      })
      const data = await safeJson(res)
      if (!res.ok) throw new Error(errorMessage(data, res.status))
      const entry = data.entry as NotebookEntry
      setNotebookEntries((items) => [entry, ...items.filter((item) => item.id !== entry.id)])
      setActiveNotebookId(entry.id)
      setDraftTitle('')
      setDraftMarkdown('')
      setStatus('Notebook entry created')
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }

  const createNotebookEntryFromLink = async (title: string) => {
    const cleanTitle = title.trim() || 'Untitled note'
    setLoading(true)
    try {
      const res = await fetch('/api/notebook/entries', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          space_id: 'default',
          entry_type: 'note',
          title: cleanTitle,
          markdown: `# ${cleanTitle}\n\n`,
          metadata: {
            created_from_unresolved_link: true,
          },
        }),
      })
      const data = await safeJson(res)
      if (!res.ok) throw new Error(errorMessage(data, res.status))
      const entry = data.entry as NotebookEntry
      setNotebookEntries((items) => [entry, ...items.filter((item) => item.id !== entry.id)])
      setActiveNotebookId(entry.id)
      setStatus(`Created linked note: ${entry.title}`)
      startEditNotebookEntry(entry)
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }

  const deleteNotebookEntry = async (entry: NotebookEntry) => {
    if (!window.confirm(`Delete "${entry.title}"?`)) return
    const previous = notebookEntries
    setNotebookEntries((items) => items.filter((item) => item.id !== entry.id))
    setActiveNotebookId((current) => current === entry.id ? null : current)
    try {
      const res = await fetch(`/api/notebook/entries/${encodeURIComponent(entry.id)}`, { method: 'DELETE' })
      if (!res.ok) {
        const data = await safeJson(res)
        throw new Error(errorMessage(data, res.status))
      }
      setStatus('Notebook entry deleted')
    } catch (err) {
      setNotebookEntries(previous)
      setStatus(err instanceof Error ? err.message : String(err))
    }
  }

  const startEditNotebookEntry = (entry: NotebookEntry) => {
    setEditingNotebookId(entry.id)
    setEditTitle(entry.title)
    setEditMarkdown(entry.markdown)
  }

  const cancelEditNotebookEntry = () => {
    setEditingNotebookId(null)
    setEditTitle('')
    setEditMarkdown('')
  }

  const saveNotebookEntry = async (entry: NotebookEntry) => {
    const title = editTitle.trim() || 'Untitled note'
    const markdown = editMarkdown.trim()
    if (!markdown) {
      setStatus('Notebook markdown is empty')
      return
    }
    setLoading(true)
    try {
      const res = await fetch(`/api/notebook/entries/${encodeURIComponent(entry.id)}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ title, markdown }),
      })
      const data = await safeJson(res)
      if (!res.ok) throw new Error(errorMessage(data, res.status))
      const updated = data.entry as NotebookEntry
      setNotebookEntries((items) => items.map((item) => item.id === updated.id ? updated : item))
      setEditingNotebookId(null)
      setStatus('Notebook entry updated')
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }

  const sendNotebookEntryToBook = async (entry: NotebookEntry) => {
    setLoading(true)
    try {
      const booksRes = await fetch('/api/books')
      const booksData = await safeJson(booksRes)
      if (!booksRes.ok) throw new Error(errorMessage(booksData, booksRes.status))
      let books = (booksData.books ?? []) as Book[]
      if (books.length === 0) {
        const createBookRes = await fetch('/api/books', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            title: 'Notebook Exports',
            description: 'Notebook entries promoted into book chapters.',
          }),
        })
        const createBookData = await safeJson(createBookRes)
        if (!createBookRes.ok) throw new Error(errorMessage(createBookData, createBookRes.status))
        books = [createBookData.book as Book]
      }
      const targetBook = books[0]
      if (!targetBook) throw new Error('No target book available')
      const chapterRes = await fetch(`/api/books/${encodeURIComponent(targetBook.id)}/chapters`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          title: entry.title,
          markdown: entry.markdown,
          source_notebook_entry_id: entry.id,
          source_session_id: entry.source_session_id,
        }),
      })
      const chapterData = await safeJson(chapterRes)
      if (!chapterRes.ok) throw new Error(errorMessage(chapterData, chapterRes.status))
      setStatus(`Sent to book: ${targetBook.title}`)
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }

  const deleteQuiz = async (quiz: QuizSession) => {
    if (!window.confirm(`Delete "${quiz.title}"?`)) return
    const previous = quizzes
    setQuizzes((items) => items.filter((item) => item.id !== quiz.id))
    setActiveQuizId((current) => current === quiz.id ? null : current)
    try {
      const res = await fetch(`/api/quizzes/${encodeURIComponent(quiz.id)}`, { method: 'DELETE' })
      if (!res.ok) {
        const data = await safeJson(res)
        throw new Error(errorMessage(data, res.status))
      }
      setStatus('Quiz deleted')
    } catch (err) {
      setQuizzes(previous)
      setStatus(err instanceof Error ? err.message : String(err))
    }
  }

  const startEditMemory = (file: MemoryFile) => {
    setEditingMemoryPath(file.path)
    setMemoryDraft(file.markdown)
  }

  const cancelEditMemory = () => {
    setEditingMemoryPath(null)
    setMemoryDraft('')
  }

  const saveMemoryFile = async (path: string) => {
    if (!memoryDraft.trim()) {
      setStatus('Memory markdown is empty')
      return
    }
    setLoading(true)
    try {
      const res = await fetch(`/api/memory/file?path=${encodeURIComponent(path)}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ markdown: memoryDraft }),
      })
      const data = await safeJson(res)
      if (!res.ok) throw new Error(errorMessage(data, res.status))
      const updated = data.file as MemoryFile
      setMemoryFiles((items) => items.map((item) => item.path === updated.path ? updated : item))
      setEditingMemoryPath(null)
      setMemoryDraft('')
      setStatus('Memory file saved')
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }

  return (
    <main className="flex h-full min-h-0 bg-white">
      <aside className="flex w-72 shrink-0 flex-col border-r border-gray-200 bg-gray-50">
        <div className="px-5 py-5">
          <div className="text-xs font-medium uppercase tracking-wide text-blue-600">Default Space</div>
          <h1 className="mt-1 text-2xl font-semibold text-gray-950">Learning Space</h1>
          <p className="mt-2 text-sm leading-6 text-gray-500">
            Organize notes, quiz records, and learner memory in one workspace.
          </p>
        </div>

        <nav className="space-y-1 px-3">
          {tabs.map((tab) => {
            const Icon = tab.icon
            const active = activeTab === tab.key
            return (
              <button
                key={tab.key}
                className={`flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left text-sm font-medium ${
                  active ? 'bg-white text-blue-700 shadow-sm ring-1 ring-blue-100' : 'text-gray-700 hover:bg-white hover:text-gray-950'
                }`}
                type="button"
                onClick={() => setActiveTab(tab.key)}
              >
                <Icon size={18} />
                <span>{tab.label}</span>
              </button>
            )
          })}
        </nav>

        <div className="mt-auto border-t border-gray-200 px-5 py-4 text-xs text-gray-500">
          Local-first space. Multi-space management can come later.
        </div>
      </aside>

      <section className="flex min-w-0 flex-1 flex-col">
        <header className="flex items-center gap-4 border-b border-gray-100 px-8 py-5">
          <div>
            <h2 className="text-xl font-semibold text-gray-950">{tabs.find((tab) => tab.key === activeTab)?.label}</h2>
            <p className="mt-1 text-sm text-gray-500">{subtitleFor(activeTab)}</p>
          </div>
          <button
            className="ml-auto inline-flex h-9 items-center gap-2 rounded-lg border border-gray-200 px-3 text-sm font-medium text-gray-700 hover:bg-blue-50 hover:text-blue-700 disabled:opacity-50"
            type="button"
            disabled={loading}
            onClick={refreshActiveTab}
          >
            <RefreshCw size={16} className={loading ? 'animate-spin' : ''} />
            Refresh
          </button>
        </header>

        {activeTab === 'notebook' && (
          <NotebookTab
            entries={notebookEntries}
            activeEntry={activeNotebookEntry}
            draftTitle={draftTitle}
            draftMarkdown={draftMarkdown}
            status={status}
            loading={loading}
            editingEntryId={editingNotebookId}
            editTitle={editTitle}
            editMarkdown={editMarkdown}
            onSelectEntry={setActiveNotebookId}
            onDraftTitleChange={setDraftTitle}
            onDraftMarkdownChange={setDraftMarkdown}
            onCreateEntry={() => void createNotebookEntry()}
            onDeleteEntry={(entry) => void deleteNotebookEntry(entry)}
            onStartEdit={startEditNotebookEntry}
            onCancelEdit={cancelEditNotebookEntry}
            onEditTitleChange={setEditTitle}
            onEditMarkdownChange={setEditMarkdown}
            onSaveEntry={(entry) => void saveNotebookEntry(entry)}
            onSendToBook={(entry) => void sendNotebookEntryToBook(entry)}
            onCreateLinkedEntry={(title) => void createNotebookEntryFromLink(title)}
            onSourceNavigate={onSourceNavigate}
          />
        )}
        {activeTab === 'quiz_bank' && (
          <QuizBankTab
            quizzes={quizzes}
            filteredQuizzes={filteredQuizzes}
            sourceFilter={quizSourceFilter}
            onSourceFilterChange={setQuizSourceFilter}
            activeQuiz={activeQuiz}
            activeQuestion={activeQuestion}
            questionIndex={questionIndex}
            status={status}
            onSelectQuiz={setActiveQuizId}
            onDeleteQuiz={(quiz) => void deleteQuiz(quiz)}
            onQuestionIndexChange={setQuestionIndex}
            onSourceNavigate={onSourceNavigate}
          />
        )}
        {activeTab === 'student_profile' && (
          <StudentProfileTab
            profile={profile}
            memoryFiles={memoryFiles}
            editingMemoryPath={editingMemoryPath}
            memoryDraft={memoryDraft}
            loading={loading}
            onStartEditMemory={startEditMemory}
            onCancelEditMemory={cancelEditMemory}
            onMemoryDraftChange={setMemoryDraft}
            onSaveMemoryFile={(path) => void saveMemoryFile(path)}
            onSourceNavigate={onSourceNavigate}
          />
        )}
      </section>
    </main>
  )
}

function NotebookTab({
  entries,
  activeEntry,
  draftTitle,
  draftMarkdown,
  status,
  loading,
  editingEntryId,
  editTitle,
  editMarkdown,
  onSelectEntry,
  onDraftTitleChange,
  onDraftMarkdownChange,
  onCreateEntry,
  onDeleteEntry,
  onStartEdit,
  onCancelEdit,
  onEditTitleChange,
  onEditMarkdownChange,
  onSaveEntry,
  onSendToBook,
  onCreateLinkedEntry,
  onSourceNavigate,
}: {
  entries: NotebookEntry[]
  activeEntry: NotebookEntry | null
  draftTitle: string
  draftMarkdown: string
  status: string
  loading: boolean
  editingEntryId: string | null
  editTitle: string
  editMarkdown: string
  onSelectEntry: (id: string) => void
  onDraftTitleChange: (value: string) => void
  onDraftMarkdownChange: (value: string) => void
  onCreateEntry: () => void
  onDeleteEntry: (entry: NotebookEntry) => void
  onStartEdit: (entry: NotebookEntry) => void
  onCancelEdit: () => void
  onEditTitleChange: (value: string) => void
  onEditMarkdownChange: (value: string) => void
  onSaveEntry: (entry: NotebookEntry) => void
  onSendToBook: (entry: NotebookEntry) => void
  onCreateLinkedEntry: (title: string) => void
  onSourceNavigate?: (target: SourceTarget, reference: SourceReference) => void
}) {
  const isEditing = activeEntry ? editingEntryId === activeEntry.id : false

  return (
    <div className="flex min-h-0 flex-1">
      <aside className="flex w-80 shrink-0 flex-col border-r border-gray-100 bg-gray-50/70">
        <div className="space-y-3 border-b border-gray-100 p-4">
          <input
            className={inputClassName}
            value={draftTitle}
            onChange={(event) => onDraftTitleChange(event.target.value)}
            placeholder="Note title"
          />
          <textarea
            className={`${inputClassName} min-h-28 resize-none leading-6`}
            value={draftMarkdown}
            onChange={(event) => onDraftMarkdownChange(event.target.value)}
            placeholder="Write Markdown..."
          />
          <button
            className="inline-flex h-9 w-full items-center justify-center gap-2 rounded-lg bg-blue-600 px-3 text-sm font-medium text-white hover:bg-blue-700 disabled:bg-gray-200 disabled:text-gray-400"
            type="button"
            disabled={!draftMarkdown.trim()}
            onClick={onCreateEntry}
          >
            <Plus size={16} />
            New note
          </button>
          <div className="text-xs text-gray-500">{status}</div>
        </div>

        <div className="flex-1 overflow-y-auto px-3 py-4">
          {entries.length === 0 ? (
            <div className="rounded-lg px-3 py-8 text-center text-sm text-gray-400">No notebook entries yet</div>
          ) : (
            <div className="space-y-2">
              {entries.map((entry) => (
                <button
                  key={entry.id}
                  className={`group flex w-full items-start gap-3 rounded-lg p-3 text-left text-sm ${
                    activeEntry?.id === entry.id ? 'bg-white shadow-sm ring-1 ring-blue-100' : 'hover:bg-white'
                  }`}
                  type="button"
                  onClick={() => onSelectEntry(entry.id)}
                >
                  <FileText size={17} className="mt-0.5 shrink-0 text-blue-600" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate font-medium text-gray-900">{entry.title}</span>
                    <span className="mt-0.5 block text-xs text-gray-500">{entry.entry_type.replaceAll('_', ' ')}</span>
                    {entry.tags && entry.tags.length > 0 && (
                      <span className="mt-2 flex flex-wrap gap-1">
                        {entry.tags.slice(0, 3).map((tag) => (
                          <span key={tag} className="rounded-full bg-blue-50 px-2 py-0.5 text-[11px] text-blue-700">
                            #{tag}
                          </span>
                        ))}
                      </span>
                    )}
                  </span>
                  <span
                    role="button"
                    tabIndex={0}
                    className="rounded p-1 text-gray-400 opacity-0 hover:bg-red-50 hover:text-red-600 group-hover:opacity-100"
                    onClick={(event) => {
                      event.stopPropagation()
                      onDeleteEntry(entry)
                    }}
                  >
                    <Trash2 size={15} />
                  </span>
                </button>
              ))}
            </div>
          )}
        </div>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        {!activeEntry ? (
          <EmptyDetail
            icon={BookMarked}
            title="Create or select a note"
            description="Notebook stores research reports, useful chat answers, source snippets, quiz summaries, and solve results as Markdown entries."
          />
        ) : (
          <>
            <div className="flex items-start gap-4 border-b border-gray-100 px-8 py-5">
              <div className="min-w-0 flex-1">
                <div className="text-xs font-medium uppercase tracking-wide text-blue-600">{activeEntry.entry_type.replaceAll('_', ' ')}</div>
                {isEditing ? (
                  <input
                    className={`${inputClassName} mt-2 max-w-2xl text-base font-semibold`}
                    value={editTitle}
                    onChange={(event) => onEditTitleChange(event.target.value)}
                  />
                ) : (
                  <h3 className="mt-1 truncate text-xl font-semibold text-gray-950">{activeEntry.title}</h3>
                )}
              </div>
              {isEditing ? (
                <div className="flex shrink-0 items-center gap-2">
                  <button
                    className={secondaryButtonClassName}
                    type="button"
                    disabled={loading || !editMarkdown.trim()}
                    onClick={() => onSaveEntry(activeEntry)}
                  >
                    <Save size={16} />
                    Save
                  </button>
                  <button className={secondaryButtonClassName} type="button" disabled={loading} onClick={onCancelEdit}>
                    <X size={16} />
                    Cancel
                  </button>
                </div>
              ) : (
                <div className="flex shrink-0 items-center gap-2">
                  <button className={secondaryButtonClassName} type="button" disabled={loading} onClick={() => onSendToBook(activeEntry)}>
                    <Send size={16} />
                    Send to Book
                  </button>
                  <button className={secondaryButtonClassName} type="button" disabled={loading} onClick={() => onStartEdit(activeEntry)}>
                    <Edit3 size={16} />
                    Edit
                  </button>
                </div>
              )}
            </div>
            <div className="flex min-h-0 flex-1 overflow-hidden">
              <div className="min-w-0 flex-1 overflow-y-auto px-8 py-6">
              {isEditing ? (
                <textarea
                  className={`${inputClassName} min-h-[520px] max-w-4xl resize-y font-mono leading-6`}
                  value={editMarkdown}
                  onChange={(event) => onEditMarkdownChange(event.target.value)}
                />
              ) : (
                <div className="max-w-4xl rounded-lg border border-gray-200 bg-gray-50 p-5">
                  <MarkdownMessage text={activeEntry.markdown || ' '} onSourceNavigate={onSourceNavigate} />
                </div>
              )}
              </div>
              {!isEditing && (
                <NotebookRelationsPanel
                  entry={activeEntry}
                  onSelectEntry={onSelectEntry}
                  onCreateLinkedEntry={onCreateLinkedEntry}
                />
              )}
            </div>
          </>
        )}
      </div>
    </div>
  )
}

function NotebookRelationsPanel({
  entry,
  onSelectEntry,
  onCreateLinkedEntry,
}: {
  entry: NotebookEntry
  onSelectEntry: (id: string) => void
  onCreateLinkedEntry: (title: string) => void
}) {
  const tags = entry.tags ?? []
  const links = entry.links ?? []
  const backlinks = entry.backlinks ?? []

  return (
    <aside className="hidden w-80 shrink-0 overflow-y-auto border-l border-gray-100 bg-white px-4 py-5 xl:block">
      <div className="space-y-6">
        <section>
          <div className="mb-2 flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-gray-500">
            <Tags size={14} />
            Tags
          </div>
          {tags.length === 0 ? (
            <div className="text-sm text-gray-400">No tags yet</div>
          ) : (
            <div className="flex flex-wrap gap-2">
              {tags.map((tag) => (
                <span key={tag} className="rounded-full bg-blue-50 px-2.5 py-1 text-xs font-medium text-blue-700">
                  #{tag}
                </span>
              ))}
            </div>
          )}
        </section>

        <section>
          <div className="mb-2 flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-gray-500">
            <Link2 size={14} />
            Links
          </div>
          {links.length === 0 ? (
            <div className="text-sm text-gray-400">No outgoing links</div>
          ) : (
            <div className="space-y-2">
              {links.map((link) => (
                <button
                  key={`${link.raw}-${link.target_id ?? link.target}`}
                  className={`w-full rounded-lg border px-3 py-2 text-left text-sm transition ${
                    link.resolved
                      ? 'border-blue-100 bg-blue-50/60 text-blue-800 hover:bg-blue-100'
                      : 'border-dashed border-amber-200 bg-amber-50/70 text-amber-700 hover:bg-amber-100'
                  }`}
                  type="button"
                  onClick={() => {
                    if (link.target_id) {
                      onSelectEntry(link.target_id)
                    } else {
                      onCreateLinkedEntry(link.target)
                    }
                  }}
                >
                  <span className="block truncate font-medium">
                    {link.alias || link.target_title || link.target}
                  </span>
                  <span className="mt-0.5 block truncate text-xs opacity-75">
                    {link.resolved ? 'resolved note' : 'create note'}
                  </span>
                </button>
              ))}
            </div>
          )}
        </section>

        <section>
          <div className="mb-2 flex items-center gap-2 text-xs font-semibold uppercase tracking-wide text-gray-500">
            <NotebookPen size={14} />
            Backlinks
          </div>
          {backlinks.length === 0 ? (
            <div className="text-sm text-gray-400">No backlinks yet</div>
          ) : (
            <div className="space-y-2">
              {backlinks.map((backlink) => (
                <button
                  key={`${backlink.source_entry_id}-${backlink.raw}`}
                  className="w-full rounded-lg border border-gray-100 bg-gray-50 px-3 py-2 text-left text-sm text-gray-700 transition hover:border-blue-100 hover:bg-blue-50"
                  type="button"
                  onClick={() => onSelectEntry(backlink.source_entry_id)}
                >
                  <span className="block truncate font-medium text-gray-900">{backlink.source_title}</span>
                  <span className="mt-1 line-clamp-2 text-xs leading-5 text-gray-500">{backlink.snippet}</span>
                </button>
              ))}
            </div>
          )}
        </section>
      </div>
    </aside>
  )
}

function QuizBankTab({
  quizzes,
  filteredQuizzes,
  sourceFilter,
  activeQuiz,
  activeQuestion,
  questionIndex,
  status,
  onSelectQuiz,
  onSourceFilterChange,
  onDeleteQuiz,
  onQuestionIndexChange,
  onSourceNavigate,
}: {
  quizzes: QuizSession[]
  filteredQuizzes: QuizSession[]
  sourceFilter: QuizSourceFilter
  activeQuiz: QuizSession | null
  activeQuestion: QuizQuestion | null
  questionIndex: number
  status: string
  onSelectQuiz: (id: string) => void
  onSourceFilterChange: (filter: QuizSourceFilter) => void
  onDeleteQuiz: (quiz: QuizSession) => void
  onQuestionIndexChange: (index: number) => void
  onSourceNavigate?: (target: SourceTarget, reference: SourceReference) => void
}) {
  const activeAnswer = activeQuiz && activeQuestion
    ? activeQuiz.answers.find((answer) => answer.question_id === activeQuestion.id) ?? null
    : null
  const missedQuestions = useMemo(() => {
    if (!activeQuiz) return []
    return activeQuiz.questions.filter((question) => {
      const answer = activeQuiz.answers.find((item) => item.question_id === question.id)
      return answer && !answer.correct
    })
  }, [activeQuiz])

  return (
    <div className="flex min-h-0 flex-1">
      <aside className="flex w-80 shrink-0 flex-col border-r border-gray-100 bg-gray-50/70">
        <div className="px-4 py-3 text-xs text-gray-500">{status}</div>
        <div className="border-b border-gray-100 px-3 pb-3">
          <div className="flex flex-wrap gap-2">
            {quizSourceFilters.map((filter) => (
              <button
                key={filter.key}
                className={`rounded-full px-3 py-1.5 text-xs font-medium transition ${
                  sourceFilter === filter.key
                    ? 'bg-blue-600 text-white'
                    : 'bg-white text-gray-600 ring-1 ring-gray-200 hover:bg-blue-50 hover:text-blue-700'
                }`}
                type="button"
                onClick={() => onSourceFilterChange(filter.key)}
              >
                {filter.label}
              </button>
            ))}
          </div>
        </div>
        <div className="flex-1 overflow-y-auto px-3 pb-4">
          {quizzes.length === 0 ? (
            <div className="rounded-lg px-3 py-8 text-center text-sm text-gray-400">No quiz records yet</div>
          ) : filteredQuizzes.length === 0 ? (
            <div className="rounded-lg px-3 py-8 text-center text-sm text-gray-400">No quizzes match this filter</div>
          ) : (
            <div className="space-y-2">
              {filteredQuizzes.map((quiz) => {
                const active = activeQuiz?.id === quiz.id
                const score = quiz.score ?? { correct: 0, total: quiz.questions.length }
                const answeredCount = quiz.answers.length
                const source = quizSourceType(quiz)
                return (
                  <button
                    key={quiz.id}
                    className={`group flex w-full items-start gap-3 rounded-lg p-3 text-left text-sm ${
                      active ? 'bg-white shadow-sm ring-1 ring-blue-100' : 'hover:bg-white'
                    }`}
                    type="button"
                    onClick={() => onSelectQuiz(quiz.id)}
                  >
                    <FileQuestion size={17} className="mt-0.5 shrink-0 text-blue-600" />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate font-medium text-gray-900">{quiz.title}</span>
                      <span className="mt-0.5 block text-xs text-gray-500">
                        {score.correct}/{score.total} correct · answered {answeredCount}/{quiz.questions.length} · {quiz.status}
                      </span>
                      <span className="mt-1 flex items-center gap-2 text-xs text-gray-400">
                        <span className="rounded-full bg-gray-100 px-2 py-0.5 text-gray-600">{quizSourceLabel(source)}</span>
                        Created {formatTime(quiz.created_at)} · Updated {formatTime(quiz.updated_at)}
                      </span>
                    </span>
                    <span
                      role="button"
                      tabIndex={0}
                      className="rounded p-1 text-gray-400 opacity-0 hover:bg-red-50 hover:text-red-600 group-hover:opacity-100"
                      onClick={(event) => {
                        event.stopPropagation()
                        onDeleteQuiz(quiz)
                      }}
                    >
                      <Trash2 size={15} />
                    </span>
                  </button>
                )
              })}
            </div>
          )}
        </div>
      </aside>

      <div className="flex min-w-0 flex-1 flex-col">
        {!activeQuiz || !activeQuestion ? (
          <EmptyDetail
            icon={FileQuestion}
            title="Select a quiz record"
            description="Quiz Bank is for review and re-practice. Generate new quizzes from the chat composer."
          />
        ) : (
          <>
            <div className="flex items-center border-b border-gray-100 px-8 py-5">
              <div>
                <h3 className="text-xl font-semibold text-gray-950">{activeQuiz.title}</h3>
                <p className="mt-1 text-sm text-gray-500">
                  Question {questionIndex + 1} of {activeQuiz.questions.length} · {difficultyLabel(activeQuestion.difficulty)}
                </p>
              </div>
              <ScorePill quiz={activeQuiz} />
            </div>

            <div className="flex-1 overflow-y-auto px-8 py-6">
              <div className="max-w-4xl">
                <div className="mb-4 flex flex-wrap gap-2">
                  {activeQuestion.tags.map((tag) => (
                    <span key={tag} className="rounded-full bg-gray-100 px-2.5 py-1 text-xs font-medium text-gray-600">{tag}</span>
                  ))}
                </div>
                <h4 className="text-2xl font-semibold leading-9 text-gray-950">{activeQuestion.stem}</h4>

                <div className="mt-6 space-y-3">
                  {activeQuestion.options.map((option) => {
                    const selected = activeAnswer?.selected_option_id === option.id
                    const correct = activeQuestion.correct_option_id === option.id
                    return (
                      <div
                        key={option.id}
                        className={`flex items-start gap-3 rounded-lg border p-4 ${
                          correct
                            ? 'border-emerald-300 bg-emerald-50'
                            : selected
                              ? 'border-red-200 bg-red-50'
                              : 'border-gray-200 bg-white'
                        }`}
                      >
                        <span className={correct ? 'text-emerald-700' : selected ? 'text-red-600' : 'text-gray-400'}>
                          <CheckCircle2 size={19} />
                        </span>
                        <span>
                          <span className="font-medium text-gray-950">{option.id}.</span>{' '}
                          <span className="text-gray-700">{option.text}</span>
                        </span>
                      </div>
                    )
                  })}
                </div>

                <section className="mt-6 rounded-lg border border-gray-200 bg-gray-50 p-4">
                  <div className="text-sm font-semibold text-gray-950">Explanation</div>
                  <p className="mt-2 text-sm leading-6 text-gray-600">{activeQuestion.explanation}</p>
                  {activeQuestion.citations.length > 0 && (
                    <QuizSourceReferences
                      quizId={activeQuiz.id}
                      questionId={activeQuestion.id}
                      citations={activeQuestion.citations}
                      onSourceNavigate={onSourceNavigate}
                    />
                  )}
                </section>

                {missedQuestions.length > 0 && (
                  <section className="mt-6 rounded-lg border border-blue-100 bg-blue-50/50 p-4">
                    <div className="text-sm font-semibold text-gray-950">Missed-question review</div>
                    <div className="mt-3 flex flex-wrap gap-2">
                      {missedQuestions.map((question) => {
                        const index = activeQuiz.questions.findIndex((item) => item.id === question.id)
                        return (
                          <button
                            key={question.id}
                            className="rounded-full bg-white px-3 py-1.5 text-xs font-medium text-blue-700 ring-1 ring-blue-100 hover:bg-blue-50"
                            type="button"
                            onClick={() => {
                              if (index >= 0) onQuestionIndexChange(index)
                            }}
                          >
                            Q{index + 1}
                          </button>
                        )
                      })}
                    </div>
                  </section>
                )}
              </div>
            </div>

            <footer className="flex items-center gap-3 border-t border-gray-100 px-8 py-4">
              <button className={secondaryButtonClassName} type="button" disabled={questionIndex === 0} onClick={() => onQuestionIndexChange(Math.max(0, questionIndex - 1))}>
                <ChevronLeft size={16} />
                Previous
              </button>
              <button className={secondaryButtonClassName} type="button" disabled={questionIndex >= activeQuiz.questions.length - 1} onClick={() => onQuestionIndexChange(Math.min(activeQuiz.questions.length - 1, questionIndex + 1))}>
                Next
                <ChevronRight size={16} />
              </button>
            </footer>
          </>
        )}
      </div>
    </div>
  )
}

function QuizSourceReferences({
  quizId,
  questionId,
  citations,
  onSourceNavigate,
}: {
  quizId: string
  questionId: string
  citations: QuizQuestion['citations']
  onSourceNavigate?: (target: SourceTarget, reference: SourceReference) => void
}) {
  return (
    <SourceReferences
      id={`space-quiz-citations-${quizId}-${questionId}`}
      references={citations.map((citation, index) => quizCitationToSourceReference(citation, index))}
      onNavigate={onSourceNavigate}
    />
  )
}

function quizCitationToSourceReference(citation: QuizQuestion['citations'][number], index: number): SourceReference {
  const raw = quizCitationRawTarget(citation)
  const target = sourceTargetFromRaw(raw)
  return {
    id: `${index + 1}:${raw}`,
    label: String(index + 1),
    raw,
    surface: target?.type === 'web' ? 'web' : target?.type === 'kb' ? 'kb' : 'unknown',
    title: citation.title || citation.source,
    description: citation.text,
    score: citation.score,
    metadata: {
      documentName: citation.title || citation.source,
      documentId: citation.document_id ?? undefined,
      chunkId: citation.chunk_id ?? undefined,
      missingReason: target ? undefined : 'This quiz citation was generated before source navigation metadata was available.',
    },
    target,
  }
}

function quizCitationRawTarget(citation: QuizQuestion['citations'][number]) {
  if (citation.kb && citation.document_id) {
    return ['kb', citation.kb, citation.document_id, citation.chunk_id].filter(Boolean).join(':')
  }
  return citation.source
}

function quizSourceType(quiz: QuizSession): Exclude<QuizSourceFilter, 'all'> {
  if (quiz.kb_id?.trim()) return 'knowledge_base'
  const sources = quiz.questions.flatMap((question) => question.citations.map((citation) => citation.source.toLowerCase()))
  const sourceText = [quiz.title, quiz.config.topic ?? '', ...sources].join(' ').toLowerCase()
  if (sourceText.includes('space reference') || sourceText.includes('mentioned_space_items')) return 'space'
  if (sourceText.includes('notebook:') || sourceText.includes('notebook')) return 'notebook'
  return 'conversation'
}

function quizSourceLabel(source: Exclude<QuizSourceFilter, 'all'>) {
  if (source === 'knowledge_base') return 'Knowledge'
  if (source === 'space') return 'Space'
  if (source === 'notebook') return 'Notebook'
  return 'Conversation'
}

function StudentProfileTab({
  profile,
  memoryFiles,
  editingMemoryPath,
  memoryDraft,
  loading,
  onStartEditMemory,
  onCancelEditMemory,
  onMemoryDraftChange,
  onSaveMemoryFile,
  onSourceNavigate,
}: {
  profile: ReturnType<typeof buildProfile>
  memoryFiles: MemoryFile[]
  editingMemoryPath: string | null
  memoryDraft: string
  loading: boolean
  onStartEditMemory: (file: MemoryFile) => void
  onCancelEditMemory: () => void
  onMemoryDraftChange: (value: string) => void
  onSaveMemoryFile: (path: string) => void
  onSourceNavigate?: (target: SourceTarget, reference: SourceReference) => void
}) {
  const filesByPath = new Map(memoryFiles.map((file) => [file.path, file]))

  return (
    <div className="flex-1 overflow-y-auto px-8 py-6">
      <div className="grid gap-4 md:grid-cols-3">
        <Metric label="Quiz records" value={String(profile.quizCount)} />
        <Metric label="Answered" value={String(profile.answeredCount)} />
        <Metric label="Accuracy" value={profile.accuracyLabel} />
      </div>

      <div className="mt-6 grid gap-5 lg:grid-cols-2">
        <section className="rounded-lg border border-gray-200 bg-white p-5">
          <div className="flex items-center gap-2 text-sm font-semibold text-gray-950">
            <Target size={18} className="text-blue-600" />
            Current weak signals
          </div>
          {profile.weakTags.length === 0 ? (
            <p className="mt-3 text-sm leading-6 text-gray-500">No wrong-answer pattern yet. Finish quizzes to build this profile.</p>
          ) : (
            <div className="mt-4 flex flex-wrap gap-2">
              {profile.weakTags.map(([tag, count]) => (
                <span key={tag} className="rounded-full bg-blue-50 px-3 py-1.5 text-sm font-medium text-blue-700">
                  {tag} x {count}
                </span>
              ))}
            </div>
          )}
        </section>

        <section className="rounded-lg border border-gray-200 bg-white p-5">
          <div className="text-sm font-semibold text-gray-950">Profile source</div>
          <p className="mt-3 text-sm leading-6 text-gray-500">
            Student Profile is rendered from visible Markdown memory plus quiz stats. Edit the Markdown below to correct the profile.
          </p>
        </section>
      </div>

      <div className="mt-6 grid gap-5 xl:grid-cols-3">
        {profileMemoryPaths.map((path) => {
          const file = filesByPath.get(path)
          return (
            <MemoryProfileCard
              key={path}
              path={path}
              file={file}
              editing={editingMemoryPath === path}
              draft={memoryDraft}
              loading={loading}
              onStartEdit={onStartEditMemory}
              onCancel={onCancelEditMemory}
              onDraftChange={onMemoryDraftChange}
              onSave={onSaveMemoryFile}
              onSourceNavigate={onSourceNavigate}
            />
          )
        })}
      </div>
    </div>
  )
}

function MemoryProfileCard({
  path,
  file,
  editing,
  draft,
  loading,
  onStartEdit,
  onCancel,
  onDraftChange,
  onSave,
  onSourceNavigate,
}: {
  path: string
  file?: MemoryFile
  editing: boolean
  draft: string
  loading: boolean
  onStartEdit: (file: MemoryFile) => void
  onCancel: () => void
  onDraftChange: (value: string) => void
  onSave: (path: string) => void
  onSourceNavigate?: (target: SourceTarget, reference: SourceReference) => void
}) {
  const title = memoryFileLabel(path)
  return (
    <section className="flex min-h-[360px] flex-col rounded-lg border border-gray-200 bg-white">
      <div className="flex items-center gap-3 border-b border-gray-100 px-4 py-3">
        <div className="min-w-0">
          <div className="truncate text-sm font-semibold text-gray-950">{title}</div>
          <div className="truncate text-xs text-gray-500">{path}</div>
        </div>
        {file && !editing && (
          <button className={`${secondaryButtonClassName} ml-auto`} type="button" disabled={loading} onClick={() => onStartEdit(file)}>
            <Edit3 size={16} />
            Edit
          </button>
        )}
        {editing && (
          <div className="ml-auto flex items-center gap-2">
            <button className={secondaryButtonClassName} type="button" disabled={loading || !draft.trim()} onClick={() => onSave(path)}>
              <Save size={16} />
              Save
            </button>
            <button className={secondaryButtonClassName} type="button" disabled={loading} onClick={onCancel}>
              <X size={16} />
              Cancel
            </button>
          </div>
        )}
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto p-4">
        {!file ? (
          <div className="text-sm text-gray-400">Memory file has not loaded yet.</div>
        ) : editing ? (
          <textarea
            className={`${inputClassName} min-h-[260px] resize-y font-mono leading-6`}
            value={draft}
            onChange={(event) => onDraftChange(event.target.value)}
          />
        ) : (
          <div className="space-y-4">
            <div className="prose-sm max-w-none">
              <MarkdownMessage text={file.markdown || ' '} onSourceNavigate={onSourceNavigate} />
            </div>
          </div>
        )}
      </div>
    </section>
  )
}

function Metric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-lg border border-gray-200 bg-white p-5">
      <div className="text-xs font-medium uppercase tracking-wide text-gray-500">{label}</div>
      <div className="mt-2 text-2xl font-semibold text-gray-950">{value}</div>
    </div>
  )
}

function ScorePill({ quiz }: { quiz: QuizSession }) {
  const score = quiz.score ?? { correct: 0, total: quiz.questions.length }
  return (
    <div className="ml-auto rounded-lg bg-blue-50 px-3 py-2 text-sm font-medium text-blue-700">
      Score {score.correct}/{score.total}
    </div>
  )
}

function EmptyDetail({ icon: Icon, title, description }: { icon: typeof FileQuestion; title: string; description: string }) {
  return (
    <div className="flex flex-1 items-center justify-center px-6">
      <div className="max-w-md text-center">
        <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-2xl bg-blue-50 text-blue-700">
          <Icon size={28} />
        </div>
        <h3 className="mt-5 text-2xl font-semibold text-gray-950">{title}</h3>
        <p className="mt-2 text-sm leading-6 text-gray-500">{description}</p>
      </div>
    </div>
  )
}

function buildProfile(quizzes: QuizSession[]) {
  let answeredCount = 0
  let correctCount = 0
  const weakTags = new Map<string, number>()

  quizzes.forEach((quiz) => {
    quiz.answers.forEach((answer) => {
      answeredCount += 1
      if (answer.correct) correctCount += 1
      if (!answer.correct) {
        const question = quiz.questions.find((item) => item.id === answer.question_id)
        question?.tags.forEach((tag) => weakTags.set(tag, (weakTags.get(tag) ?? 0) + 1))
      }
    })
  })

  const accuracy = answeredCount ? Math.round((correctCount / answeredCount) * 100) : null
  return {
    quizCount: quizzes.length,
    answeredCount,
    accuracyLabel: accuracy == null ? 'No data' : `${accuracy}%`,
    weakTags: [...weakTags.entries()].sort((a, b) => b[1] - a[1]).slice(0, 8),
  }
}

function memoryFileLabel(path: string) {
  const labels: Record<string, string> = {
    'L3/profile.md': 'Student profile',
    'L3/recent.md': 'Recent context',
    'L3/teaching_strategy.md': 'Teaching strategy',
  }
  return labels[path] ?? path
}

function subtitleFor(tab: SpaceTab) {
  if (tab === 'notebook') return 'Saved reports, notes, snippets, and reusable learning records.'
  if (tab === 'quiz_bank') return 'Review historical quizzes and missed questions.'
  return 'A visible learner profile built from Markdown memory and practice data.'
}

async function safeJson(res: Response): Promise<Record<string, unknown>> {
  try {
    return await res.json()
  } catch {
    return {}
  }
}

function errorMessage(data: Record<string, unknown>, status: number) {
  return typeof data.error === 'string' ? data.error : `HTTP ${status}`
}

function difficultyLabel(value: string) {
  return value.charAt(0).toUpperCase() + value.slice(1)
}

function formatTime(value: string) {
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return date.toLocaleString()
}

const secondaryButtonClassName = 'inline-flex h-9 items-center justify-center gap-1.5 rounded-lg border border-gray-200 px-3.5 text-sm font-medium text-gray-700 hover:bg-blue-50 hover:text-blue-700 disabled:opacity-50'
const inputClassName = 'w-full rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm text-gray-900 outline-none focus:border-blue-300 focus:ring-2 focus:ring-blue-50'
