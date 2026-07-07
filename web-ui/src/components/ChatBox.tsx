import { useEffect, useId, useMemo, useRef, useState } from 'react'
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
  Edit3,
  FileText,
  FileQuestion,
  SearchCheck,
  MessageSquare,
  Paperclip,
  Sparkles,
  Circle,
  Square,
  X,
} from 'lucide-react'
import type { LlmModelConfig } from '../settings'
import type { QuizSession } from '../quizTypes'
import { useI18n, type TranslationKey } from '../i18n'
import { DeepSolveMessage, type DeepSolveTraceEntry } from './DeepSolveMessage'
import { MarkdownMessage, SourceReferences, sourceTargetFromRaw } from './MarkdownMessage'
import type { SourceReference, SourceTarget } from './MarkdownMessage'

type Capability = 'chat' | 'deep_solve' | 'code_exec' | 'quiz' | 'research' | 'organize'
type OpenMenu = 'mode' | 'knowledge' | 'space' | 'model' | null
type SpaceMentionFilter = 'all' | SpaceMention['type']

interface Message {
  role: 'user' | 'assistant' | 'status'
  text: string
  kind?: 'idle' | 'thinking' | 'tool' | 'done' | 'error'
  citations?: Citation[]
  deepSolve?: DeepSolveTraceEntry[]
  quiz?: QuizSession
  quizPlan?: QuizPlan
  notebookEditProposal?: NotebookEditProposal
  attachments?: ChatAttachment[]
  mentions?: SpaceMention[]
}

interface QuizPlan {
  title: string
  topic: string
  source: string
  difficulty: string
  questionCount: number
  notes: string[]
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

export interface SpaceMention {
  id: string
  type: 'notebook_entry' | 'quiz_session' | 'quiz_question'
  target_id?: string | null
  question_id?: string | null
  title: string
  preview?: string | null
  metadata?: Record<string, unknown>
}

const spaceMentionFilterOptions: Array<{
  value: SpaceMentionFilter
  labelKey: TranslationKey
  icon: ReactNode
}> = [
  { value: 'all', labelKey: 'mention.filter.all', icon: <AtSign size={14} /> },
  { value: 'notebook_entry', labelKey: 'mention.filter.notes', icon: <FileText size={14} /> },
  { value: 'quiz_session', labelKey: 'mention.filter.quizzes', icon: <SearchCheck size={14} /> },
  { value: 'quiz_question', labelKey: 'mention.filter.questions', icon: <FileQuestion size={14} /> },
]

export interface NotebookEditProposal {
  entryId: string
  entryTitle: string
  proposedTitle: string
  proposedMarkdown: string
  summary: string
  proposalKind?: 'edit' | 'links' | 'tags' | 'merge'
  suggestedLinks?: Array<{ text: string; target: string; reason?: string }>
  suggestedTags?: Array<{ tag: string; action: 'add' | 'keep' | 'remove'; reason?: string }>
  mergeSourceEntryIds?: string[]
  applied?: boolean
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
  selectedNotebookEnabled: boolean
  onSend: (text: string, attachments?: ChatAttachment[], mentions?: SpaceMention[]) => void
  onStop?: () => void
  onEditUserMessage?: (messageIndex: number, nextText: string) => void
  onAskDeepSolveStep?: (step: { id: string; title: string; summary?: string }) => void
  onCapabilityChange: (capability: Capability) => void
  onKnowledgeBaseChange: (id: string) => void
  onNotebookEnabledChange: (enabled: boolean) => void
  onLlmConfigChange: (id: string) => void
  onSaveToNotebook?: (markdown: string) => Promise<void>
  onApplyNotebookEdit?: (proposal: NotebookEditProposal) => Promise<void>
  onQuizAnswer?: (quizId: string, questionId: string, selectedOptionId: string) => Promise<void>
  onQuizFinish?: (quizId: string) => Promise<void>
  onSourceNavigate?: (target: SourceTarget, reference: SourceReference) => void
  disabled: boolean
  running?: boolean
}

export interface ContextStats {
  usedTokens: number
  maxTokens: number
  source: 'provider' | 'estimate'
}

const modeOptions: Array<{
  value: Capability
  labelKey: TranslationKey
  descriptionKey: TranslationKey
  icon: ReactNode
}> = [
  {
    value: 'chat',
    labelKey: 'cap.chat',
    descriptionKey: 'cap.chat.description',
    icon: <MessageSquare size={21} />,
  },
  {
    value: 'deep_solve',
    labelKey: 'cap.deepSolve',
    descriptionKey: 'cap.deepSolve.description',
    icon: <Brain size={21} />,
  },
  {
    value: 'code_exec',
    labelKey: 'cap.codeExec',
    descriptionKey: 'cap.codeExec.description',
    icon: <Code2 size={21} />,
  },
  {
    value: 'quiz',
    labelKey: 'cap.quiz',
    descriptionKey: 'cap.quiz.description',
    icon: <FileQuestion size={21} />,
  },
  {
    value: 'research',
    labelKey: 'cap.research',
    descriptionKey: 'cap.research.description',
    icon: <SearchCheck size={21} />,
  },
  {
    value: 'organize',
    labelKey: 'cap.organize',
    descriptionKey: 'cap.organize.description',
    icon: <FileText size={21} />,
  },
]

const visibleModeOptions = modeOptions.filter((mode) => mode.value !== 'code_exec')

export function ChatBox({
  messages,
  streamingText,
  contextStats,
  capability,
  llmConfigs,
  activeLlmConfigId,
  knowledgeBases,
  selectedKnowledgeBaseId,
  selectedNotebookEnabled,
  onSend,
  onStop,
  onEditUserMessage,
  onAskDeepSolveStep,
  onCapabilityChange,
  onKnowledgeBaseChange,
  onNotebookEnabledChange,
  onLlmConfigChange,
  onSaveToNotebook,
  onApplyNotebookEdit,
  onQuizAnswer,
  onQuizFinish,
  onSourceNavigate,
  disabled,
  running = false,
}: Props) {
  const { t } = useI18n()
  const [input, setInput] = useState('')
  const [editingMessageIndex, setEditingMessageIndex] = useState<number | null>(null)
  const [editingMessageText, setEditingMessageText] = useState('')
  const [attachments, setAttachments] = useState<ChatAttachment[]>([])
  const [mentions, setMentions] = useState<SpaceMention[]>([])
  const scrollRef = useRef<HTMLDivElement>(null)
  const shouldStickToBottomRef = useRef(true)
  const empty = messages.length === 0 && !streamingText

  const handleSend = () => {
    const readyAttachments = attachments.filter((attachment) => !attachment.error)
    if ((!input.trim() && readyAttachments.length === 0 && mentions.length === 0) || disabled || running) return
    onSend(input.trim(), readyAttachments, mentions)
    setInput('')
    setAttachments([])
    setMentions([])
  }

  const startEditUserMessage = (index: number, text: string) => {
    if (running) return
    setEditingMessageIndex(index)
    setEditingMessageText(text)
  }

  const cancelEditUserMessage = () => {
    setEditingMessageIndex(null)
    setEditingMessageText('')
  }

  const submitEditUserMessage = () => {
    if (editingMessageIndex === null || !editingMessageText.trim() || !onEditUserMessage || running) return
    onEditUserMessage(editingMessageIndex, editingMessageText.trim())
    cancelEditUserMessage()
  }

  const handleAddAttachments = (items: ChatAttachment[]) => {
    setAttachments((current) => [...current, ...items])
  }

  const handleRemoveAttachment = (id: string) => {
    setAttachments((current) => current.filter((attachment) => attachment.id !== id))
  }

  const handleAddMention = (mention: SpaceMention) => {
    setMentions((current) => current.some((item) => item.id === mention.id) ? current : [...current, mention])
  }

  const handleRemoveMention = (id: string) => {
    setMentions((current) => current.filter((mention) => mention.id !== id))
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
              <h2 className="text-4xl font-semibold text-gray-900">{t('chat.empty.title')}</h2>
            </div>
            <Composer
              input={input}
              setInput={setInput}
              capability={capability}
              llmConfigs={llmConfigs}
              activeLlmConfigId={activeLlmConfigId}
              knowledgeBases={knowledgeBases}
              selectedKnowledgeBaseId={selectedKnowledgeBaseId}
              selectedNotebookEnabled={selectedNotebookEnabled}
              onCapabilityChange={onCapabilityChange}
              onKnowledgeBaseChange={onKnowledgeBaseChange}
              onNotebookEnabledChange={onNotebookEnabledChange}
              onLlmConfigChange={onLlmConfigChange}
              onSend={handleSend}
              onStop={onStop}
              attachments={attachments}
              onAddAttachments={handleAddAttachments}
              onRemoveAttachment={handleRemoveAttachment}
              mentions={mentions}
              onAddMention={handleAddMention}
              onRemoveMention={handleRemoveMention}
              disabled={disabled}
              running={running}
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
                  ) : msg.quizPlan ? (
                    <QuizPlanCard plan={msg.quizPlan} text={msg.text} />
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
                      {msg.notebookEditProposal && onApplyNotebookEdit && (
                        <NotebookEditProposalCard
                          proposal={msg.notebookEditProposal}
                          onApply={onApplyNotebookEdit}
                        />
                      )}
                    </>
                  )
                ) : (
                  <>
                    {editingMessageIndex === i ? (
                      <div className="space-y-2">
                        <textarea
                          className="min-h-24 w-full resize-y rounded-lg border border-blue-200 bg-white px-3 py-2 text-sm text-gray-900 outline-none focus:border-blue-400"
                          value={editingMessageText}
                          onChange={(event) => setEditingMessageText(event.target.value)}
                          autoFocus
                        />
                        <div className="flex justify-end gap-2">
                          <button
                            className="rounded-md border border-gray-200 bg-white px-3 py-1.5 text-xs font-medium text-gray-600 hover:bg-gray-50"
                            type="button"
                            onClick={cancelEditUserMessage}
                          >
                            取消
                          </button>
                          <button
                            className="rounded-md bg-blue-600 px-3 py-1.5 text-xs font-semibold text-white hover:bg-blue-700 disabled:bg-gray-200"
                            type="button"
                            disabled={!editingMessageText.trim()}
                            onClick={submitEditUserMessage}
                          >
                            Regenerate
                          </button>
                        </div>
                      </div>
                    ) : (
                      <div className="group/user-message relative">
                        <pre className="whitespace-pre-wrap pr-8 font-sans text-sm">{msg.text}</pre>
                        {onEditUserMessage && !running && (
                          <button
                            className="absolute right-0 top-0 hidden h-7 w-7 items-center justify-center rounded-md text-blue-700 hover:bg-blue-100 group-hover/user-message:flex"
                            type="button"
                            title="Edit and regenerate"
                            onClick={() => startEditUserMessage(i, msg.text)}
                          >
                            <Edit3 size={15} />
                          </button>
                        )}
                      </div>
                    )}
                    {msg.attachments && msg.attachments.length > 0 && (
                      <AttachmentSummary attachments={msg.attachments} />
                    )}
                    {msg.mentions && msg.mentions.length > 0 && (
                      <MentionSummary mentions={msg.mentions} />
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
              selectedNotebookEnabled={selectedNotebookEnabled}
              onCapabilityChange={onCapabilityChange}
              onKnowledgeBaseChange={onKnowledgeBaseChange}
              onNotebookEnabledChange={onNotebookEnabledChange}
              onLlmConfigChange={onLlmConfigChange}
              onSend={handleSend}
              onStop={onStop}
              attachments={attachments}
              onAddAttachments={handleAddAttachments}
              onRemoveAttachment={handleRemoveAttachment}
              mentions={mentions}
              onAddMention={handleAddMention}
              onRemoveMention={handleRemoveMention}
              disabled={disabled}
              running={running}
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
        <span className="font-medium text-gray-700">Context capacity</span>
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

function NotebookEditProposalCard({
  proposal,
  onApply,
}: {
  proposal: NotebookEditProposal
  onApply: (proposal: NotebookEditProposal) => Promise<void>
}) {
  return (
    <div className="mt-3 overflow-hidden rounded-lg border border-blue-100 bg-white">
      <div className="flex items-start justify-between gap-3 border-b border-blue-50 px-4 py-3">
        <div>
          <div className="text-sm font-semibold text-gray-900">{notebookProposalTitle(proposal)}</div>
          <div className="mt-1 text-xs text-gray-500">{proposal.entryTitle}</div>
        </div>
        {proposal.applied ? (
          <span className="inline-flex items-center gap-1 rounded-full bg-green-50 px-2 py-1 text-xs font-medium text-green-700">
            <CheckCircle2 size={14} />
            Applied
          </span>
        ) : (
          <button
            className="inline-flex h-8 items-center gap-2 rounded-md bg-blue-600 px-3 text-xs font-semibold text-white hover:bg-blue-700"
            type="button"
            onClick={() => {
              void onApply(proposal)
            }}
          >
            <CheckCircle2 size={15} />
            Apply
          </button>
        )}
      </div>
      <div className="space-y-3 px-4 py-3">
        <p className="text-sm text-gray-700">{proposal.summary}</p>
        {proposal.suggestedLinks && proposal.suggestedLinks.length > 0 && (
          <ProposalDetailList
            title="Suggested links"
            items={proposal.suggestedLinks.map((link) =>
              `${link.text} -> [[${link.target}]]${link.reason ? ` 路 ${link.reason}` : ''}`,
            )}
          />
        )}
        {proposal.suggestedTags && proposal.suggestedTags.length > 0 && (
          <ProposalDetailList
            title="Suggested tags"
            items={proposal.suggestedTags.map((tag) =>
              `${tag.action}: #${tag.tag.replace(/^#/, '')}${tag.reason ? ` 路 ${tag.reason}` : ''}`,
            )}
          />
        )}
        {proposal.mergeSourceEntryIds && proposal.mergeSourceEntryIds.length > 0 && (
          <ProposalDetailList
            title="Merge sources"
            items={proposal.mergeSourceEntryIds.map((id) => `Notebook entry ${id}`)}
          />
        )}
        <div className="rounded-md bg-gray-50 px-3 py-2 text-xs text-gray-600">
          <span className="font-medium text-gray-900">New title:</span> {proposal.proposedTitle}
        </div>
        <details className="rounded-md border border-gray-100 bg-gray-50 px-3 py-2">
          <summary className="cursor-pointer text-xs font-medium text-gray-700">Preview Markdown</summary>
          <pre className="mt-2 max-h-72 overflow-auto whitespace-pre-wrap text-xs text-gray-700">{proposal.proposedMarkdown}</pre>
        </details>
      </div>
    </div>
  )
}

function notebookProposalTitle(proposal: NotebookEditProposal) {
  if (proposal.proposalKind === 'links') return 'Notebook link proposal'
  if (proposal.proposalKind === 'tags') return 'Notebook tag proposal'
  if (proposal.proposalKind === 'merge') return 'Notebook merge proposal'
  return 'Notebook edit proposal'
}

function ProposalDetailList({ title, items }: { title: string; items: string[] }) {
  return (
    <div className="rounded-md border border-blue-50 bg-blue-50/40 px-3 py-2">
      <div className="text-xs font-semibold text-blue-900">{title}</div>
      <ul className="mt-1 space-y-1 text-xs leading-5 text-gray-700">
        {items.map((item, index) => (
          <li key={`${title}-${index}`}>{item}</li>
        ))}
      </ul>
    </div>
  )
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
      <div className="mb-2 text-xs font-medium text-gray-500">{hasWeb ? '缃戦〉鏉ユ簮' : '寮曠敤鏉ユ簮'}</div>
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

function QuizPlanCard({ plan, text }: { plan: QuizPlan; text: string }) {
  return (
    <div className="space-y-3 rounded-lg border border-blue-100 bg-white p-4">
      {text.trim() && <MarkdownMessage text={text} />}
      <div className="rounded-lg border border-blue-100 bg-blue-50/40 p-3">
        <div className="flex items-center gap-2 text-sm font-semibold text-blue-800">
          <FileQuestion size={17} />
          Quiz plan
        </div>
        <div className="mt-3 grid gap-2 text-sm text-gray-700 sm:grid-cols-2">
          <div>
            <span className="text-xs font-medium uppercase text-gray-400">Title</span>
            <p className="font-medium text-gray-950">{plan.title}</p>
          </div>
          <div>
            <span className="text-xs font-medium uppercase text-gray-400">Topic</span>
            <p>{plan.topic}</p>
          </div>
          <div>
            <span className="text-xs font-medium uppercase text-gray-400">Source</span>
            <p>{plan.source}</p>
          </div>
          <div>
            <span className="text-xs font-medium uppercase text-gray-400">Settings</span>
            <p>
              {plan.questionCount} questions · {plan.difficulty}
            </p>
          </div>
        </div>
        {plan.notes.length > 0 && (
          <ul className="mt-3 list-disc space-y-1 pl-5 text-sm text-gray-600">
            {plan.notes.map((note, index) => (
              <li key={`${index}:${note}`}>{note}</li>
            ))}
          </ul>
        )}
        <p className="mt-3 text-xs text-gray-500">Reply with confirmation or changes before generating the quiz.</p>
      </div>
    </div>
  )
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
        <p className="mt-3 text-sm text-gray-600">This quiz does not have generated questions yet.</p>
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
          <div className="flex flex-wrap items-center gap-2">
            <h3 className="truncate text-base font-semibold text-gray-950">{quiz.title || 'Quiz'}</h3>
            <span className="rounded-full bg-blue-50 px-2 py-0.5 text-xs font-medium text-blue-700">
              {quiz.status}
            </span>
            {quiz.verification && (
              <span
                className={`rounded-full px-2 py-0.5 text-xs font-medium ${
                  quiz.verification.status === 'verified'
                    ? 'bg-emerald-50 text-emerald-700'
                    : 'bg-amber-50 text-amber-700'
                }`}
                title={quiz.verification.method}
              >
                {quiz.verification.status === 'verified' ? 'Verified' : 'Needs review'}
              </span>
            )}
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
          <div className="font-medium">{answer.correct ? 'Correct' : 'Incorrect'}</div>
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
          Previous
        </button>
        <button
          className="inline-flex h-8 items-center rounded-lg border border-gray-200 px-3 text-xs font-medium text-gray-700 hover:bg-blue-50 disabled:opacity-50"
          type="button"
          disabled={currentIndex >= quiz.questions.length - 1}
          onClick={() => setCurrentIndex((value) => Math.min(quiz.questions.length - 1, value + 1))}
        >
          Next
        </button>
        <button
          className="ml-auto inline-flex h-8 items-center rounded-lg bg-blue-600 px-3 text-xs font-medium text-white hover:bg-blue-700 disabled:bg-gray-200 disabled:text-gray-400"
          type="button"
          disabled={!selectedOptionId || Boolean(answer) || busy}
          onClick={submit}
        >
          Submit answer
        </button>
        <button
          className="inline-flex h-8 items-center rounded-lg border border-gray-200 px-3 text-xs font-medium text-gray-700 hover:bg-blue-50 disabled:opacity-50"
          type="button"
          disabled={busy || quiz.status === 'finished'}
          onClick={finish}
        >
          Finish quiz
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
  selectedNotebookEnabled,
  onCapabilityChange,
  onKnowledgeBaseChange,
  onNotebookEnabledChange,
  onLlmConfigChange,
  onSend,
  onStop,
  attachments,
  onAddAttachments,
  onRemoveAttachment,
  mentions,
  onAddMention,
  onRemoveMention,
  disabled,
  running,
  variant,
}: {
  input: string
  setInput: (value: string) => void
  capability: Capability
  llmConfigs: LlmModelConfig[]
  activeLlmConfigId: string | null
  knowledgeBases: Array<{ id: string; name: string }>
  selectedKnowledgeBaseId: string
  selectedNotebookEnabled: boolean
  onCapabilityChange: (capability: Capability) => void
  onKnowledgeBaseChange: (id: string) => void
  onNotebookEnabledChange: (enabled: boolean) => void
  onLlmConfigChange: (id: string) => void
  onSend: () => void
  onStop?: () => void
  attachments: ChatAttachment[]
  onAddAttachments: (attachments: ChatAttachment[]) => void
  onRemoveAttachment: (id: string) => void
  mentions: SpaceMention[]
  onAddMention: (mention: SpaceMention) => void
  onRemoveMention: (id: string) => void
  disabled: boolean
  running: boolean
  variant: 'center' | 'bottom'
}) {
  const { t } = useI18n()
  const [openMenu, setOpenMenu] = useState<OpenMenu>(null)
  const [readingAttachments, setReadingAttachments] = useState(false)
  const [spaceQuery, setSpaceQuery] = useState('')
  const [spaceMentionFilter, setSpaceMentionFilter] = useState<SpaceMentionFilter>('all')
  const [spaceMentions, setSpaceMentions] = useState<SpaceMention[]>([])
  const [loadingSpaceMentions, setLoadingSpaceMentions] = useState(false)
  const fileInputRef = useRef<HTMLInputElement>(null)
  const activeMode = modeOptions.find((mode) => mode.value === capability) ?? modeOptions[0]!
  const activeKnowledge = selectedNotebookEnabled
    ? { id: '__notebook__', name: 'Notebook' }
    : knowledgeBases.find((item) => item.id === selectedKnowledgeBaseId)
  const activeModel = llmConfigs.find((item) => item.id === activeLlmConfigId) ?? llmConfigs[0] ?? null
  const visibleSpaceMentions = useMemo(
    () => filterSpaceMentions(spaceMentions, spaceMentionFilter),
    [spaceMentions, spaceMentionFilter],
  )
  const knowledgeOptions = [
    {
      id: '',
      name: t('chat.knowledge.none'),
      description: t('chat.knowledge.none.description'),
      icon: <Database size={21} />,
    },
    ...knowledgeBases.map((item) => ({
      id: item.id,
      name: item.name,
      description: t('chat.knowledge.use.description'),
      icon: <Database size={21} />,
    })),
  ]
  const sourceOptions = [
    { ...knowledgeOptions[0]!, type: 'none' as const },
    {
      id: '__notebook__',
      type: 'notebook' as const,
      name: 'Notebook',
      description: t('chat.notebook.description'),
      icon: <FileText size={21} />,
    },
    ...knowledgeOptions.slice(1).map((item) => ({
      ...item,
      type: 'knowledge_base' as const,
    })),
  ]

  const toggleMenu = (menu: OpenMenu) => {
    if (disabled || running) return
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

  useEffect(() => {
    if (openMenu !== 'space') return
    let cancelled = false
    const controller = new AbortController()
    setLoadingSpaceMentions(true)
    const timer = window.setTimeout(() => {
      const params = new URLSearchParams()
      if (spaceQuery.trim()) params.set('q', spaceQuery.trim())
      if (spaceMentionFilter !== 'all') params.set('type', spaceMentionFilter)
      params.set('limit', '50')
      fetch(`/api/space/mentions?${params.toString()}`, { signal: controller.signal })
        .then(async (res) => {
          const data = await res.json().catch(() => ({})) as { mentions?: SpaceMention[] }
          if (!res.ok) throw new Error(`HTTP ${res.status}`)
          if (!cancelled) setSpaceMentions(data.mentions ?? [])
        })
        .catch(() => {
          if (!cancelled) setSpaceMentions([])
        })
        .finally(() => {
          if (!cancelled) setLoadingSpaceMentions(false)
        })
    }, 160)

    return () => {
      cancelled = true
      controller.abort()
      window.clearTimeout(timer)
    }
  }, [openMenu, spaceQuery, spaceMentionFilter])

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
        placeholder={t('chat.input.placeholder')}
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
      {mentions.length > 0 && (
        <div className="border-t border-blue-50 px-4 py-2">
          <MentionSummary mentions={mentions} removable onRemove={onRemoveMention} />
        </div>
      )}
      <div className="relative flex flex-wrap items-center gap-2 border-t border-blue-50 px-4 py-2">
        <div className="relative">
          <ToolbarButton
            active={openMenu === 'mode'}
            icon={activeMode.icon}
            label={t(activeMode.labelKey)}
            onClick={() => toggleMenu('mode')}
          />
          {openMenu === 'mode' && (
            <DropdownPanel widthClassName="w-[33rem]">
              {visibleModeOptions.map((mode) => (
                <DropdownOption
                  key={mode.value}
                  selected={mode.value === capability}
                  icon={mode.icon}
                  title={t(mode.labelKey)}
                  description={t(mode.descriptionKey)}
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
          disabled={disabled || running || readingAttachments}
          onClick={() => fileInputRef.current?.click()}
        >
          <Paperclip size={18} />
          {t('chat.attachments')}
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
            label={activeKnowledge?.name ?? t('chat.knowledge.none')}
            onClick={() => toggleMenu('knowledge')}
          />
          {openMenu === 'knowledge' && (
            <DropdownPanel widthClassName="w-[28rem]">
              {sourceOptions.map((item) => (
                <DropdownOption
                  key={item.id || 'none'}
                  selected={
                    item.type === 'notebook'
                      ? selectedNotebookEnabled
                      : item.type === 'none'
                        ? !selectedNotebookEnabled && !selectedKnowledgeBaseId
                        : !selectedNotebookEnabled && item.id === selectedKnowledgeBaseId
                  }
                  icon={item.icon}
                  title={item.name}
                  description={item.description}
                  onClick={() => {
                    if (item.type === 'notebook') {
                      onNotebookEnabledChange(true)
                    } else {
                      onKnowledgeBaseChange(item.id)
                    }
                    setOpenMenu(null)
                  }}
                />
              ))}
            </DropdownPanel>
          )}
        </div>

        <div className="relative">
          <ToolbarButton
            active={openMenu === 'space'}
            icon={<AtSign size={18} />}
            label={mentions.length > 0 ? `${t('nav.space')} ${mentions.length}` : t('nav.space')}
            onClick={() => toggleMenu('space')}
          />
          {openMenu === 'space' && (
            <DropdownPanel
              widthClassName="w-[30rem] max-w-[calc(100vw-2rem)]"
              className="flex max-h-[min(28rem,calc(100vh-10rem))] flex-col"
            >
              <div className="shrink-0 space-y-2 border-b border-blue-50 bg-white px-4 pb-2 pt-1">
                <div className="flex rounded-xl bg-gray-50 p-1">
                  {spaceMentionFilterOptions.map((option) => (
                    <button
                      key={option.value}
                      className={`flex h-8 flex-1 items-center justify-center gap-1.5 rounded-lg text-xs font-medium transition ${
                        spaceMentionFilter === option.value
                          ? 'bg-white text-blue-700 shadow-sm'
                          : 'text-gray-500 hover:text-gray-900'
                      }`}
                      type="button"
                      onClick={() => setSpaceMentionFilter(option.value)}
                    >
                      {option.icon}
                      {t(option.labelKey)}
                    </button>
                  ))}
                </div>
                <input
                  className="h-9 w-full rounded-xl border border-blue-100 px-3 text-sm outline-none focus:border-blue-300"
                  value={spaceQuery}
                  onChange={(event) => setSpaceQuery(event.target.value)}
                  placeholder={t('chat.space.searchPlaceholder')}
                  autoFocus
                />
                {loadingSpaceMentions && (
                  <div className="px-1 text-xs text-blue-500">{t('chat.space.updating')}</div>
                )}
              </div>
              <div className="min-h-0 overflow-y-auto py-2">
                {visibleSpaceMentions.length === 0 ? (
                  <div className="px-5 py-4 text-sm text-gray-500">
                    {t('chat.space.noMatching')}
                  </div>
                ) : (
                  visibleSpaceMentions.map((mention) => (
                    <DropdownOption
                      key={mention.id}
                      selected={mentions.some((item) => item.id === mention.id)}
                      icon={spaceMentionIcon(mention)}
                      title={mention.title}
                      description={spaceMentionDescription(mention)}
                      onClick={() => {
                        onAddMention(mention)
                        setOpenMenu(null)
                      }}
                    />
                  ))
                )}
              </div>
            </DropdownPanel>
          )}
        </div>

        <div className="relative ml-auto">
          <ToolbarButton
            active={openMenu === 'model'}
            icon={<Brain size={16} />}
            label={activeModel?.model ?? t('chat.model.select')}
            onClick={() => toggleMenu('model')}
          />
          {openMenu === 'model' && (
            <DropdownPanel widthClassName="right-0 left-auto w-[30rem]">
              {llmConfigs.length === 0 ? (
                <DropdownOption
                  selected
                  icon={<Brain size={21} />}
                  title={t('chat.model.none')}
                  description={t('chat.model.configureFirst')}
                  onClick={() => setOpenMenu(null)}
                />
              ) : (
                llmConfigs.map((config) => (
                  <DropdownOption
                    key={config.id}
                    selected={config.id === activeModel?.id}
                    icon={<Brain size={21} />}
                    title={config.name || config.model}
                    description={`${llmApiModeLabel(config.provider)} 路 ${config.model}`}
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
          className={`flex h-9 w-9 items-center justify-center rounded-full text-white disabled:bg-gray-200 disabled:text-gray-400 ${
            running ? 'bg-gray-900 hover:bg-gray-800' : 'bg-blue-600 hover:bg-blue-700'
          }`}
          onClick={running ? onStop : onSend}
          disabled={disabled || (!running && !input.trim() && attachments.filter((attachment) => !attachment.error).length === 0 && mentions.length === 0)}
          type="button"
          title={running ? t('chat.stop') : t('chat.send')}
        >
          {running ? <Square size={15} /> : <ArrowUp size={20} />}
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
      error: `Attachment exceeds ${formatBytes(MAX_ATTACHMENT_BYTES)}. Split it before sending.`,
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
    return { ...base, error: `Read failed: ${message}` }
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
      return { ...base, error: data.error || `Attachment parse failed: HTTP ${res.status}` }
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
    return { ...base, error: `Attachment parse request failed: ${message}` }
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
              title="Remove attachment"
            >
              <X size={14} />
            </button>
          )}
        </div>
      ))}
    </div>
  )
}

function MentionSummary({
  mentions,
  removable = false,
  onRemove,
}: {
  mentions: SpaceMention[]
  removable?: boolean
  onRemove?: (id: string) => void
}) {
  return (
    <div className="flex flex-wrap gap-2">
      {mentions.map((mention) => (
        <div
          key={mention.id}
          className="flex max-w-full items-center gap-2 rounded-xl border border-blue-100 bg-white px-3 py-2 text-xs text-gray-700"
          title={mention.preview || mention.title}
        >
          <span className="shrink-0 text-blue-600">{spaceMentionIcon(mention, 16)}</span>
          <span className="shrink-0 rounded-full bg-blue-50 px-2 py-0.5 font-medium text-blue-700">
            {spaceMentionTypeLabel(mention)}
          </span>
          <span className="min-w-0 truncate font-medium">{mention.title}</span>
          {removable && (
            <button
              className="ml-1 flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-gray-500 hover:bg-blue-50 hover:text-gray-900"
              type="button"
              onClick={() => onRemove?.(mention.id)}
              title="Remove Space reference"
            >
              <X size={14} />
            </button>
          )}
        </div>
      ))}
    </div>
  )
}

function spaceMentionIcon(mention: SpaceMention, size = 21) {
  if (mention.type === 'notebook_entry') return <FileText size={size} />
  return <FileQuestion size={size} />
}

function spaceMentionTypeLabel(mention: SpaceMention) {
  if (mention.type === 'notebook_entry') return 'Note'
  if (mention.type === 'quiz_question') return 'Question'
  return 'Quiz'
}

function filterSpaceMentions(mentions: SpaceMention[], filter: SpaceMentionFilter) {
  if (filter === 'all') return mentions
  return mentions.filter((mention) => mention.type === filter)
}

function spaceMentionDescription(mention: SpaceMention) {
  return [spaceMentionTypeLabel(mention), mention.preview].filter(Boolean).join(' - ')
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

function DropdownPanel({
  children,
  widthClassName,
  className = '',
}: {
  children: ReactNode
  widthClassName: string
  className?: string
}) {
  return (
    <div
      className={`absolute bottom-12 left-0 z-30 overflow-hidden rounded-2xl border border-blue-100 bg-white shadow-2xl shadow-blue-950/10 ${widthClassName} ${className || 'py-2'}`}
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

