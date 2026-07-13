import type { QuizSession } from './quizTypes'

export interface QuizMessageArtifact {
  type: string
  quiz_id?: string
}

export interface QuizRestorableMessage {
  role: 'user' | 'assistant' | 'status'
  text: string
  quiz?: QuizSession
  artifacts?: QuizMessageArtifact[]
}

export interface QuizTraceEntry {
  kind: string
  payload: Record<string, unknown>
}

export async function attachRestoredQuizzesToMessages<T extends QuizRestorableMessage>(
  messages: T[],
  traceEntries: QuizTraceEntry[],
  fetchQuiz: (id: string) => Promise<QuizSession | null>,
): Promise<T[]> {
  const artifactQuizIds = messages
    .flatMap((message) =>
      message.role === 'assistant'
        ? (message.artifacts ?? [])
          .filter((artifact) => artifact.type === 'quiz_session' && typeof artifact.quiz_id === 'string')
          .map((artifact) => artifact.quiz_id as string)
        : [],
    )
  const restoredQuizzes = traceEntries
    .map((entry) => quizFromTrace(entry.payload))
    .filter((quiz): quiz is QuizSession => Boolean(quiz))
  const quizIds = traceEntries
    .filter((entry) => entry.kind === 'quiz_created')
    .map((entry) => {
      const payload = entry.payload
      return typeof payload.quiz_id === 'string' ? payload.quiz_id : null
    })
    .filter((id): id is string => Boolean(id))

  if (artifactQuizIds.length === 0 && restoredQuizzes.length === 0 && quizIds.length === 0) return messages

  const idsToFetch = Array.from(new Set([...artifactQuizIds, ...quizIds]))
  const fetchedQuizzes = await Promise.all(idsToFetch.map(fetchQuiz))
  const quizzes = [
    ...restoredQuizzes,
    ...fetchedQuizzes.filter((quiz): quiz is QuizSession => Boolean(quiz)),
  ]
  const quizzesById = new Map(quizzes.map((quiz) => [quiz.id, quiz]))

  let nextMessages = messages.map((message) => {
    if (message.role !== 'assistant' || message.quiz) return message
    const quizId = (message.artifacts ?? []).find(
      (artifact) => artifact.type === 'quiz_session' && typeof artifact.quiz_id === 'string',
    )?.quiz_id
    const quiz = quizId ? quizzesById.get(quizId) : undefined
    return quiz ? { ...message, quiz } : message
  }) as T[]

  const attachedQuizIds = new Set(
    nextMessages
      .map((message) => message.quiz?.id)
      .filter((id): id is string => Boolean(id)),
  )
  const remainingQuizzes = quizzes.filter((quiz) => !attachedQuizIds.has(quiz.id))
  if (remainingQuizzes.length === 0) return nextMessages

  let quizIndex = 0
  nextMessages = nextMessages.map((message) => {
    if (message.role !== 'assistant') return message
    if (message.quiz) return message
    const quiz = remainingQuizzes[quizIndex]
    quizIndex += 1
    return quiz ? { ...message, quiz } : message
  }) as T[]

  const unattachedQuizzes = remainingQuizzes.slice(quizIndex)
  if (unattachedQuizzes.length === 0) return nextMessages

  return [
    ...nextMessages,
    ...unattachedQuizzes.map((quiz) => ({
      role: 'assistant' as const,
      text: `Quiz "${quiz.title}" is ready.`,
      quiz,
      artifacts: [{ type: 'quiz_session', quiz_id: quiz.id }],
    } as T)),
  ]
}

export function quizFromTrace(payload: Record<string, unknown>): QuizSession | undefined {
  if (payload.kind !== 'tool_result' || payload.tool !== 'create_quiz' || payload.ok === false) return undefined
  const details = payload.details
  if (!details || typeof details !== 'object') return undefined
  const quiz = (details as Record<string, unknown>).quiz
  if (!quiz || typeof quiz !== 'object') return undefined
  const id = (quiz as Record<string, unknown>).id
  const title = (quiz as Record<string, unknown>).title
  const questions = (quiz as Record<string, unknown>).questions
  if (typeof id !== 'string' || typeof title !== 'string' || !Array.isArray(questions)) return undefined
  return quiz as QuizSession
}
