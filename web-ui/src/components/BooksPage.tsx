import { useEffect, useState } from 'react'
import { BookOpen, FileText, RefreshCw } from 'lucide-react'
import { MarkdownMessage } from './MarkdownMessage'

interface Book {
  id: string
  title: string
  description?: string | null
  chapters: BookChapter[]
  updated_at: string
}

interface BookChapter {
  id: string
  title: string
  markdown: string
  source_session_id?: string | null
  updated_at: string
}

export function BooksPage() {
  const [books, setBooks] = useState<Book[]>([])
  const [activeBookId, setActiveBookId] = useState<string | null>(null)
  const [activeChapterId, setActiveChapterId] = useState<string | null>(null)
  const [status, setStatus] = useState('Loading books...')

  const activeBook = books.find((book) => book.id === activeBookId) ?? books[0] ?? null
  const activeChapter =
    activeBook?.chapters.find((chapter) => chapter.id === activeChapterId) ??
    activeBook?.chapters[0] ??
    null

  const loadBooks = async () => {
    setStatus('Loading books...')
    try {
      const res = await fetch('/api/books')
      const data = await res.json().catch(() => ({})) as { books?: Book[]; error?: string }
      if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`)
      const items = data.books ?? []
      setBooks(items)
      setActiveBookId((current) => current && items.some((book) => book.id === current) ? current : items[0]?.id ?? null)
      setStatus(items.length ? 'Books loaded' : 'No books yet')
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      setStatus(`Load failed: ${message}`)
    }
  }

  useEffect(() => {
    void loadBooks()
  }, [])

  useEffect(() => {
    if (!activeBook) {
      setActiveChapterId(null)
      return
    }
    setActiveChapterId((current) =>
      current && activeBook.chapters.some((chapter) => chapter.id === current)
        ? current
        : activeBook.chapters[0]?.id ?? null,
    )
  }, [activeBook?.id, activeBook?.chapters.length])

  return (
    <div className="flex h-full min-h-0 bg-gray-50">
      <aside className="flex w-80 shrink-0 flex-col border-r border-gray-100 bg-white">
        <div className="flex items-center gap-3 border-b border-gray-100 px-5 py-4">
          <BookOpen size={22} className="text-blue-600" />
          <div className="min-w-0">
            <h2 className="text-lg font-semibold text-gray-950">书籍</h2>
            <p className="truncate text-xs text-gray-500">{status}</p>
          </div>
          <button
            className="ml-auto flex h-8 w-8 items-center justify-center rounded-lg text-gray-500 hover:bg-blue-50 hover:text-blue-700"
            type="button"
            onClick={() => void loadBooks()}
            title="刷新"
          >
            <RefreshCw size={16} />
          </button>
        </div>
        <div className="min-h-0 flex-1 overflow-y-auto p-3">
          {books.length === 0 ? (
            <div className="rounded-lg px-3 py-10 text-center text-sm text-gray-400">
              研究报告保存到书籍后会出现在这里。
            </div>
          ) : (
            books.map((book) => (
              <button
                key={book.id}
                className={`mb-2 w-full rounded-lg px-3 py-3 text-left transition ${
                  activeBook?.id === book.id ? 'bg-blue-50 text-blue-800' : 'hover:bg-gray-50'
                }`}
                type="button"
                onClick={() => setActiveBookId(book.id)}
              >
                <span className="block truncate text-sm font-semibold">{book.title}</span>
                <span className="mt-1 block text-xs text-gray-500">{book.chapters.length} chapters</span>
              </button>
            ))
          )}
        </div>
      </aside>

      {activeBook && (
        <aside className="flex w-80 shrink-0 flex-col border-r border-gray-100 bg-white/80">
          <div className="border-b border-gray-100 px-5 py-4">
            <h3 className="truncate text-base font-semibold text-gray-950">{activeBook.title}</h3>
            <p className="mt-1 text-xs text-gray-500">章节</p>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto p-3">
            {activeBook.chapters.length === 0 ? (
              <div className="rounded-lg px-3 py-8 text-center text-sm text-gray-400">暂无章节</div>
            ) : (
              activeBook.chapters.map((chapter) => (
                <button
                  key={chapter.id}
                  className={`mb-2 flex w-full items-start gap-2 rounded-lg px-3 py-3 text-left transition ${
                    activeChapter?.id === chapter.id ? 'bg-white shadow-sm ring-1 ring-blue-100' : 'hover:bg-white'
                  }`}
                  type="button"
                  onClick={() => setActiveChapterId(chapter.id)}
                >
                  <FileText size={16} className="mt-0.5 shrink-0 text-blue-600" />
                  <span className="min-w-0">
                    <span className="block truncate text-sm font-medium text-gray-900">{chapter.title}</span>
                    <span className="mt-1 block truncate text-xs text-gray-500">
                      {chapter.source_session_id ? '来自研究会话' : '手动章节'}
                    </span>
                  </span>
                </button>
              ))
            )}
          </div>
        </aside>
      )}

      <main className="min-w-0 flex-1 overflow-y-auto bg-white p-8">
        {activeChapter ? (
          <article className="mx-auto max-w-4xl">
            <h1 className="mb-6 text-2xl font-semibold text-gray-950">{activeChapter.title}</h1>
            <div className="rounded-lg bg-gray-50 p-5">
              <MarkdownMessage text={activeChapter.markdown} />
            </div>
          </article>
        ) : (
          <div className="flex h-full items-center justify-center text-sm text-gray-400">
            选择一个章节查看内容。
          </div>
        )}
      </main>
    </div>
  )
}
