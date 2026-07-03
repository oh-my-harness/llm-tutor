import { useState, useCallback, useEffect, useMemo, useRef } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import { ChatBox } from './components/ChatBox'
import type { ChatAttachment, ContextStats, NotebookEditProposal, SpaceMention } from './components/ChatBox'
import { TracePanel, TraceEntry } from './components/TracePanel'
import { BudgetPanel } from './components/BudgetPanel'
import { ApprovalDialog } from './components/ApprovalDialog'
import { SettingsPage } from './components/SettingsPage'
import { KnowledgePage } from './components/KnowledgePage'
import { SpacePage } from './components/SpacePage'
import { MemoryPage } from './components/MemoryPage'
import { PlaceholderPage } from './components/PlaceholderPage'
import { AppView, Sidebar } from './components/Sidebar'
import type { DeepSolveTraceEntry } from './components/DeepSolveMessage'
import type { SourceReference, SourceTarget } from './components/MarkdownMessage'
import { AgentStatus } from './agentStatus'
import { useWebSocket } from './hooks/useWebSocket'
import {
  DEFAULT_CONTEXT_WINDOW_TOKENS,
  activeLlmConfig,
  hasLocalLlmSettings,
  loadLlmSettings,
  loadStoredLlmSettings,
  saveLlmSettings,
  saveStoredLlmSettings,
  searchForSession,
  settingsForSession,
} from './settings'
import type { QuizSession } from './quizTypes'
import { I18nProvider, translate, type TranslationKey } from './i18n'

type Capability = 'chat' | 'deep_solve' | 'code_exec' | 'quiz' | 'research' | 'organize'

interface Message {
  role: 'user' | 'assistant' | 'status'
  text: string
  kind?: AgentStatus['kind']
  transient?: boolean
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

interface RecentSession {
  id: string
  title: string
}

interface KnowledgeBaseOption {
  id: string
  name: string
}

interface SessionListResponse {
  sessions?: Array<{
    id: string
    title?: string
    name?: string | null
  }>
}

interface SessionDetailResponse {
  capability?: Capability
  kb?: string | null
  notebook_enabled?: boolean
  messages?: Array<{
    role: 'user' | 'assistant'
    text: string
    mentions?: SpaceMention[]
    citations?: Citation[]
  }>
  trace?: Array<{
    kind: string
    timestamp?: string
    payload?: Record<string, unknown>
  }>
  compact_summary?: {
    summary: string
    timestamp?: string
    message_count?: number
  } | null
  latest_usage?: TokenUsagePayload | null
  metadata?: {
    name?: string | null
  }
}

interface TokenUsagePayload {
  input_tokens?: number
  output_tokens?: number
  cache_read_tokens?: number
  cache_creation_tokens?: number
  total_tokens?: number
  source?: string
}

export default function App() {
  const [view, setView] = useState<AppView>('chat')
  const [capability, setCapability] = useState<Capability>('chat')
  const [llmSettings, setLlmSettings] = useState(loadLlmSettings)
  const [sessionId, setSessionId] = useState<string | null>(null)
  const [messages, setMessages] = useState<Message[]>([])
  const [streamingText, setStreamingText] = useState('')
  const streamingRef = useRef('')
  const pendingSessionSendRef = useRef<{ sessionId: string; content: string; mentions: SpaceMention[] } | null>(null)
  const [traceEntries, setTraceEntries] = useState<TraceEntry[]>([])
  const pendingCitationsRef = useRef<Citation[]>([])
  const pendingDeepSolveRef = useRef<DeepSolveTraceEntry[]>([])
  const pendingNotebookEditProposalRef = useRef<NotebookEditProposal | undefined>(undefined)
  const pendingQuizRef = useRef<QuizSession | undefined>(undefined)
  const pendingQuizPlanRef = useRef<QuizPlan | undefined>(undefined)
  const [budgetSpent, setBudgetSpent] = useState(0)
  const [budgetWarning, setBudgetWarning] = useState(false)
  const [pendingApproval, setPendingApproval] = useState<{ tool: string; args: Record<string, unknown>; requestId: string } | null>(null)
  const [running, setRunning] = useState(false)
  const [recentSessions, setRecentSessions] = useState<RecentSession[]>([])
  const [knowledgeBases, setKnowledgeBases] = useState<KnowledgeBaseOption[]>([])
  const [selectedKnowledgeBaseId, setSelectedKnowledgeBaseId] = useState<string>('')
  const [selectedNotebookEnabled, setSelectedNotebookEnabled] = useState(false)
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)
  const [traceCollapsed, setTraceCollapsed] = useState(true)
  const [spaceFocusTarget, setSpaceFocusTarget] = useState<Extract<SourceTarget, { type: 'notebook' | 'quiz' | 'research' }> | null>(null)
  const [knowledgeFocusTarget, setKnowledgeFocusTarget] = useState<Extract<SourceTarget, { type: 'kb' }> | null>(null)
  const [latestUsage, setLatestUsage] = useState<TokenUsagePayload | null>(null)
  const contextStats = useMemo<ContextStats>(() => {
    const config = activeLlmConfig(llmSettings)
    const providerInputTokens = typeof latestUsage?.input_tokens === 'number' ? latestUsage.input_tokens : null
    return {
      usedTokens: providerInputTokens ?? estimateContextTokens(messages, streamingText),
      maxTokens: config?.contextWindowTokens || DEFAULT_CONTEXT_WINDOW_TOKENS,
      source: providerInputTokens === null ? 'estimate' : 'provider',
    }
  }, [latestUsage, llmSettings, messages, streamingText])

  const pushStatus = useCallback((status: AgentStatus) => {
    if (status.kind === 'idle') return

    const text = status.detail ? `${status.label}: ${status.detail}` : status.label
    setMessages((prev) => {
      const last = prev[prev.length - 1]
      const transient = status.kind === 'thinking' || status.kind === 'tool'
      if (last?.role === 'status' && last.text === text && last.kind === status.kind) {
        return prev
      }
      if (
        last?.role === 'status' &&
        last.transient &&
        transient
      ) {
        return [
          ...prev.slice(0, -1),
          { role: 'status', text, kind: status.kind, transient },
        ]
      }
      return [...prev, { role: 'status', text, kind: status.kind, transient }]
    })
  }, [])

  const { send } = useWebSocket(sessionId, {
    onEvent: (event) => {
      if (event.type === 'content') {
        if (event.payload.chunk) {
          streamingRef.current += event.payload.text
          setStreamingText(streamingRef.current)
        } else {
          const finalText = streamingRef.current + event.payload.text
          const citations = pendingCitationsRef.current
          const deepSolve = pendingDeepSolveRef.current
          const notebookEditProposal = pendingNotebookEditProposalRef.current
          const quiz = pendingQuizRef.current
          const quizPlan = pendingQuizPlanRef.current
          if (finalText.trim() || citations.length > 0 || deepSolve.length > 0 || notebookEditProposal || quiz || quizPlan) {
            setMessages((prev) => [
              ...dropTrailingTransientStatus(prev),
              {
                role: 'assistant',
                text: finalText || (quiz ? `Quiz "${quiz.title}" is ready.` : ''),
                citations,
                deepSolve: deepSolve.length > 0 ? deepSolve : undefined,
                notebookEditProposal,
                quiz,
                quizPlan,
              },
            ])
          } else {
            setMessages((prev) => dropTrailingTransientStatus(prev))
          }
          pendingCitationsRef.current = []
          pendingDeepSolveRef.current = []
          pendingNotebookEditProposalRef.current = undefined
          pendingQuizRef.current = undefined
          pendingQuizPlanRef.current = undefined
          streamingRef.current = ''
          setStreamingText('')
          setRunning(false)
          if (sessionId && citations.length > 0) {
            void persistMessageCitations(sessionId, citations).catch((err) => {
              console.warn('failed to persist message citations', err)
            })
          }
          void refreshSessions()
        }
      } else if (event.type === 'trace') {
        const citations = citationsFromTrace(event.payload as Record<string, unknown>)
        if (citations.length > 0) {
          pendingCitationsRef.current = mergeCitations(pendingCitationsRef.current, citations)
        }
        const deepSolveEvent = deepSolveEventFromTrace(event.payload as Record<string, unknown>)
        if (deepSolveEvent) {
          pendingDeepSolveRef.current = [...pendingDeepSolveRef.current, deepSolveEvent]
        }
        const notebookEditProposal = notebookEditProposalFromTrace(event.payload as Record<string, unknown>)
        if (notebookEditProposal) {
          pendingNotebookEditProposalRef.current = notebookEditProposal
        }
        const quiz = quizFromTrace(event.payload as Record<string, unknown>)
        if (quiz) {
          pendingQuizRef.current = quiz
        }
        const quizPlan = quizPlanFromTrace(event.payload as Record<string, unknown>)
        if (quizPlan) {
          pendingQuizPlanRef.current = quizPlan
        }
        pushStatus(statusFromTrace(event.payload as Record<string, unknown>))
        setTraceEntries((prev) => [
          ...prev,
          { kind: event.payload.kind, payload: event.payload, timestamp: Date.now() },
        ])
      } else if (event.type === 'status') {
        const payload = event.payload as Record<string, unknown>
        const kind = payload.kind as string
        if (kind === 'budget_warning') {
          setBudgetWarning(true)
          setBudgetSpent((payload.spent_usd as number) ?? 0)
        } else if (kind === 'running') {
          pushStatus({
            kind: 'thinking',
            label: 'Working',
            detail: typeof payload.capability === 'string' ? capabilityLabel(payload.capability) : undefined,
          })
        } else if (kind === 'done') {
          setLatestUsage(isTokenUsagePayload(payload.usage) ? payload.usage : null)
          if (!streamingRef.current) {
            pushStatus({
              kind: 'done',
              label: 'Done',
              detail: typeof payload.history_len === 'number' ? `${payload.history_len} context messages` : undefined,
            })
          }
        } else if (kind === 'stopped') {
          pushStatus({
            kind: 'done',
            label: 'Stopped',
            detail: typeof payload.capability === 'string' ? capabilityLabel(payload.capability) : undefined,
          })
          setRunning(false)
        } else if (kind === 'approval_request') {
          pushStatus({
            kind: 'tool',
            label: 'Waiting for approval',
            detail: typeof payload.tool === 'string' ? payload.tool : undefined,
          })
          setPendingApproval({
            tool: payload.tool as string,
            args: payload.args as Record<string, unknown>,
            requestId: payload.request_id as string,
          })
        } else if (kind === 'error') {
          const message = typeof payload.message === 'string' ? payload.message : 'WebSocket error'
          pushStatus({ kind: 'error', label: 'Error', detail: message })
          setRunning(false)
        }
      }
    },
    onClose: () => {
      setRunning(false)
      pushStatus({ kind: 'idle', label: 'Disconnected', detail: 'WebSocket closed' })
    },
    onError: () => {
      pushStatus({ kind: 'error', label: 'Connection failed', detail: 'Check tutor-web server' })
      setMessages((prev) => [
        ...prev,
        { role: 'assistant', text: 'Error: WebSocket connection failed. Check that tutor-web is running on 127.0.0.1:8080.' },
      ])
      setRunning(false)
    },
  })

  const refreshSessions = useCallback(async () => {
    const res = await fetch('/api/sessions')
    if (!res.ok) {
      throw new Error(`failed to load sessions: HTTP ${res.status}`)
    }
    const data = await res.json() as SessionListResponse
    setRecentSessions((data.sessions ?? []).map((session) => ({
      id: session.id,
      title: session.title || session.name || 'New session',
    })))
  }, [])

  const refreshKnowledgeBases = useCallback(async () => {
    const res = await fetch('/api/knowledge-bases')
    if (!res.ok) {
      throw new Error(`failed to load knowledge bases: HTTP ${res.status}`)
    }
    const data = await res.json() as { knowledge_bases?: KnowledgeBaseOption[] }
    const items = data.knowledge_bases ?? []
    setKnowledgeBases(items.map((item) => ({ id: item.id, name: item.name })))
    setSelectedKnowledgeBaseId((current) => {
      if (current && items.some((item) => item.id === current)) return current
      return ''
    })
  }, [])

  useEffect(() => {
    const pending = pendingSessionSendRef.current
    if (!pending || pending.sessionId !== sessionId) return
    pendingSessionSendRef.current = null
    send({ type: 'message', content: pending.content, mentions: pending.mentions })
    void refreshSessions()
  }, [sessionId, send, refreshSessions])

  const persistSettings = useCallback((nextSettings: typeof llmSettings) => {
    saveLlmSettings(nextSettings)
    saveStoredLlmSettings(nextSettings).catch((err) => {
      const message = err instanceof Error ? err.message : String(err)
      pushStatus({ kind: 'error', label: 'Settings not saved', detail: message })
    })
  }, [pushStatus])

  useEffect(() => {
    let cancelled = false
    loadStoredLlmSettings()
      .then((storedSettings) => {
        if (cancelled) return
        if (storedSettings) {
          setLlmSettings(storedSettings)
          saveLlmSettings(storedSettings)
        } else if (hasLocalLlmSettings()) {
          const localSettings = loadLlmSettings()
          void saveStoredLlmSettings(localSettings)
        }
      })
      .catch((err) => {
        if (cancelled) return
        const message = err instanceof Error ? err.message : String(err)
        pushStatus({ kind: 'error', label: 'Settings load failed', detail: message })
      })
    return () => {
      cancelled = true
    }
  }, [pushStatus])

  useEffect(() => {
    refreshSessions().catch((err) => {
      const message = err instanceof Error ? err.message : String(err)
      pushStatus({ kind: 'error', label: 'Error', detail: message })
    })
    refreshKnowledgeBases().catch((err) => {
      const message = err instanceof Error ? err.message : String(err)
      pushStatus({ kind: 'error', label: 'Error', detail: message })
    })
  }, [refreshSessions, refreshKnowledgeBases, pushStatus])

  const handleSend = useCallback(async (text: string, attachments: ChatAttachment[] = [], mentions: SpaceMention[] = []) => {
    try {
      const content = buildMessageContentWithAttachments(text, attachments)
      const displayText = text.trim() || (attachments.length > 0 ? `Sent ${attachments.length} attachment(s)` : `Referenced ${mentions.length} Space item(s)`)
      let sid = sessionId
      if (!sid) {
        const kb = selectedKnowledgeBaseId || null
        const res = await fetch('/api/sessions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            capability,
            kb,
            notebook_enabled: selectedNotebookEnabled,
            llm: settingsForSession(llmSettings),
            search: searchForSession(llmSettings),
          }),
        })
        if (!res.ok) {
          throw new Error(`failed to create session: HTTP ${res.status}`)
        }
        const data = await res.json()
        const createdSessionId = data.id as string
        sid = createdSessionId
        setSessionId(createdSessionId)
        upsertRecentSession(setRecentSessions, createdSessionId, sessionTitleFromMessage(displayText))
      }

      setMessages((prev) => [...prev, { role: 'user', text: displayText, attachments, mentions }])
      setRunning(true)
      pushStatus({ kind: 'thinking', label: 'Thinking', detail: capabilityLabel(capability) })
      send({ type: 'message', content, mentions })
      void refreshSessions()
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      pushStatus({ kind: 'error', label: 'Error', detail: message })
      setMessages((prev) => [...prev, { role: 'assistant', text: `Error: ${message}` }])
      setRunning(false)
    }
  }, [sessionId, capability, llmSettings, selectedKnowledgeBaseId, selectedNotebookEnabled, send, pushStatus, refreshSessions, messages])

  const handleStopGeneration = useCallback(() => {
    if (!running) return
    send({ type: 'stop' })
    pushStatus({ kind: 'tool', label: 'Stopping', detail: capabilityLabel(capability) })
  }, [capability, pushStatus, running, send])

  const handleEditUserMessage = useCallback(async (messageIndex: number, nextText: string) => {
    if (running || !nextText.trim()) return
    try {
      const priorMessages = messages
        .slice(0, messageIndex)
        .filter((message) => message.role === 'user' || message.role === 'assistant')
      if (sessionId) {
        const forkRes = await fetch(`/api/sessions/${encodeURIComponent(sessionId)}/fork-before-message`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            message_index: messageIndex,
            label: 'edited user message',
          }),
        })
        const forkData = await safeJson(forkRes)
        if (!forkRes.ok) {
          throw new Error(errorMessage(forkData, forkRes.status))
        }
        if (forkData.forked === true) {
          setMessages([...priorMessages, { role: 'user', text: nextText }])
          setTraceEntries([])
          setLatestUsage(null)
          setStreamingText('')
          streamingRef.current = ''
          pendingCitationsRef.current = []
          pendingDeepSolveRef.current = []
          pendingNotebookEditProposalRef.current = undefined
          setRunning(true)
          pushStatus({ kind: 'thinking', label: 'Thinking', detail: capabilityLabel(capability) })
          send({ type: 'message', content: nextText, mentions: [] })
          void refreshSessions()
          return
        }
      }

      const res = await fetch('/api/sessions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          capability,
          kb: selectedKnowledgeBaseId || null,
          notebook_enabled: selectedNotebookEnabled,
          llm: settingsForSession(llmSettings),
          search: searchForSession(llmSettings),
        }),
      })
      const data = await safeJson(res)
      if (!res.ok) {
        throw new Error(errorMessage(data, res.status))
      }
      const nextSessionId = data.id as string

      setSessionId(nextSessionId)
      upsertRecentSession(setRecentSessions, nextSessionId, sessionTitleFromMessage(nextText))
      setMessages([{ role: 'user', text: nextText }])
      setTraceEntries([])
      setLatestUsage(null)
      setStreamingText('')
      streamingRef.current = ''
      pendingCitationsRef.current = []
      pendingDeepSolveRef.current = []
      pendingNotebookEditProposalRef.current = undefined
      pendingQuizRef.current = undefined
      pendingQuizPlanRef.current = undefined
      setRunning(true)
      pushStatus({ kind: 'thinking', label: 'Thinking', detail: capabilityLabel(capability) })
      pendingSessionSendRef.current = { sessionId: nextSessionId, content: nextText, mentions: [] }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      pushStatus({ kind: 'error', label: 'Error', detail: message })
      setMessages((prev) => [...prev, { role: 'assistant', text: `Error: ${message}` }])
      setRunning(false)
    }
  }, [capability, llmSettings, messages, pushStatus, refreshSessions, running, selectedKnowledgeBaseId, selectedNotebookEnabled, send, sessionId])

  const updateQuizInMessages = useCallback((quiz: QuizSession) => {
    setMessages((prev) =>
      prev.map((message) => {
        if (message.quiz?.id !== quiz.id) return message
        return { ...message, quiz }
      }),
    )
  }, [])

  const handleQuizAnswer = useCallback(async (quizId: string, questionId: string, selectedOptionId: string) => {
    const res = await fetch(`/api/quizzes/${encodeURIComponent(quizId)}/answers`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        question_id: questionId,
        selected_option_id: selectedOptionId,
      }),
    })
    const data = await safeJson(res)
    if (!res.ok) {
      throw new Error(errorMessage(data, res.status))
    }
    updateQuizInMessages(data.quiz as QuizSession)
  }, [updateQuizInMessages])

  const handleQuizFinish = useCallback(async (quizId: string) => {
    const res = await fetch(`/api/quizzes/${encodeURIComponent(quizId)}/finish`, { method: 'POST' })
    const data = await safeJson(res)
    if (!res.ok) {
      throw new Error(errorMessage(data, res.status))
    }
    updateQuizInMessages(data.quiz as QuizSession)
  }, [updateQuizInMessages])

  const handleSaveToNotebook = useCallback(async (markdown: string) => {
    try {
      const title = titleFromMarkdown(markdown)
      const res = await fetch('/api/notebook/entries', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          space_id: 'default',
          entry_type: 'research_report',
          title,
          markdown,
          metadata: {
            generatedBy: 'research',
          },
          source_session_id: sessionId,
        }),
      })
      const data = await safeJson(res)
      if (!res.ok) {
        throw new Error(errorMessage(data, res.status))
      }
      pushStatus({ kind: 'done', label: 'Saved', detail: `Notebook: ${title}` })
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      pushStatus({ kind: 'error', label: 'Save failed', detail: message })
    }
  }, [pushStatus, sessionId])

  const handleApplyNotebookEdit = useCallback(async (proposal: NotebookEditProposal) => {
    try {
      const res = await fetch(`/api/notebook/entries/${encodeURIComponent(proposal.entryId)}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          title: proposal.proposedTitle,
          markdown: proposal.proposedMarkdown,
          metadata: {
            updated_by: 'agent_proposal',
            proposal_kind: proposal.proposalKind ?? 'edit',
            proposal_summary: proposal.summary,
            suggested_links: proposal.suggestedLinks ?? [],
            suggested_tags: proposal.suggestedTags ?? [],
            merge_source_entry_ids: proposal.mergeSourceEntryIds ?? [],
            source_session_id: sessionId,
          },
        }),
      })
      const data = await safeJson(res)
      if (!res.ok) {
        throw new Error(errorMessage(data, res.status))
      }
      setMessages((prev) => prev.map((message) => {
        if (message.notebookEditProposal?.entryId !== proposal.entryId) return message
        return {
          ...message,
          notebookEditProposal: {
            ...message.notebookEditProposal,
            applied: true,
          },
        }
      }))
      pushStatus({ kind: 'done', label: 'Notebook updated', detail: proposal.proposedTitle })
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      pushStatus({ kind: 'error', label: 'Notebook update failed', detail: message })
    }
  }, [pushStatus, sessionId])

  const handleAskDeepSolveStep = useCallback((step: { id: string; title: string; summary?: string }) => {
    const prompt = [
      `Please explain Deep Solve step ${step.id}: ${step.title}.`,
      step.summary ? `Step summary: ${step.summary}` : '',
      'Focus only on this step, clarify why it works, and connect it back to the original problem.',
    ]
      .filter(Boolean)
      .join('\n\n')
    void handleSend(prompt)
  }, [handleSend])

  const handleSettingsChange = (nextSettings: typeof llmSettings) => {
    setLlmSettings(nextSettings)
    persistSettings(nextSettings)
    setSessionId(null)
  }

  const startNewChat = useCallback(() => {
    setSessionId(null)
    setMessages([])
    setStreamingText('')
    streamingRef.current = ''
    setTraceEntries([])
    pendingCitationsRef.current = []
    pendingDeepSolveRef.current = []
    pendingNotebookEditProposalRef.current = undefined
    pendingQuizRef.current = undefined
    pendingQuizPlanRef.current = undefined
    setLatestUsage(null)
    setBudgetWarning(false)
    setRunning(false)
    setView('chat')
  }, [])

  const handleNavigate = useCallback((nextView: AppView) => {
    if (nextView === 'chat') {
      startNewChat()
      return
    }
    setView(nextView)
  }, [startNewChat])

  const handleCapabilityChange = useCallback(async (nextCapability: Capability) => {
    if (running) return

    setCapability(nextCapability)
    if (nextCapability === 'organize') {
      setSelectedNotebookEnabled(true)
      setSelectedKnowledgeBaseId('')
    }
    if (!sessionId) return

    try {
      const body = nextCapability === 'organize'
        ? { capability: nextCapability, notebook_enabled: true, kb: '' }
        : { capability: nextCapability }
      const res = await fetch(`/api/sessions/${sessionId}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      })
      if (!res.ok) {
        throw new Error(`failed to update session mode: HTTP ${res.status}`)
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      setMessages((prev) => [...prev, { role: 'assistant', text: `Error: ${message}` }])
    }
  }, [running, sessionId])

  const handleKnowledgeBaseChange = useCallback(async (nextKb: string) => {
    if (running) return
    setSelectedKnowledgeBaseId(nextKb)
    setSelectedNotebookEnabled(false)
    if (!sessionId) return

    try {
      const res = await fetch(`/api/sessions/${sessionId}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ kb: nextKb, notebook_enabled: false }),
      })
      if (!res.ok) {
        throw new Error(`failed to update session knowledge base: HTTP ${res.status}`)
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      setMessages((prev) => [...prev, { role: 'assistant', text: `Error: ${message}` }])
    }
  }, [running, sessionId])

  const handleNotebookEnabledChange = useCallback(async (enabled: boolean) => {
    if (running) return
    setSelectedNotebookEnabled(enabled)
    if (enabled) setSelectedKnowledgeBaseId('')
    if (!sessionId) return

    try {
      const res = await fetch(`/api/sessions/${sessionId}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ notebook_enabled: enabled, kb: enabled ? '' : undefined }),
      })
      if (!res.ok) {
        throw new Error(`failed to update session source: HTTP ${res.status}`)
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      setMessages((prev) => [...prev, { role: 'assistant', text: `Error: ${message}` }])
    }
  }, [running, sessionId])

  const handleLlmConfigChange = useCallback(async (id: string) => {
    if (running) return
    const nextSettings = { ...llmSettings, activeLlmConfigId: id }
    setLlmSettings(nextSettings)
    persistSettings(nextSettings)
    if (!sessionId) return

    try {
      const res = await fetch(`/api/sessions/${sessionId}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ llm: settingsForSession(nextSettings) }),
      })
      if (!res.ok) {
        throw new Error(`failed to update session model: HTTP ${res.status}`)
      }
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      setMessages((prev) => [...prev, { role: 'assistant', text: `Error: ${message}` }])
    }
  }, [llmSettings, persistSettings, running, sessionId])

  const handleApproval = (requestId: string, approved: boolean) => {
    send({ type: 'approval_response', request_id: requestId, approved })
    setPendingApproval(null)
    pushStatus({
      kind: 'thinking',
      label: approved ? 'Approval sent' : 'Tool denied',
      detail: 'Waiting for agent to continue',
    })
  }

  const handleSelectSession = async (id: string) => {
    if (id !== sessionId) {
      setMessages([])
      setStreamingText('')
      streamingRef.current = ''
      setTraceEntries([])
      pendingCitationsRef.current = []
      pendingDeepSolveRef.current = []
      pendingNotebookEditProposalRef.current = undefined
      pendingQuizRef.current = undefined
      pendingQuizPlanRef.current = undefined
      setLatestUsage(null)
      setSessionId(id)
      try {
        const res = await fetch(`/api/sessions/${id}`)
        if (!res.ok) {
          throw new Error(`failed to load session: HTTP ${res.status}`)
        }
        const data = await res.json() as SessionDetailResponse
        const restoredTrace = restoreTraceEntries(data.trace ?? [], data.compact_summary ?? null)
        const withCitations = attachRestoredCitations(
          (data.messages ?? []).map((message) => ({
            role: message.role,
            text: message.text,
            mentions: message.mentions,
            citations: message.citations,
          })),
          restoredTrace,
        )
        const restored = attachRestoredQuizPlans(attachRestoredDeepSolve(withCitations, restoredTrace), restoredTrace)
        setMessages(restored)
        void attachRestoredQuizzes(restored, restoredTrace).then(setMessages)
        setTraceEntries(restoredTrace)
        setLatestUsage(data.latest_usage ?? null)
        if (data.capability && isCapability(data.capability)) {
          setCapability(data.capability)
        }
        setSelectedKnowledgeBaseId(data.kb ?? '')
        setSelectedNotebookEnabled(Boolean(data.notebook_enabled))
        const title = data.metadata?.name || restored.find((message) => message.role === 'user')?.text
        if (title) {
          upsertRecentSession(setRecentSessions, id, sessionTitleFromMessage(title))
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : String(err)
        setMessages([{ role: 'assistant', text: `Error: ${message}` }])
      }
    }
    setView('chat')
  }

  const handleRenameSession = async (id: string, title: string) => {
    const previousSessions = recentSessions
    setRecentSessions((prev) =>
      prev.map((session) => (session.id === id ? { ...session, title } : session)),
    )

    try {
      const res = await fetch(`/api/sessions/${id}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name: title }),
      })
      if (!res.ok) {
        throw new Error(`failed to rename session: HTTP ${res.status}`)
      }
      void refreshSessions()
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      setRecentSessions(previousSessions)
      setMessages((prev) => [...prev, { role: 'assistant', text: `Error: ${message}` }])
    }
  }

  const handleDeleteSession = async (id: string) => {
    const session = recentSessions.find((item) => item.id === id)
    if (!window.confirm(`Delete "${session?.title ?? 'this session'}"?`)) return

    const previousSessions = recentSessions
    setRecentSessions((prev) => prev.filter((item) => item.id !== id))
    if (sessionId === id) {
      setSessionId(null)
      setMessages([])
      setStreamingText('')
      streamingRef.current = ''
      setTraceEntries([])
      pendingDeepSolveRef.current = []
      setLatestUsage(null)
    }

    try {
      const res = await fetch(`/api/sessions/${id}`, { method: 'DELETE' })
      if (!res.ok) {
        throw new Error(`failed to delete session: HTTP ${res.status}`)
      }
      void refreshSessions()
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      setRecentSessions(previousSessions)
      setMessages((prev) => [...prev, { role: 'assistant', text: `Error: ${message}` }])
    }
  }

  const handleSourceNavigate = useCallback((target: SourceTarget, reference: SourceReference) => {
    if (target.type === 'chat') {
      void handleSelectSession(target.sessionId)
      pushStatus({
        kind: 'done',
        label: 'Opened source',
        detail: target.messageId ? `Chat message ${target.messageId}` : 'Chat session',
      })
      return
    }

    if (target.type === 'web') {
      window.open(target.url, '_blank', 'noopener,noreferrer')
      return
    }

    if (target.type === 'notebook' || target.type === 'quiz' || target.type === 'research') {
      setSpaceFocusTarget(target)
      setView('space')
      pushStatus({
        kind: 'done',
        label: 'Opened source area',
        detail: sourceTargetDetail(target, reference),
      })
      return
    }

    if (target.type === 'kb') {
      setKnowledgeFocusTarget(target)
      setView('knowledge')
      pushStatus({
        kind: 'done',
        label: 'Opened source area',
        detail: sourceTargetDetail(target, reference),
      })
    }
  }, [handleSelectSession, pushStatus])

  const chatIsEmpty = view === 'chat' && messages.length === 0 && !streamingText && !sessionId
  const t = (key: TranslationKey) => translate(llmSettings.language, key)

  return (
    <I18nProvider language={llmSettings.language}>
    <div className="flex h-screen bg-gray-50">
      <Sidebar
        activeView={view}
        collapsed={sidebarCollapsed}
        recentSessions={recentSessions}
        onNavigate={handleNavigate}
        onSelectSession={handleSelectSession}
        onRenameSession={handleRenameSession}
        onDeleteSession={handleDeleteSession}
        onToggleCollapsed={() => setSidebarCollapsed((value) => !value)}
      />

      <div className="flex min-w-0 flex-1 flex-col">
        {view === 'chat' && (
          <>
            <header className="flex items-center gap-4 bg-white px-6 py-3">
              <div>
                <h1 className="text-lg font-semibold text-gray-900">{t('chat.title')}</h1>
                <p className="text-xs text-gray-500">{t('chat.subtitle')}</p>
              </div>
              <div className="ml-auto">
                <BudgetPanel spent={budgetSpent} limit={llmSettings.budgetLimitUsd} warning={budgetWarning} />
              </div>
            </header>
            <div className="flex flex-1 min-h-0">
              <main className="flex-1">
                <ChatBox
                  messages={messages}
                  streamingText={streamingText}
                  contextStats={contextStats}
                  capability={capability}
                  llmConfigs={llmSettings.llmConfigs}
                  activeLlmConfigId={llmSettings.activeLlmConfigId}
                  knowledgeBases={knowledgeBases}
                  selectedKnowledgeBaseId={selectedKnowledgeBaseId}
                  selectedNotebookEnabled={selectedNotebookEnabled}
                  onSend={handleSend}
                  onStop={handleStopGeneration}
                  onEditUserMessage={handleEditUserMessage}
                  onAskDeepSolveStep={handleAskDeepSolveStep}
                  onCapabilityChange={handleCapabilityChange}
                  onKnowledgeBaseChange={handleKnowledgeBaseChange}
                  onNotebookEnabledChange={handleNotebookEnabledChange}
                  onLlmConfigChange={handleLlmConfigChange}
                  onSaveToNotebook={handleSaveToNotebook}
                  onApplyNotebookEdit={handleApplyNotebookEdit}
                  onQuizAnswer={handleQuizAnswer}
                  onQuizFinish={handleQuizFinish}
                  onSourceNavigate={handleSourceNavigate}
                  disabled={false}
                  running={running}
                />
              </main>
              {!chatIsEmpty && (
                <aside
                  className={`shrink-0 bg-white transition-[width] duration-200 ${
                    traceCollapsed ? 'w-12' : 'w-72'
                  }`}
                >
                  <TracePanel
                    entries={traceEntries}
                    collapsed={traceCollapsed}
                    onToggleCollapsed={() => setTraceCollapsed((value) => !value)}
                  />
                </aside>
              )}
            </div>
          </>
        )}

        {view === 'tutor' && (
          <PlaceholderPage
            title="Tutor Agent"
            description="A guided learning entry point for explanations, mistake analysis, step-by-step follow-up, and study path suggestions."
          />
        )}

        {view === 'knowledge' && (
          <KnowledgePage settings={llmSettings} onChanged={refreshKnowledgeBases} focusTarget={knowledgeFocusTarget} />
        )}

        {view === 'space' && (
          <SpacePage focusTarget={spaceFocusTarget} onSourceNavigate={handleSourceNavigate} />
        )}

        {view === 'memory' && (
          <MemoryPage settings={llmSettings} onSourceNavigate={handleSourceNavigate} />
        )}

        {view === 'settings' && (
          <SettingsPage settings={llmSettings} onChange={handleSettingsChange} />
        )}
      </div>

      <ApprovalDialog request={pendingApproval} onDecision={handleApproval} />
    </div>
    </I18nProvider>
  )
}

function dropTrailingTransientStatus(messages: Message[]) {
  const last = messages[messages.length - 1]
  if (last?.role === 'status' && last.transient) {
    return messages.slice(0, -1)
  }
  return messages
}

function statusFromTrace(payload: Record<string, unknown>): AgentStatus {
  const kind = payload.kind
  const capability = typeof payload.capability === 'string' ? capabilityLabel(payload.capability) : 'Agent'
  const phase = typeof payload.phase === 'string' ? phaseLabel(payload.phase) : undefined

  if (kind === 'phase_start') {
    return {
      kind: 'thinking',
      label: phase ?? 'Thinking',
      detail: capability,
    }
  }

  if (kind === 'phase_end') {
    return {
      kind: 'thinking',
      label: `${phase ?? 'Phase'} complete`,
      detail: capability,
    }
  }

  if (kind === 'tool_call') {
    return {
      kind: 'tool',
      label: `Using ${String(payload.tool ?? 'tool')}`,
      detail: typeof payload.step_id === 'string' ? `Step ${payload.step_id}` : capability,
    }
  }

  if (kind === 'tool_result') {
    return {
      kind: 'thinking',
      label: `${String(payload.tool ?? 'Tool')} finished`,
      detail: payload.ok === false ? 'Tool returned an error' : 'Reading result',
    }
  }

  if (kind === 'rag_citations') {
    const details = payload.details as { hits?: unknown } | undefined
    return {
      kind: 'tool',
      label: 'Sources attached',
      detail: typeof details?.hits === 'number' ? `${details.hits} citations` : capability,
    }
  }

  if (kind === 'research_stage_start') {
    return {
      kind: 'thinking',
      label: typeof payload.title === 'string' ? payload.title : 'Planning research',
      detail: capability,
    }
  }

  if (kind === 'research_search') {
    return {
      kind: 'tool',
      label: 'Searching web',
      detail: capability,
    }
  }

  if (kind === 'research_read') {
    return {
      kind: 'tool',
      label: 'Reading source',
      detail: capability,
    }
  }

  if (kind === 'research_report_done') {
    return {
      kind: 'done',
      label: 'Research report ready',
      detail: capability,
    }
  }

  if (kind === 'replan') {
    return {
      kind: 'thinking',
      label: 'Replanning',
      detail: typeof payload.reason === 'string' ? payload.reason : undefined,
    }
  }

  if (kind === 'event_lagged') {
    return {
      kind: 'thinking',
      label: 'Catching up',
      detail: `Skipped ${String(payload.skipped ?? 0)} stale events`,
    }
  }

  return { kind: 'thinking', label: 'Working', detail: capability }
}

function citationsFromTrace(payload: Record<string, unknown>): Citation[] {
  const isRagToolResult = payload.kind === 'tool_result' && payload.tool === 'rag_search'
  const isWebToolResult =
    payload.kind === 'tool_result' && (payload.tool === 'web_search' || payload.tool === 'web_fetch')
  const isRagCitationEvent = payload.kind === 'rag_citations'
  if (!isRagToolResult && !isWebToolResult && !isRagCitationEvent) return []
  const details = payload.details
  if (!details || typeof details !== 'object') return []
  const sources = (details as { sources?: unknown }).sources
  if (!Array.isArray(sources)) return []
  return sources
    .map((source): Citation | null => {
      if (!source || typeof source !== 'object') return null
      const item = source as Record<string, unknown>
      return {
        index: typeof item.index === 'number' ? item.index : 0,
        source: typeof item.source === 'string' ? item.source : 'source',
        text: typeof item.text === 'string' ? item.text : '',
        kind: item.kind === 'web' ? 'web' : 'rag',
        title: typeof item.title === 'string' ? item.title : undefined,
        url: typeof item.url === 'string' ? item.url : undefined,
        score: typeof item.score === 'number' ? item.score : null,
        kb: typeof item.kb === 'string' ? item.kb : undefined,
        documentId: typeof item.document_id === 'string' ? item.document_id : undefined,
        chunkId: typeof item.chunk_id === 'string' ? item.chunk_id : typeof item.id === 'string' ? item.id : undefined,
        rawSource: typeof item.raw_source === 'string' ? item.raw_source : undefined,
        page: typeof item.page === 'string' || typeof item.page === 'number' ? item.page : undefined,
      }
    })
    .filter((source): source is Citation => Boolean(source && source.text))
}

function mergeCitations(existing: Citation[], incoming: Citation[]): Citation[] {
  const merged = [...existing]
  for (const citation of incoming) {
    const key = citation.url || `${citation.source}:${citation.text.slice(0, 80)}`
    const seen = merged.some((item) => (item.url || `${item.source}:${item.text.slice(0, 80)}`) === key)
    if (!seen) {
      merged.push({ ...citation, index: merged.length + 1 })
    }
  }
  return merged
}

function notebookEditProposalFromTrace(payload: Record<string, unknown>): NotebookEditProposal | undefined {
  if (payload.kind !== 'tool_result' || payload.tool !== 'propose_notebook_edit' || payload.ok === false) return undefined
  const details = payload.details
  if (!details || typeof details !== 'object') return undefined
  const item = details as Record<string, unknown>
  if (item.found === false) return undefined
  const entryId = typeof item.entry_id === 'string' ? item.entry_id : ''
  const proposedMarkdown = typeof item.proposed_markdown === 'string' ? item.proposed_markdown : ''
  if (!entryId || !proposedMarkdown.trim()) return undefined
  const entryTitle = typeof item.entry_title === 'string' ? item.entry_title : 'Notebook entry'
  return {
    entryId,
    entryTitle,
    proposedTitle: typeof item.proposed_title === 'string' && item.proposed_title.trim() ? item.proposed_title : entryTitle,
    proposedMarkdown,
    summary: typeof item.summary === 'string' && item.summary.trim() ? item.summary : 'Proposed Notebook update',
    proposalKind: notebookProposalKind(item.proposal_kind),
    suggestedLinks: notebookSuggestedLinks(item.suggested_links),
    suggestedTags: notebookSuggestedTags(item.suggested_tags),
    mergeSourceEntryIds: notebookMergeSourceEntryIds(item.merge_source_entry_ids),
  }
}

function quizFromTrace(payload: Record<string, unknown>): QuizSession | undefined {
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

function quizPlanFromTrace(payload: Record<string, unknown>): QuizPlan | undefined {
  if (payload.kind !== 'tool_result' || payload.tool !== 'propose_quiz_plan' || payload.ok === false) return undefined
  const details = payload.details
  if (!details || typeof details !== 'object') return undefined
  const item = details as Record<string, unknown>
  const title = typeof item.title === 'string' && item.title.trim() ? item.title : 'Quiz plan'
  const topic = typeof item.topic === 'string' && item.topic.trim() ? item.topic : 'selected material'
  const source = typeof item.source === 'string' && item.source.trim() ? item.source : 'current conversation'
  const difficulty = typeof item.difficulty === 'string' && item.difficulty.trim() ? item.difficulty : 'medium'
  const questionCount = typeof item.question_count === 'number' ? item.question_count : 5
  const notes = Array.isArray(item.notes)
    ? item.notes.filter((note): note is string => typeof note === 'string' && note.trim().length > 0)
    : []
  return {
    title,
    topic,
    source,
    difficulty,
    questionCount,
    notes,
  }
}

function notebookProposalKind(value: unknown): NotebookEditProposal['proposalKind'] {
  return value === 'links' || value === 'tags' || value === 'merge' || value === 'edit' ? value : 'edit'
}

function notebookSuggestedLinks(value: unknown): NotebookEditProposal['suggestedLinks'] {
  if (!Array.isArray(value)) return []
  const links: NonNullable<NotebookEditProposal['suggestedLinks']> = []
  for (const item of value) {
    if (!item || typeof item !== 'object') continue
    const record = item as Record<string, unknown>
    const text = typeof record.text === 'string' ? record.text.trim() : ''
    const target = typeof record.target === 'string' ? record.target.trim() : ''
    if (!text || !target) continue
    links.push({
      text,
      target,
      reason: typeof record.reason === 'string' && record.reason.trim() ? record.reason.trim() : undefined,
    })
  }
  return links
}

function notebookSuggestedTags(value: unknown): NotebookEditProposal['suggestedTags'] {
  if (!Array.isArray(value)) return []
  const tags: NonNullable<NotebookEditProposal['suggestedTags']> = []
  for (const item of value) {
    if (!item || typeof item !== 'object') continue
    const record = item as Record<string, unknown>
    const tag = typeof record.tag === 'string' ? record.tag.trim().replace(/^#/, '') : ''
    const action = record.action
    if (!tag || (action !== 'add' && action !== 'keep' && action !== 'remove')) continue
    tags.push({
      tag,
      action,
      reason: typeof record.reason === 'string' && record.reason.trim() ? record.reason.trim() : undefined,
    })
  }
  return tags
}

function notebookMergeSourceEntryIds(value: unknown): string[] {
  if (!Array.isArray(value)) return []
  return value
    .filter((item): item is string => typeof item === 'string')
    .map((item) => item.trim())
    .filter(Boolean)
}

function restoreTraceEntries(
  trace: NonNullable<SessionDetailResponse['trace']>,
  compactSummary: NonNullable<SessionDetailResponse['compact_summary']> | null,
): TraceEntry[] {
  const entries: TraceEntry[] = trace.map((entry) => {
    const payload = {
      ...(entry.payload ?? {}),
      kind: entry.kind,
    }
    return {
      kind: entry.kind,
      payload,
      timestamp: entry.timestamp ? Date.parse(entry.timestamp) : Date.now(),
    }
  })

  if (compactSummary?.summary) {
    const payload: Record<string, unknown> = {
      kind: 'compact_summary',
      summary: compactSummary.summary,
      message_count: compactSummary.message_count,
    }
    entries.unshift({
      kind: 'compact_summary',
      payload,
      timestamp: compactSummary.timestamp ? Date.parse(compactSummary.timestamp) : Date.now(),
    })
  }

  return entries
}

function attachRestoredCitations(messages: Message[], traceEntries: TraceEntry[]): Message[] {
  const citationGroups = traceEntries
    .filter((entry) => {
      const payload = entry.payload as Record<string, unknown>
      return (
        entry.kind === 'rag_citations' ||
        (entry.kind === 'tool_result' &&
          (payload.tool === 'rag_search' || payload.tool === 'web_search' || payload.tool === 'web_fetch'))
      )
    })
    .map((entry) => citationsFromTrace(entry.payload))
    .filter((citations) => citations.length > 0)

  if (citationGroups.length === 0) return messages

  let citationIndex = 0
  return messages.map((message) => {
    if (message.role !== 'assistant') return message
    if (message.citations && message.citations.length > 0) return message
    const citations = citationGroups[citationIndex]
    citationIndex += 1
    return citations ? { ...message, citations } : message
  })
}

function deepSolveEventFromTrace(payload: Record<string, unknown>, timestamp = Date.now()): DeepSolveTraceEntry | null {
  const kind = typeof payload.kind === 'string' ? payload.kind : ''
  const capability = typeof payload.capability === 'string' ? payload.capability : ''
  if (!kind.startsWith('deep_solve_') && capability !== 'deep_solve') return null

  return {
    kind,
    payload,
    timestamp,
  }
}

function attachRestoredDeepSolve(messages: Message[], traceEntries: TraceEntry[]): Message[] {
  const deepSolveEvents = traceEntries
    .map((entry) => deepSolveEventFromTrace(entry.payload, entry.timestamp))
    .filter((entry): entry is DeepSolveTraceEntry => Boolean(entry))

  if (deepSolveEvents.length === 0) return messages

  const groups: DeepSolveTraceEntry[][] = []
  let current: DeepSolveTraceEntry[] = []
  for (const event of deepSolveEvents) {
    current.push(event)
    if (event.kind === 'deep_solve_final') {
      groups.push(current)
      current = []
    }
  }
  if (current.length > 0) {
    groups.push(current)
  }

  let groupIndex = 0
  return messages.map((message) => {
    if (message.role !== 'assistant') return message
    const group = groups[groupIndex]
    if (!group) return message
    groupIndex += 1
    return { ...message, deepSolve: group }
  })
}

function attachRestoredQuizPlans(messages: Message[], traceEntries: TraceEntry[]): Message[] {
  const plans = traceEntries
    .map((entry) => quizPlanFromTrace(entry.payload))
    .filter((plan): plan is QuizPlan => Boolean(plan))
  if (plans.length === 0) return messages

  let planIndex = 0
  return messages.map((message) => {
    if (message.role !== 'assistant') return message
    if (message.quiz || message.quizPlan) return message
    const plan = plans[planIndex]
    planIndex += 1
    return plan ? { ...message, quizPlan: plan } : message
  })
}

async function attachRestoredQuizzes(messages: Message[], traceEntries: TraceEntry[]): Promise<Message[]> {
  const restoredQuizzes = traceEntries
    .map((entry) => quizFromTrace(entry.payload))
    .filter((quiz): quiz is QuizSession => Boolean(quiz))
  const quizIds = traceEntries
    .filter((entry) => entry.kind === 'quiz_created')
    .map((entry) => {
      const payload = entry.payload as Record<string, unknown>
      return typeof payload.quiz_id === 'string' ? payload.quiz_id : null
    })
    .filter((id): id is string => Boolean(id))

  if (restoredQuizzes.length === 0 && quizIds.length === 0) return messages

  const fetchedQuizzes = await Promise.all(
    quizIds.map(async (id) => {
      try {
        const res = await fetch(`/api/quizzes/${encodeURIComponent(id)}`)
        const data = await safeJson(res)
        return res.ok ? data.quiz as QuizSession : null
      } catch {
        return null
      }
    }),
  )

  const quizzes = [
    ...restoredQuizzes,
    ...fetchedQuizzes.filter((quiz): quiz is QuizSession => Boolean(quiz)),
  ]
  let quizIndex = 0
  return messages.map((message) => {
    if (message.role !== 'assistant') return message
    if (message.quiz) return message
    const quiz = quizzes[quizIndex]
    quizIndex += 1
    return quiz ? { ...message, quiz } : message
  })
}

function capabilityLabel(value: string): string {
  if (value === 'deep_solve') return 'Deep Solve'
  if (value === 'code_exec') return 'Code Exec'
  if (value === 'quiz') return 'Quiz'
  if (value === 'research') return 'Research'
  if (value === 'organize') return 'Organize'
  return 'Chat'
}

function phaseLabel(value: string): string {
  const labels: Record<string, string> = {
    respond: 'Responding',
    execute: 'Executing code',
    pre_retrieve: 'Preparing knowledge',
    plan: 'Planning',
    solve_steps: 'Solving steps',
    solve_step: 'Solving step',
    synthesize: 'Synthesizing answer',
  }
  return labels[value] ?? value
}

function sessionTitleFromMessage(text: string) {
  const normalized = text.replace(/\s+/g, ' ').trim()
  if (!normalized) return '鏂扮殑浼氳瘽'
  return normalized.length > 18 ? `${normalized.slice(0, 18)}...` : normalized
}

function upsertRecentSession(
  setRecentSessions: Dispatch<SetStateAction<RecentSession[]>>,
  id: string,
  title: string,
) {
  setRecentSessions((prev) => [
    { id, title },
    ...prev.filter((session) => session.id !== id),
  ])
}

function estimateContextTokens(messages: Message[], streamingText: string) {
  const text = [
    ...messages
      .filter((message) => message.role === 'user' || message.role === 'assistant')
      .map((message) => message.text),
    streamingText,
  ].join('\n')
  if (!text.trim()) return 0

  let ascii = 0
  let nonAscii = 0
  for (const char of text) {
    if (char.charCodeAt(0) <= 0x7f) ascii += 1
    else nonAscii += 1
  }

  const messageOverhead = messages.filter((message) => message.role === 'user' || message.role === 'assistant').length * 4
  return Math.ceil(ascii / 4 + nonAscii * 1.2 + messageOverhead)
}

function buildMessageContentWithAttachments(text: string, attachments: ChatAttachment[]) {
  const baseText = text.trim()
  const source = attachmentSourceText(attachments)
  if (!source) return baseText
  return `${baseText || 'Please continue based on the attachment content.'}\n\n${source}`
}

function attachmentSourceText(attachments: ChatAttachment[]) {
  const readable = attachments.filter((attachment) => attachment.text?.trim())
  if (readable.length === 0) return ''
  return [
    '[Attachment context]',
    ...readable.map((attachment, index) => [
      `### ${index + 1}. ${attachment.name}`,
      `Type: ${attachment.type || 'unknown'}`,
      `Size: ${formatBytes(attachment.size)}`,
      attachment.truncated ? 'Note: content was truncated.' : null,
      '',
      attachment.text?.trim() ?? '',
    ].filter(Boolean).join('\n')),
  ].join('\n\n')
}

function titleFromMarkdown(markdown: string) {
  const heading = markdown
    .split('\n')
    .map((line) => line.trim())
    .find((line) => line.startsWith('# '))
  if (heading) return heading.replace(/^#\s+/, '').trim().slice(0, 80) || 'Research Report'
  const first = markdown.trim().split('\n').find((line) => line.trim())
  return first?.trim().slice(0, 80) || 'Research Report'
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}

function isTokenUsagePayload(value: unknown): value is TokenUsagePayload {
  return Boolean(value && typeof value === 'object')
}

function isCapability(value: string): value is Capability {
  return value === 'chat' || value === 'deep_solve' || value === 'code_exec' || value === 'quiz' || value === 'research' || value === 'organize'
}

function sourceTargetDetail(target: SourceTarget, reference: SourceReference) {
  if (target.type === 'notebook') return `Notebook ${target.entryId}`
  if (target.type === 'quiz') return target.questionId ? `Quiz ${target.quizId}, question ${target.questionId}` : `Quiz ${target.quizId}`
  if (target.type === 'research') return `Research report ${target.notebookEntryId}`
  if (target.type === 'book') return target.chapterId ? `Book ${target.bookId}, chapter ${target.chapterId}` : `Book ${target.bookId}`
  if (target.type === 'kb') return target.chunkId ? `Knowledge ${target.documentId}, chunk ${target.chunkId}` : `Knowledge ${target.documentId}`
  return reference.raw
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

async function persistMessageCitations(sessionId: string, citations: Citation[]) {
  const res = await fetch(`/api/sessions/${sessionId}/message-citations`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ citations }),
  })
  const data = await safeJson(res)
  if (!res.ok) {
    throw new Error(errorMessage(data, res.status))
  }
}
