export interface QuizSession {
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
  verification?: {
    status: 'verified' | 'warning'
    method: string
    checked_at: string
    issues: string[]
  } | null
  created_at: string
  updated_at: string
}

export interface QuizQuestion {
  id: string
  question_type: 'single_choice'
  stem: string
  options: Array<{ id: string; text: string }>
  correct_option_id: string
  explanation: string
  citations: Array<{
    source: string
    text: string
    score?: number | null
    kb?: string | null
    document_id?: string | null
    chunk_id?: string | null
    title?: string | null
  }>
  tags: string[]
  difficulty: 'easy' | 'medium' | 'hard'
}

export interface QuizAnswer {
  question_id: string
  selected_option_id: string
  correct: boolean
  answered_at: string
}
