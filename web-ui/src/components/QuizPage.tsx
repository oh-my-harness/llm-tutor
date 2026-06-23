import { useCallback, useEffect, useMemo, useState } from 'react'
import { BookOpenCheck, CheckCircle2, Circle, FileQuestion, Play, RefreshCw, Trash2 } from 'lucide-react'
import type { LlmSettings } from '../settings'
import { settingsForSession } from '../settings'

interface KnowledgeBaseOption {
  id: string
  name: string
}

interface QuizSession {
  id: string
  title: string
  kb_id: string
  status: 'draft' | 'generating' | 'active' | 'finished' | 'error'
  config: {
    topic?: string | null
    difficulty: 'easy' | 'medium' | 'hard'
    question_count: number
    question_type: 'single_choice'
  }
  questions: QuizQuestion[]
  answers: QuizAnswer[]
  score?: {
    correct: number
    total: number
  } | null
  created_at: string
  updated_at: string
}

interface QuizQuestion {
  id: string
  question_type: 'single_choice'
  stem: string
  options: Array<{ id: string; text: string }>
  correct_option_id: string
  explanation: string
  citations: Array<{ source: string; text: string; score?: number | null }>
  tags: string[]
  difficulty: 'easy' | 'medium' | 'hard'
}

interface QuizAnswer {
  question_id: string
  selected_option_id: string
  correct: boolean
  answered_at: string
}

interface Props {
  knowledgeBases: KnowledgeBaseOption[]
  settings: LlmSettings
  onRefreshKnowledgeBases?: () => void
}

const difficultyOptions = [
  { value: 'easy', label: 'Easy' },
  { value: 'medium', label: 'Medium' },
  { value: 'hard', label: 'Hard' },
] as const

export function QuizPage({ knowledgeBases, settings, onRefreshKnowledgeBases }: Props) {
  const [quizzes, setQuizzes] = useState<QuizSession[]>([])
  const [activeQuizId, setActiveQuizId] = useState<string | null>(null)
  const [kbId, setKbId] = useState('')
  const [topic, setTopic] = useState('')
  const [difficulty, setDifficulty] = useState<'easy' | 'medium' | 'hard'>('medium')
  const [questionCount, setQuestionCount] = useState(5)
  const [currentIndex, setCurrentIndex] = useState(0)
  const [selectedOptionId, setSelectedOptionId] = useState('')
  const [busy, setBusy] = useState(false)
  const [status, setStatus] = useState('Ready')

  const activeQuiz = quizzes.find((quiz) => quiz.id === activeQuizId) ?? null
  const currentQuestion = activeQuiz?.questions[currentIndex] ?? null
  const currentAnswer = currentQuestion
    ? activeQuiz?.answers.find((answer) => answer.question_id === currentQuestion.id) ?? null
    : null
  const score = activeQuiz?.score ?? { correct: 0, total: activeQuiz?.questions.length ?? 0 }

  const missedQuestions = useMemo(() => {
    if (!activeQuiz) return []
    return activeQuiz.questions.filter((question) => {
      const answer = activeQuiz.answers.find((item) => item.question_id === question.id)
      return answer && !answer.correct
    })
  }, [activeQuiz])

  const refreshQuizzes = useCallback(async () => {
    const res = await fetch('/api/quizzes')
    const data = await safeJson(res)
    if (!res.ok) throw new Error(errorMessage(data, res.status))
    const items = (data.quizzes ?? []) as QuizSession[]
    setQuizzes(items)
    setActiveQuizId((current) => current && items.some((quiz) => quiz.id === current) ? current : items[0]?.id ?? null)
  }, [])

  useEffect(() => {
    refreshQuizzes().catch((err) => setStatus(err instanceof Error ? err.message : String(err)))
  }, [refreshQuizzes])

  useEffect(() => {
    setKbId((current) => current || knowledgeBases[0]?.id || '')
  }, [knowledgeBases])

  useEffect(() => {
    setSelectedOptionId(currentAnswer?.selected_option_id ?? '')
  }, [currentAnswer?.selected_option_id, currentQuestion?.id])

  const createQuiz = async () => {
    if (!kbId || busy) return
    setBusy(true)
    setStatus('Generating quiz...')
    try {
      const res = await fetch('/api/quizzes', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          kb_id: kbId,
          topic: topic.trim() || null,
          difficulty,
          question_count: questionCount,
          llm: settingsForSession(settings),
        }),
      })
      const data = await safeJson(res)
      if (!res.ok) throw new Error(errorMessage(data, res.status))
      const quiz = data.quiz as QuizSession
      setQuizzes((prev) => [quiz, ...prev.filter((item) => item.id !== quiz.id)])
      setActiveQuizId(quiz.id)
      setCurrentIndex(0)
      setStatus('Quiz generated from knowledge chunks')
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const submitAnswer = async () => {
    if (!activeQuiz || !currentQuestion || !selectedOptionId || busy) return
    setBusy(true)
    setStatus('Scoring answer...')
    try {
      const res = await fetch(`/api/quizzes/${encodeURIComponent(activeQuiz.id)}/answers`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          question_id: currentQuestion.id,
          selected_option_id: selectedOptionId,
        }),
      })
      const data = await safeJson(res)
      if (!res.ok) throw new Error(errorMessage(data, res.status))
      upsertQuiz(data.quiz as QuizSession)
      setStatus('Answer scored')
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const finishQuiz = async () => {
    if (!activeQuiz || busy) return
    setBusy(true)
    setStatus('Finishing quiz...')
    try {
      const res = await fetch(`/api/quizzes/${encodeURIComponent(activeQuiz.id)}/finish`, { method: 'POST' })
      const data = await safeJson(res)
      if (!res.ok) throw new Error(errorMessage(data, res.status))
      upsertQuiz(data.quiz as QuizSession)
      setStatus('Quiz finished')
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const deleteQuiz = async (quiz: QuizSession) => {
    if (!window.confirm(`Delete "${quiz.title}"?`)) return
    const previous = quizzes
    setQuizzes((prev) => prev.filter((item) => item.id !== quiz.id))
    if (activeQuizId === quiz.id) setActiveQuizId(null)
    try {
      const res = await fetch(`/api/quizzes/${encodeURIComponent(quiz.id)}`, { method: 'DELETE' })
      if (!res.ok) {
        const data = await safeJson(res)
        throw new Error(errorMessage(data, res.status))
      }
    } catch (err) {
      setQuizzes(previous)
      setStatus(err instanceof Error ? err.message : String(err))
    }
  }

  const upsertQuiz = (quiz: QuizSession) => {
    setQuizzes((prev) => [quiz, ...prev.filter((item) => item.id !== quiz.id)])
    setActiveQuizId(quiz.id)
  }

  return (
    <div className="flex h-full min-h-0 bg-white">
      <aside className="flex w-80 shrink-0 flex-col border-r border-gray-200 bg-gray-50">
        <div className="border-b border-gray-200 px-5 py-4">
          <div className="flex items-center justify-between">
            <div>
              <h1 className="text-xl font-semibold text-gray-950">Quiz</h1>
              <p className="mt-1 text-sm text-gray-500">Generate and review knowledge checks.</p>
            </div>
            <button
              className="rounded-lg p-2 text-gray-500 hover:bg-white hover:text-blue-700"
              type="button"
              title="Refresh"
              onClick={() => {
                void refreshQuizzes()
                onRefreshKnowledgeBases?.()
              }}
            >
              <RefreshCw size={18} />
            </button>
          </div>
        </div>

        <div className="space-y-4 border-b border-gray-200 p-4">
          <Field label="Knowledge base">
            <select className={inputClassName} value={kbId} onChange={(event) => setKbId(event.target.value)}>
              {knowledgeBases.length === 0 ? (
                <option value="">No knowledge base</option>
              ) : (
                knowledgeBases.map((kb) => <option key={kb.id} value={kb.id}>{kb.name}</option>)
              )}
            </select>
          </Field>
          <Field label="Topic">
            <input
              className={inputClassName}
              value={topic}
              onChange={(event) => setTopic(event.target.value)}
              placeholder="Optional focus topic"
            />
          </Field>
          <div className="grid grid-cols-2 gap-3">
            <Field label="Difficulty">
              <select className={inputClassName} value={difficulty} onChange={(event) => setDifficulty(event.target.value as typeof difficulty)}>
                {difficultyOptions.map((item) => <option key={item.value} value={item.value}>{item.label}</option>)}
              </select>
            </Field>
            <Field label="Questions">
              <input
                className={inputClassName}
                type="number"
                min={1}
                max={10}
                value={questionCount}
                onChange={(event) => setQuestionCount(Number(event.target.value))}
              />
            </Field>
          </div>
          <button
            className="flex h-10 w-full items-center justify-center gap-2 rounded-lg bg-blue-600 text-sm font-medium text-white shadow-sm shadow-blue-950/10 hover:bg-blue-700 disabled:bg-gray-200 disabled:text-gray-400"
            type="button"
            disabled={!kbId || busy}
            onClick={createQuiz}
          >
            <Play size={17} />
            Generate quiz
          </button>
          <div className="text-xs text-gray-500">{status}</div>
        </div>

        <div className="flex-1 overflow-y-auto p-3">
          <div className="mb-2 px-1 text-xs font-medium uppercase text-gray-500">Recent quizzes</div>
          {quizzes.length === 0 ? (
            <div className="rounded-lg px-3 py-8 text-center text-sm text-gray-400">No quizzes yet</div>
          ) : (
            <div className="space-y-2">
              {quizzes.map((quiz) => (
                <button
                  key={quiz.id}
                  className={`group flex w-full items-start gap-2 rounded-lg p-3 text-left text-sm ${
                    activeQuizId === quiz.id ? 'bg-white shadow-sm ring-1 ring-blue-100' : 'hover:bg-white'
                  }`}
                  type="button"
                  onClick={() => {
                    setActiveQuizId(quiz.id)
                    setCurrentIndex(0)
                  }}
                >
                  <FileQuestion size={17} className="mt-0.5 shrink-0 text-blue-600" />
                  <span className="min-w-0 flex-1">
                    <span className="block truncate font-medium text-gray-900">{quiz.title}</span>
                    <span className="mt-0.5 block text-xs text-gray-500">{quiz.score?.correct ?? 0}/{quiz.score?.total ?? quiz.questions.length} correct</span>
                  </span>
                  <span
                    role="button"
                    tabIndex={0}
                    className="rounded p-1 text-gray-400 opacity-0 hover:bg-red-50 hover:text-red-600 group-hover:opacity-100"
                    onClick={(event) => {
                      event.stopPropagation()
                      void deleteQuiz(quiz)
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

      <main className="flex min-w-0 flex-1 flex-col">
        {!activeQuiz || !currentQuestion ? (
          <div className="flex flex-1 items-center justify-center px-6">
            <div className="max-w-md text-center">
              <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-2xl bg-blue-50 text-blue-700">
                <BookOpenCheck size={28} />
              </div>
              <h2 className="mt-5 text-2xl font-semibold text-gray-950">Create a quiz from your knowledge base</h2>
              <p className="mt-2 text-sm leading-6 text-gray-500">
                Select a knowledge base and generate single-choice questions grounded in retrieved source chunks.
              </p>
            </div>
          </div>
        ) : (
          <div className="flex flex-1 min-h-0 flex-col">
            <header className="flex items-center border-b border-gray-100 px-8 py-5">
              <div>
                <h2 className="text-xl font-semibold text-gray-950">{activeQuiz.title}</h2>
                <p className="mt-1 text-sm text-gray-500">
                  Question {currentIndex + 1} of {activeQuiz.questions.length} · {difficultyLabel(activeQuiz.config.difficulty)}
                </p>
              </div>
              <div className="ml-auto rounded-lg bg-blue-50 px-3 py-2 text-sm font-medium text-blue-700">
                Score {score.correct}/{score.total}
              </div>
            </header>

            <div className="flex-1 overflow-y-auto px-8 py-6">
              <section className="max-w-4xl">
                <div className="mb-4 flex flex-wrap gap-2">
                  {currentQuestion.tags.map((tag) => (
                    <span key={tag} className="rounded-full bg-gray-100 px-2.5 py-1 text-xs font-medium text-gray-600">{tag}</span>
                  ))}
                </div>
                <h3 className="text-2xl font-semibold leading-9 text-gray-950">{currentQuestion.stem}</h3>

                <div className="mt-6 space-y-3">
                  {currentQuestion.options.map((option) => {
                    const selected = selectedOptionId === option.id
                    const answered = Boolean(currentAnswer)
                    const isCorrect = currentQuestion.correct_option_id === option.id
                    return (
                      <button
                        key={option.id}
                        type="button"
                        disabled={answered}
                        onClick={() => setSelectedOptionId(option.id)}
                        className={`flex w-full items-start gap-3 rounded-lg border p-4 text-left transition ${
                          selected
                            ? 'border-blue-300 bg-blue-50'
                            : 'border-gray-200 bg-white hover:border-blue-200 hover:bg-blue-50/40'
                        } ${answered && isCorrect ? 'border-emerald-300 bg-emerald-50' : ''}`}
                      >
                        <span className="mt-0.5 text-blue-700">
                          {selected || (answered && isCorrect) ? <CheckCircle2 size={19} /> : <Circle size={19} />}
                        </span>
                        <span>
                          <span className="font-medium text-gray-950">{option.id}.</span>{' '}
                          <span className="text-gray-700">{option.text}</span>
                        </span>
                      </button>
                    )
                  })}
                </div>

                {currentAnswer && (
                  <div className={`mt-6 rounded-lg p-4 ${currentAnswer.correct ? 'bg-emerald-50 text-emerald-900' : 'bg-red-50 text-red-900'}`}>
                    <div className="font-medium">{currentAnswer.correct ? 'Correct' : 'Not quite'}</div>
                    <p className="mt-2 text-sm leading-6">{currentQuestion.explanation}</p>
                    {currentQuestion.citations.length > 0 && (
                      <div className="mt-3 space-y-2">
                        {currentQuestion.citations.map((citation, index) => (
                          <div key={`${citation.source}-${index}`} className="rounded-md bg-white/70 p-2 text-xs">
                            <div className="font-medium">{citation.source}</div>
                            <div className="mt-1 text-gray-600">{citation.text}</div>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                )}
              </section>

              {activeQuiz.status === 'finished' && (
                <section className="mt-8 max-w-4xl rounded-lg border border-gray-200 bg-gray-50 p-5">
                  <h3 className="text-lg font-semibold text-gray-950">Summary</h3>
                  <p className="mt-2 text-sm text-gray-600">Final score: {score.correct}/{score.total}</p>
                  {missedQuestions.length > 0 ? (
                    <div className="mt-4 space-y-2">
                      {missedQuestions.map((question) => (
                        <div key={question.id} className="rounded-md bg-white p-3 text-sm text-gray-700">{question.stem}</div>
                      ))}
                    </div>
                  ) : (
                    <p className="mt-4 text-sm text-emerald-700">No missed questions.</p>
                  )}
                </section>
              )}
            </div>

            <footer className="flex items-center gap-3 border-t border-gray-100 px-8 py-4">
              <button className={secondaryButtonClassName} type="button" disabled={currentIndex === 0} onClick={() => setCurrentIndex((value) => Math.max(0, value - 1))}>
                Previous
              </button>
              <button className={secondaryButtonClassName} type="button" disabled={currentIndex >= activeQuiz.questions.length - 1} onClick={() => setCurrentIndex((value) => Math.min(activeQuiz.questions.length - 1, value + 1))}>
                Next
              </button>
              <button className={primaryButtonClassName} type="button" disabled={!selectedOptionId || Boolean(currentAnswer) || busy} onClick={submitAnswer}>
                Submit answer
              </button>
              <button className={secondaryButtonClassName} type="button" disabled={busy || activeQuiz.status === 'finished'} onClick={finishQuiz}>
                Finish
              </button>
            </footer>
          </div>
        )}
      </main>
    </div>
  )
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-xs font-medium uppercase text-gray-500">{label}</span>
      {children}
    </label>
  )
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

const inputClassName = 'w-full rounded-lg border border-gray-200 bg-white px-3 py-2 text-sm text-gray-900 outline-none focus:border-blue-300 focus:ring-2 focus:ring-blue-50'
const primaryButtonClassName = 'ml-auto inline-flex h-9 items-center justify-center rounded-lg bg-blue-600 px-3.5 text-sm font-medium text-white hover:bg-blue-700 disabled:bg-gray-200 disabled:text-gray-400'
const secondaryButtonClassName = 'inline-flex h-9 items-center justify-center rounded-lg border border-gray-200 px-3.5 text-sm font-medium text-gray-700 hover:bg-blue-50 hover:text-blue-700 disabled:opacity-50'
