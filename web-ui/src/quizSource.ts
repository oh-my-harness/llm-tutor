import type { QuizSession } from './quizTypes'

export type QuizSourceFilter = 'all' | 'knowledge_base' | 'conversation' | 'space' | 'notebook'
export type QuizSourceType = Exclude<QuizSourceFilter, 'all'>

export const quizSourceFilters: Array<{ key: QuizSourceFilter; label: string }> = [
  { key: 'all', label: 'All' },
  { key: 'knowledge_base', label: 'Knowledge' },
  { key: 'conversation', label: 'Conversation' },
  { key: 'space', label: 'Space refs' },
  { key: 'notebook', label: 'Notebook' },
]

export function quizSourceType(quiz: QuizSession): QuizSourceType {
  const kbId = quiz.kb_id?.trim()
  if (kbId === '__notebook__') return 'notebook'
  if (kbId === '__conversation__') return 'conversation'
  if (kbId) return 'knowledge_base'

  const sources = quiz.questions.flatMap((question) =>
    question.citations.map((citation) => citation.source.toLowerCase()),
  )
  const sourceText = [quiz.title, quiz.config.topic ?? '', ...sources].join(' ').toLowerCase()
  if (sourceText.includes('space reference') || sourceText.includes('mentioned_space_items')) return 'space'
  if (sourceText.includes('notebook:') || sourceText.includes('notebook')) return 'notebook'
  return 'conversation'
}

export function quizSourceLabel(source: QuizSourceType) {
  if (source === 'knowledge_base') return 'Knowledge'
  if (source === 'space') return 'Space'
  if (source === 'notebook') return 'Notebook'
  return 'Conversation'
}
