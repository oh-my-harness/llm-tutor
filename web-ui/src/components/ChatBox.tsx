import { useEffect, useId, useRef, useState } from 'react'
import type { ChangeEvent, ReactNode } from 'react'
import {
  AlertCircle,
  ArrowUp,
  AtSign,
  Brain,
  CheckCircle2,
  ChevronDown,
  Code2,
  Database,
  FileText,
  FileQuestion,
  SearchCheck,
  MessageSquare,
  Paperclip,
  Sparkles,
  Circle,
  X,
} from 'lucide-react'
import type { LlmModelConfig } from '../settings'
import type { QuizSession } from '../quizTypes'
import { DeepSolveMessage, type DeepSolveTraceEntry } from './DeepSolveMessage'
import { MarkdownMessage, SourceReferences, sourceTargetFromRaw } from './MarkdownMessage'
import type { SourceReference, SourceTarget } from './MarkdownMessage'

type Capability = 'chat' | 'deep_solve' | 'code_exec' | 'quiz' | 'research'
type OpenMenu = 'mode' | 'knowledge' | 'model' | null

interface Message {
  role: 'user' | 'assistant' | 'status'
  text: string
  kind?: 'idle' | 'thinking' | 'tool' | 'done' | 'error'
  citations?: Citation[]
  deepSolve?: DeepSolveTraceEntry[]
  quiz?: QuizSession
  attachments?: ChatAttachment[]
}

export interface ChatAttachment {
  id: string
  name: string
  size: number
  type: string
  text?: string
  error?: string
  truncated?: boolean
}

interface Citation {
  index: number
  source: string
  text: string
  kind?: 'rag' | 'web'
  title?: string
  url?: string
  score?: number | null
  kb?: string
  documentId?: string
  chunkId?: string
  rawSource?: string
  page?: string | number
}

interface Props {
  messages: Message[]
  streamingText: string
  contextStats: ContextStats
  capability: Capability
  llmConfigs: LlmModelConfig[]
  activeLlmConfigId: string | null
  knowledgeBases: Array<{ id: string; name: string }>
  selectedKnowledgeBaseId: string
  onSend: (text: string, attachments?: ChatAttachment[]) => void
  onAskDeepSolveStep?: (step: { id: string; title: string; summary?: string }) => void
  onCapabilityChange: (capability: Capability) => void
  onKnowledgeBaseChange: (id: string) => void
  onLlmConfigChange: (id: string) => void
  onSaveToNotebook?: (markdown: string) => Promise<void>
  onQuizAnswer?: (quizId: string, questionId: string, selectedOptionId: string) => Promise<void>
  onQuizFinish?: (quizId: string) => Promise<void>
  onSourceNavigate?: (target: SourceTarget, reference: SourceReference) => void
  disabled: boolean
}

export interface ContextStats {
  usedTokens: number
  maxTokens: number
  source: 'provider' | 'estimate'
}

const modeOptions: Array<{ value: Capability; label: string; description: string; icon: ReactNode }> = [
  {
    value: 'chat',
    label: '聊天',
    description: '灵活对话，可使用任意工具',
    icon: <MessageSquare size={21} />,
  },
  {
    value: 'deep_solve',
    label: '解题',
    description: '多步推理与问题求解',
    icon: <Brain size={21} />,
  },
  {
    value: 'code_exec',
    label: '代码',
    description: '运行代码并验证结果',
    icon: <Code2 size={21} />,
  },
  {
    value: 'quiz',
    label: 'Quiz',
    description: '基于对话或知识库生成测验',
    icon: <FileQuestion size={21} />,
  },
  {
    value: 'research',
    label: '研究',
    description: '搜索、阅读并生成带引用的研究报告',
    icon: <SearchCheck size={21} />,
  },
]

export function ChatBox({
  messages,
  streamingText,
  contextStats,
  capability,
  llmConfigs,
  activeLlmConfigId,
  knowledgeBases,
  selectedKnowledgeBaseId,
  onSend,
  onAskDeepSolveStep,
  onCapabilityChange,
  onKnowledgeBaseChange,
  onLlmConfigChange,
  onSaveToNotebook,
  onQuizAnswer,
  onQuizFinish,
  onSourceNavigate,
  disabled,
}: Props) {
  const [input, setInput] = useState('')
  const [attachments, setAttachments] = useState<ChatAttachment[]>([])
  const scrollRef = useRef<HTMLDivElement>(null)
  const shouldStickToBottomRef = useRef(true)
  const empty = messages.length === 0 && !streamingText

  const handleSend = () => {
    const readyAttachments = attachments.filter((attachment) => !attachment.error)
    if ((!input.trim() && readyAttachments.length === 0) || disabled) return
    onSend(input.trim(), readyAttachments)
    setInput('')
    setAttachments([])
  }

  const handleAddAttachments = (items: ChatAttachment[]) => {
    setAttachments((current) => [...current, ...items])
  }

  const handleRemoveAttachment = (id: string) => {
    setAttachments((current) => current.filter((attachment) => attachment.id !== id))
  }

  const handleScroll = () => {
    const el = scrollRef.current
    if (!el) return

    const distanceFromBottom = el.scrollHeight - el.scrollTop - el.clientHeight
    shouldStickToBottomRef.current = distanceFromBottom < 80
  }

  useEffect(() => {
    const el = scrollRef.current
    if (!el || !shouldStickToBottomRef.current) return

    el.scrollTop = el.scrollHeight
  }, [messages, streamingText])

  return (
    <div className="flex h-full flex-col">
      {empty ? (
        <div className="flex flex-1 items-center justify-center px-6 pb-16">
          <div className="w-full max-w-4xl">
            <div className="mb-10 flex items-center justify-center gap-4 text-center">
              <Sparkles size={42} className="text-gray-800" />
              <h2 className="text-4xl font-semibold text-gray-900">你想学点什么？</h2>
            </div>
            <Composer
              input={input}
              setInput={setInput}
              capability={capability}
              llmConfigs={llmConfigs}
              activeLlmConfigId={activeLlmConfigId}
              knowledgeBases={knowledgeBases}
              selectedKnowledgeBaseId={selectedKnowledgeBaseId}
              onCapabilityChange={onCapabilityChange}
              onKnowledgeBaseChange={onKnowledgeBaseChange}
              onLlmConfigChange={onLlmConfigChange}
              onSend={handleSend}
              attachments={attachments}
              onAddAttachments={handleAddAttachments}
              onRemoveAttachment={handleRemoveAttachment}
              disabled={disabled}
              variant="center"
            />
          </div>
        </div>
      ) : (
        <>
          <ContextCapacity stats={contextStats} />
          <div ref={scrollRef} onScroll={handleScroll} className="flex-1 space-y-3 overflow-y-auto p-4">
            {messages.map((msg, i) => (
              <div key={i} className={messageClassName(msg)}>
                {msg.role === 'status' ? (
                  <div className="flex items-center gap-2 text-sm text-gray-600">
                    {(msg.kind === 'thinking' || msg.kind === 'tool') && (
                      <span className="h-2 w-2 animate-pulse rounded-full bg-current" />
                    )}
                    <span>{msg.text}</span>
                  </div>
                ) : msg.role === 'assistant' ? (
                  msg.quiz ? (
                    <ChatQuizCard
                      quiz={msg.quiz}
                      onAnswer={onQuizAnswer}
                      onFinish={onQuizFinish}
                      onSourceNavigate={onSourceNavigate}
                    />
                  ) : msg.deepSolve && msg.deepSolve.length > 0 ? (
                    <DeepSolveMessage
                      text={msg.text}
                      events={msg.deepSolve}
                      citations={msg.citations}
                      citationList={(citations) => <CitationList citations={citations} onSourceNavigate={onSourceNavigate} />}
                      onAskStep={onAskDeepSolveStep}
                    />
                  ) : (
                    <>
                      <MarkdownMessage text={msg.text} onSourceNavigate={onSourceNavigate} />
                      {capability === 'research' && msg.text.trim() && onSaveToNotebook && (
                        <div className="mt-3 flex justify-end">
                          <button
                            className="inline-flex h-8 items-center gap-2 rounded-lg border border-blue-100 bg-white px-3 text-xs font-medium text-blue-700 hover:bg-blue-50"
                            type="button"
                            onClick={() => {
                              void onSaveToNotebook(msg.text)
                            }}
                          >
                            <FileText size={16} />
                            保存到笔记本
                          </button>
                        </div>
                      )}
                      {msg.citations && msg.citations.length > 0 && (
                        <CitationList citations={msg.citations} onSourceNavigate={onSourceNavigate} />
                      )}
                    </>
                  )
                ) : (
                  <>
                    <pre className="whitespace-pre-wrap font-sans text-sm">{msg.text}</pre>
                    {msg.attachments && msg.attachments.length > 0 && (
                      <AttachmentSummary attachments={msg.attachments} />
                    )}
                  </>
                )}
              </div>
            ))}
            {streamingText && (
              <div className="max-w-3xl rounded-lg bg-gray-100 p-3">
                <MarkdownMessage text={streamingText} onSourceNavigate={onSourceNavigate} />
                <span className="animate-pulse">|</span>
              </div>
            )}
          </div>
          <div className="bg-gray-50 p-4">
            <Composer
              input={input}
              setInput={setInput}
              capability={capability}
              llmConfigs={llmConfigs}
              activeLlmConfigId={activeLlmConfigId}
              knowledgeBases={knowledgeBases}
              selectedKnowledgeBaseId={selectedKnowledgeBaseId}
              onCapabilityChange={onCapabilityChange}
              onKnowledgeBaseChange={onKnowledgeBaseChange}
              onLlmConfigChange={onLlmConfigChange}
              onSend={handleSend}
              attachments={attachments}
              onAddAttachments={handleAddAttachments}
              onRemoveAttachment={handleRemoveAttachment}
              disabled={disabled}
              variant="bottom"
            />
          </div>
        </>
      )}
    </div>
  )
}

function ContextCapacity({ stats }: { stats: ContextStats }) {
  const maxTokens = Math.max(1, stats.maxTokens)
  const usedTokens = Math.max(0, stats.usedTokens)
  const percent = Math.min(100, Math.round((usedTokens / maxTokens) * 100))
  const tone =
    percent >= 90
      ? 'bg-red-500'
      : percent >= 75
        ? 'bg-amber-500'
        : 'bg-blue-600'

  return (
    <div className="border-b border-blue-50 bg-white px-5 py-2">
      <div className="flex items-center gap-3 text-xs text-gray-500">
        <span className="font-medium text-gray-700">上下文容量</span>
        <div className="h-1.5 w-36 overflow-hidden rounded-full bg-gray-100">
          <div className={`h-full rounded-full ${tone}`} style={{ width: `${percent}%` }} />
        </div>
        <span>
          {formatTokenCount(usedTokens)} / {formatTokenCount(maxTokens)}
        </span>
        <span className="text-gray-400">{percent}%</span>
        <span className="text-gray-400">{stats.source === 'provider' ? '上次请求' : '估算'}</span>
      </div>
    </div>
  )
}

function formatTokenCount(value: number) {
  if (value >= 1000) return `${(value / 1000).toFixed(value >= 10000 ? 0 : 1)}k`
  return String(value)
}

function CitationList({
  citations,
  onSourceNavigate,
}: {
  citations: Citation[]
  onSourceNavigate?: (target: SourceTarget, reference: SourceReference) => void
}) {
  const rawId = useId()
  const hasWeb = citations.some((citation) => citation.kind === 'web' || citation.url)
  const references = citations.map(citationToSourceReference)
  return (
    <div className="mt-3 border-t border-gray-200 pt-3" data-source-kind={hasWeb ? 'web' : 'rag'}>
      <div className="mb-2 text-xs font-medium text-gray-500">{hasWeb ? '网页来源' : '引用来源'}</div>
      <SourceReferences
        id={`chat-citations-${rawId.replace(/[^a-zA-Z0-9_-]/g, '')}`}
        references={references}
        onNavigate={onSourceNavigate}
      />
    </div>
  )
}

function citationToSourceReference(citation: Citation, index: number): SourceReference {
  const raw = citation.url || citationRawTarget(citation)
  const target = sourceTargetFromRaw(raw)
  return {
    id: `${citation.index || index + 1}:${raw}`,
    label: String(citation.index || index + 1),
    raw,
    surface: citation.kind === 'web' || citation.url || target?.type === 'web' ? 'web' : target?.type === 'kb' ? 'kb' : 'unknown',
    title: citation.title || citation.source,
    description: citation.text,
    score: citation.score,
    metadata: {
      documentName: citation.kind === 'rag' ? citation.title || citation.source : undefined,
      documentId: citation.documentId,
      chunkId: citation.chunkId,
      page: citation.page,
      url: citation.url,
      missingReason: target ? undefined : 'No navigable source id was provided by the tool result.',
    },
    target,
  }
}

function citationRawTarget(citation: Citation) {
  if (citation.kb && citation.documentId) {
    return ['kb', citation.kb, citation.documentId, citation.chunkId].filter(Boolean).join(':')
  }
  return citation.rawSource || citation.source
}

function ChatQuizCard({
  quiz,
  onAnswer,
  onFinish,
  onSourceNavigate,
}: {
  quiz: QuizSession
  onAnswer?: (quizId: string, questionId: string, selectedOptionId: string) => Promise<void>
  onFinish?: (quizId: string) => Promise<void>
  onSourceNavigate?: (target: SourceTarget, reference: SourceReference) => void
}) {
  const [currentIndex, setCurrentIndex] = useState(0)
  const [selectedOptionId, setSelectedOptionId] = useState('')
  const [busy, setBusy] = useState(false)
  const question = quiz.questions[currentIndex] ?? null
  const answer = question ? quiz.answers.find((item) => item.question_id === question.id) ?? null : null
  const score = quiz.score ?? { correct: 0, total: quiz.questions.length }

  useEffect(() => {
    setSelectedOptionId(answer?.selected_option_id ?? '')
  }, [answer?.selected_option_id, question?.id])

  if (!question) {
    return (
      <div className="rounded-lg border border-blue-100 bg-white p-4">
        <div className="flex items-center gap-2 text-sm font-semibold text-blue-800">
          <FileQuestion size={18} />
          Quiz
        </div>
        <p className="mt-3 text-sm text-gray-600">测验还没有生成题目。</p>
      </div>
    )
  }

  const submit = async () => {
    if (!selectedOptionId || answer || !onAnswer || busy) return
    setBusy(true)
    try {
      await onAnswer(quiz.id, question.id, selectedOptionId)
    } finally {
      setBusy(false)
    }
  }

  const finish = async () => {
    if (!onFinish || busy || quiz.status === 'finished') return
    setBusy(true)
    try {
      await onFinish(quiz.id)
    } finally {
      setBusy(false)
    }
  }

  return (
    <div className="space-y-4 rounded-lg border border-blue-100 bg-white p-4">
      <div className="flex items-start gap-3">
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-blue-50 text-blue-700">
          <FileQuestion size={19} />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <h3 className="truncate text-base font-semibold text-gray-950">{quiz.title || 'Quiz'}</h3>
            <span className="rounded-full bg-blue-50 px-2 py-0.5 text-xs font-medium text-blue-700">
              {quiz.status}
            </span>
          </div>
          <p className="mt-1 text-xs text-gray-500">
            Question {currentIndex + 1} of {quiz.questions.length} · Score {score.correct}/{score.total}
          </p>
        </div>
      </div>

      <div>
        <div className="mb-3 flex flex-wrap gap-2">
          {question.tags.map((tag) => (
            <span key={tag} className="rounded-full bg-gray-100 px-2 py-0.5 text-xs text-gray-600">
              {tag}
            </span>
          ))}
        </div>
        <p className="text-base font-medium leading-7 text-gray-950">{question.stem}</p>
      </div>

      <div className="space-y-2">
        {question.options.map((option) => {
          const selected = selectedOptionId === option.id
          const answered = Boolean(answer)
          const isCorrect = question.correct_option_id === option.id
          return (
            <button
              key={option.id}
              type="button"
              disabled={answered || busy}
              onClick={() => setSelectedOptionId(option.id)}
              className={`flex w-full items-start gap-3 rounded-lg border p-3 text-left text-sm transition ${
                selected ? 'border-blue-300 bg-blue-50' : 'border-gray-200 bg-white hover:border-blue-200 hover:bg-blue-50/40'
              } ${answered && isCorrect ? 'border-emerald-300 bg-emerald-50' : ''}`}
            >
              <span className="mt-0.5 text-blue-700">
                {selected || (answered && isCorrect) ? <CheckCircle2 size={18} /> : <Circle size={18} />}
              </span>
              <span className="leading-6 text-gray-700">
                <span className="font-medium text-gray-950">{option.id}.</span> {option.text}
              </span>
            </button>
          )
        })}
      </div>

      {answer && (
        <div className={`rounded-lg p-3 text-sm ${answer.correct ? 'bg-emerald-50 text-emerald-900' : 'bg-red-50 text-red-900'}`}>
          <div className="font-medium">{answer.correct ? '回答正确' : '回答错误'}</div>
          <p className="mt-2 leading-6">{question.explanation}</p>
          {question.citations.length > 0 && (
            <QuizCitationReferences
              quizId={quiz.id}
              questionId={question.id}
              citations={question.citations}
              onSourceNavigate={onSourceNavigate}
            />
          )}
        </div>
      )}

      <div className="flex flex-wrap items-center gap-2 border-t border-gray-100 pt-3">
        <button
          className="inline-flex h-8 items-center rounded-lg border border-gray-200 px-3 text-xs font-medium text-gray-700 hover:bg-blue-50 disabled:opacity-50"
          type="button"
          disabled={currentIndex === 0}
          onClick={() => setCurrentIndex((value) => Math.max(0, value - 1))}
        >
          上一题
        </button>
        <button
          className="inline-flex h-8 items-center rounded-lg border border-gray-200 px-3 text-xs font-medium text-gray-700 hover:bg-blue-50 disabled:opacity-50"
          type="button"
          disabled={currentIndex >= quiz.questions.length - 1}
          onClick={() => setCurrentIndex((value) => Math.min(quiz.questions.length - 1, value + 1))}
        >
          下一题
        </button>
        <button
          className="ml-auto inline-flex h-8 items-center rounded-lg bg-blue-600 px-3 text-xs font-medium text-white hover:bg-blue-700 disabled:bg-gray-200 disabled:text-gray-400"
          type="button"
          disabled={!selectedOptionId || Boolean(answer) || busy}
          onClick={submit}
        >
          提交答案
        </button>
        <button
          className="inline-flex h-8 items-center rounded-lg border border-gray-200 px-3 text-xs font-medium text-gray-700 hover:bg-blue-50 disabled:opacity-50"
          type="button"
          disabled={busy || quiz.status === 'finished'}
          onClick={finish}
        >
          结束测验
        </button>
      </div>
    </div>
  )
}

function QuizCitationReferences({
  quizId,
  questionId,
  citations,
  onSourceNavigate,
}: {
  quizId: string
  questionId: string
  citations: QuizSession['questions'][number]['citations']
  onSourceNavigate?: (target: SourceTarget, reference: SourceReference) => void
}) {
  const references = citations.map((citation, index) => quizCitationToSourceReference(citation, index))
  return (
    <SourceReferences
      id={`quiz-citations-${quizId}-${questionId}`}
      references={references}
      onNavigate={onSourceNavigate}
    />
  )
}

function quizCitationToSourceReference(
  citation: QuizSession['questions'][number]['citations'][number],
  index: number,
): SourceReference {
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

function quizCitationRawTarget(citation: QuizSession['questions'][number]['citations'][number]) {
  if (citation.kb && citation.document_id) {
    return ['kb', citation.kb, citation.document_id, citation.chunk_id].filter(Boolean).join(':')
  }
  return citation.source
}

function Composer({
  input,
  setInput,
  capability,
  llmConfigs,
  activeLlmConfigId,
  knowledgeBases,
  selectedKnowledgeBaseId,
  onCapabilityChange,
  onKnowledgeBaseChange,
  onLlmConfigChange,
  onSend,
  attachments,
  onAddAttachments,
  onRemoveAttachment,
  disabled,
  variant,
}: {
  input: string
  setInput: (value: string) => void
  capability: Capability
  llmConfigs: LlmModelConfig[]
  activeLlmConfigId: string | null
  knowledgeBases: Array<{ id: string; name: string }>
  selectedKnowledgeBaseId: string
  onCapabilityChange: (capability: Capability) => void
  onKnowledgeBaseChange: (id: string) => void
  onLlmConfigChange: (id: string) => void
  onSend: () => void
  attachments: ChatAttachment[]
  onAddAttachments: (attachments: ChatAttachment[]) => void
  onRemoveAttachment: (id: string) => void
  disabled: boolean
  variant: 'center' | 'bottom'
}) {
  const [openMenu, setOpenMenu] = useState<OpenMenu>(null)
  const [readingAttachments, setReadingAttachments] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)
  const activeMode = modeOptions.find((mode) => mode.value === capability) ?? modeOptions[0]!
  const activeKnowledge = knowledgeBases.find((item) => item.id === selectedKnowledgeBaseId)
  const activeModel = llmConfigs.find((item) => item.id === activeLlmConfigId) ?? llmConfigs[0] ?? null
  const knowledgeOptions = [
    {
      id: '',
      name: '不关联知识库',
      description: '仅使用当前对话上下文',
      icon: <Database size={21} />,
    },
    ...knowledgeBases.map((item) => ({
      id: item.id,
      name: item.name,
      description: '关联此知识库进行检索',
      icon: <Database size={21} />,
    })),
  ]

  const toggleMenu = (menu: OpenMenu) => {
    if (disabled) return
    setOpenMenu((current) => (current === menu ? null : menu))
  }

  const handleFileChange = async (event: ChangeEvent<HTMLInputElement>) => {
    const files = Array.from(event.target.files ?? [])
    event.target.value = ''
    if (files.length === 0) return

    setReadingAttachments(true)
    try {
      const parsed = await Promise.all(files.map(readChatAttachment))
      onAddAttachments(parsed)
    } finally {
      setReadingAttachments(false)
    }
  }

  return (
    <div
      className={`relative rounded-3xl border border-blue-100 bg-white shadow-sm ${
        variant === 'center' ? 'shadow-xl shadow-blue-950/5' : ''
      }`}
    >
      <textarea
        className={`${
          variant === 'center' ? 'min-h-36 text-base' : 'min-h-16 text-sm'
        } w-full resize-none rounded-t-3xl px-5 py-4 outline-none placeholder:text-gray-400 disabled:bg-white`}
        value={input}
        onChange={(event) => setInput(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Enter' && !event.shiftKey) {
            event.preventDefault()
            onSend()
          }
        }}
        placeholder="今天我能帮您什么？"
      />
      {attachments.length > 0 && (
        <div className="border-t border-blue-50 px-4 py-2">
          <AttachmentSummary
            attachments={attachments}
            removable
            onRemove={onRemoveAttachment}
          />
        </div>
      )}
      <div className="relative flex flex-wrap items-center gap-2 border-t border-blue-50 px-4 py-2">
        <div className="relative">
          <ToolbarButton
            active={openMenu === 'mode'}
            icon={activeMode.icon}
            label={activeMode.label}
            onClick={() => toggleMenu('mode')}
          />
          {openMenu === 'mode' && (
            <DropdownPanel widthClassName="w-[33rem]">
              {modeOptions.map((mode) => (
                <DropdownOption
                  key={mode.value}
                  selected={mode.value === capability}
                  icon={mode.icon}
                  title={mode.label}
                  description={mode.description}
                  onClick={() => {
                    onCapabilityChange(mode.value)
                    setOpenMenu(null)
                  }}
                />
              ))}
            </DropdownPanel>
          )}
        </div>

        <button
          className="inline-flex h-9 items-center gap-2 rounded-full px-3 text-sm text-gray-600 hover:bg-blue-50 disabled:text-gray-400"
          type="button"
          disabled={disabled || readingAttachments}
          onClick={() => fileInputRef.current?.click()}
        >
          <Paperclip size={18} />
          附件
        </button>
        <input
          ref={fileInputRef}
          className="hidden"
          type="file"
          multiple
          onChange={handleFileChange}
        />

        <div className="relative">
          <ToolbarButton
            active={openMenu === 'knowledge'}
            icon={<Database size={18} />}
            label={activeKnowledge?.name ?? '不关联知识库'}
            onClick={() => toggleMenu('knowledge')}
          />
          {openMenu === 'knowledge' && (
            <DropdownPanel widthClassName="w-[28rem]">
              {knowledgeOptions.map((item) => (
                <DropdownOption
                  key={item.id || 'none'}
                  selected={item.id === selectedKnowledgeBaseId}
                  icon={item.icon}
                  title={item.name}
                  description={item.description}
                  onClick={() => {
                    onKnowledgeBaseChange(item.id)
                    setOpenMenu(null)
                  }}
                />
              ))}
            </DropdownPanel>
          )}
        </div>

        <button
          className="inline-flex h-9 items-center gap-2 rounded-full px-3 text-sm text-gray-600 hover:bg-blue-50"
          type="button"
        >
          <AtSign size={18} />
          空间
          <ChevronDown size={16} />
        </button>

        <div className="relative ml-auto">
          <ToolbarButton
            active={openMenu === 'model'}
            icon={<Brain size={16} />}
            label={activeModel?.model ?? '选择模型'}
            onClick={() => toggleMenu('model')}
          />
          {openMenu === 'model' && (
            <DropdownPanel widthClassName="right-0 left-auto w-[30rem]">
              {llmConfigs.length === 0 ? (
                <DropdownOption
                  selected
                  icon={<Brain size={21} />}
                  title="暂无模型配置"
                  description="请先到设置中添加 LLM 配置"
                  onClick={() => setOpenMenu(null)}
                />
              ) : (
                llmConfigs.map((config) => (
                  <DropdownOption
                    key={config.id}
                    selected={config.id === activeModel?.id}
                    icon={<Brain size={21} />}
                    title={config.name || config.model}
                    description={`${llmApiModeLabel(config.provider)} · ${config.model}`}
                    onClick={() => {
                      onLlmConfigChange(config.id)
                      setOpenMenu(null)
                    }}
                  />
                ))
              )}
            </DropdownPanel>
          )}
        </div>

        <button
          className="flex h-9 w-9 items-center justify-center rounded-full bg-blue-600 text-white disabled:bg-gray-200 disabled:text-gray-400"
          onClick={onSend}
          disabled={disabled || (!input.trim() && attachments.filter((attachment) => !attachment.error).length === 0)}
          type="button"
          title="发送"
        >
          <ArrowUp size={20} />
        </button>
      </div>
    </div>
  )
}

const MAX_ATTACHMENT_BYTES = 16 * 1024 * 1024
const MAX_ATTACHMENT_CHARS = 20000
const TEXT_EXTENSIONS = new Set([
  'txt',
  'md',
  'markdown',
  'csv',
  'tsv',
  'json',
  'jsonl',
  'log',
  'rs',
  'ts',
  'tsx',
  'js',
  'jsx',
  'py',
  'toml',
  'yaml',
  'yml',
  'xml',
  'html',
  'css',
  'sql',
])

async function readChatAttachment(file: File): Promise<ChatAttachment> {
  const base = {
    id: `${file.name}-${file.size}-${file.lastModified}-${Math.random().toString(36).slice(2)}`,
    name: file.name,
    size: file.size,
    type: file.type || 'application/octet-stream',
  }

  if (file.size > MAX_ATTACHMENT_BYTES) {
    return {
      ...base,
      error: `附件超过 ${formatBytes(MAX_ATTACHMENT_BYTES)}，请拆分后再发送。`,
    }
  }

  if (!isTextFile(file) || isPdfFile(file)) {
    return parseAttachmentOnServer(file, base)
  }

  try {
    const raw = await file.text()
    const truncated = raw.length > MAX_ATTACHMENT_CHARS
    return {
      ...base,
      text: truncated ? raw.slice(0, MAX_ATTACHMENT_CHARS) : raw,
      truncated,
    }
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err)
    return { ...base, error: `读取失败：${message}` }
  }
}

async function parseAttachmentOnServer(
  file: File,
  base: Pick<ChatAttachment, 'id' | 'name' | 'size' | 'type'>,
): Promise<ChatAttachment> {
  const form = new FormData()
  form.append('file', file)
  try {
    const res = await fetch('/api/attachments/parse', {
      method: 'POST',
      body: form,
    })
    const data = await res.json().catch(() => ({})) as {
      attachment?: {
        name?: string
        size?: number
        mime_type?: string | null
        text?: string
        truncated?: boolean
      }
      error?: string
    }
    if (!res.ok || !data.attachment?.text) {
      return { ...base, error: data.error || `附件解析失败：HTTP ${res.status}` }
    }
    return {
      ...base,
      name: data.attachment.name || base.name,
      size: data.attachment.size ?? base.size,
      type: data.attachment.mime_type || base.type,
      text: data.attachment.text,
      truncated: Boolean(data.attachment.truncated),
    }
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err)
    return { ...base, error: `附件解析请求失败：${message}` }
  }
}

function isPdfFile(file: File) {
  return file.type === 'application/pdf' || file.name.toLowerCase().endsWith('.pdf')
}

function isTextFile(file: File) {
  if (file.type.startsWith('text/')) return true
  const ext = file.name.split('.').pop()?.toLowerCase()
  return Boolean(ext && TEXT_EXTENSIONS.has(ext))
}

function AttachmentSummary({
  attachments,
  removable = false,
  onRemove,
}: {
  attachments: ChatAttachment[]
  removable?: boolean
  onRemove?: (id: string) => void
}) {
  return (
    <div className="flex flex-wrap gap-2">
      {attachments.map((attachment) => (
        <div
          key={attachment.id}
          className={`flex max-w-full items-center gap-2 rounded-xl border px-3 py-2 text-xs ${
            attachment.error
              ? 'border-red-100 bg-red-50 text-red-700'
              : 'border-blue-100 bg-blue-50 text-gray-700'
          }`}
          title={attachment.error || attachment.name}
        >
          {attachment.error ? (
            <AlertCircle size={16} className="shrink-0" />
          ) : (
            <FileText size={16} className="shrink-0 text-blue-600" />
          )}
          <span className="min-w-0 truncate font-medium">{attachment.name}</span>
          <span className="shrink-0 text-gray-500">{formatBytes(attachment.size)}</span>
          {attachment.truncated && <span className="shrink-0 text-amber-600">truncated</span>}
          {attachment.error && <span className="min-w-0 truncate">{attachment.error}</span>}
          {removable && (
            <button
              className="ml-1 flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-gray-500 hover:bg-white hover:text-gray-900"
              type="button"
              onClick={() => onRemove?.(attachment.id)}
              title="移除附件"
            >
              <X size={14} />
            </button>
          )}
        </div>
      ))}
    </div>
  )
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}

function llmApiModeLabel(provider: LlmModelConfig['provider']) {
  if (provider === 'anthropic') return 'Anthropic Messages'
  return 'OpenAI-compatible'
}

function ToolbarButton({
  active,
  icon,
  label,
  onClick,
}: {
  active: boolean
  icon: ReactNode
  label: string
  onClick: () => void
}) {
  return (
    <button
      className={`inline-flex h-9 max-w-56 items-center gap-2 rounded-full border px-3 text-sm transition ${
        active
          ? 'border-blue-200 bg-blue-50 text-blue-700 shadow-sm'
          : 'border-transparent text-gray-700 hover:bg-blue-50'
      }`}
      type="button"
      onClick={onClick}
    >
      <span className="shrink-0">{icon}</span>
      <span className="truncate">{label}</span>
      <ChevronDown size={16} className={`shrink-0 transition ${active ? 'rotate-180' : ''}`} />
    </button>
  )
}

function DropdownPanel({ children, widthClassName }: { children: ReactNode; widthClassName: string }) {
  return (
    <div
      className={`absolute bottom-12 left-0 z-30 overflow-hidden rounded-2xl border border-blue-100 bg-white py-2 shadow-2xl shadow-blue-950/10 ${widthClassName}`}
    >
      {children}
    </div>
  )
}

function DropdownOption({
  selected,
  icon,
  title,
  description,
  onClick,
}: {
  selected: boolean
  icon: ReactNode
  title: string
  description: string
  onClick: () => void
}) {
  return (
    <button
      className={`flex w-full items-center gap-4 px-5 py-4 text-left transition ${
        selected ? 'bg-blue-50' : 'hover:bg-gray-50'
      }`}
      type="button"
      onClick={onClick}
    >
      <span className={`${selected ? 'text-blue-700' : 'text-gray-500'}`}>{icon}</span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-base font-semibold text-gray-950">{title}</span>
        <span className="mt-0.5 block truncate text-sm text-gray-500">{description}</span>
      </span>
      {selected ? (
        <CheckCircle2 size={18} className="shrink-0 text-blue-600" />
      ) : (
        <span className="h-2.5 w-2.5 shrink-0 rounded-full bg-transparent" />
      )}
    </button>
  )
}

function messageClassName(msg: Message) {
  if (msg.role === 'user') return 'ml-auto max-w-3xl rounded-lg bg-blue-100 p-3'
  if (msg.role === 'assistant') return 'max-w-3xl rounded-lg bg-gray-100 p-3'

  const tones: Record<NonNullable<Message['kind']>, string> = {
    idle: 'bg-gray-50',
    thinking: 'bg-gray-50',
    tool: 'bg-amber-50',
    done: 'bg-gray-50',
    error: 'bg-red-50',
  }
  return `max-w-3xl rounded-lg p-3 ${tones[msg.kind ?? 'idle']}`
}
