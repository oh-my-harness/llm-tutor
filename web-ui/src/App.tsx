import { useState, useCallback, useEffect, useMemo, useRef } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import { ChatBox } from './components/ChatBox'
import type { ChatAttachment, ContextStats } from './components/ChatBox'
import { TracePanel, TraceEntry } from './components/TracePanel'
import { BudgetPanel } from './components/BudgetPanel'
import { ApprovalDialog } from './components/ApprovalDialog'
import { SettingsPage } from './components/SettingsPage'
import { KnowledgePage } from './components/KnowledgePage'
import { BooksPage } from './components/BooksPage'
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

type Capability = 'chat' | 'deep_solve' | 'code_exec' | 'quiz' | 'research'

interface Message {
  role: 'user' | 'assistant' | 'status'
  text: string
  kind?: AgentStatus['kind']
  transient?: boolean
  citations?: Citation[]
  deepSolve?: DeepSolveTraceEntry[]
  quiz?: QuizSession
  attachments?: ChatAttachment[]
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
  messages?: Array<{
    role: 'user' | 'assistant'
    text: string
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
  const [traceEntries, setTraceEntries] = useState<TraceEntry[]>([])
  const pendingCitationsRef = useRef<Citation[]>([])
  const pendingDeepSolveRef = useRef<DeepSolveTraceEntry[]>([])
  const [budgetSpent, setBudgetSpent] = useState(0)
  const [budgetWarning, setBudgetWarning] = useState(false)
  const [pendingApproval, setPendingApproval] = useState<{ tool: string; args: Record<string, unknown>; requestId: string } | null>(null)
  const [running, setRunning] = useState(false)
  const [recentSessions, setRecentSessions] = useState<RecentSession[]>([])
  const [knowledgeBases, setKnowledgeBases] = useState<KnowledgeBaseOption[]>([])
  const [selectedKnowledgeBaseId, setSelectedKnowledgeBaseId] = useState<string>('')
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)
  const [traceCollapsed, setTraceCollapsed] = useState(true)
  const [spaceFocusTarget, setSpaceFocusTarget] = useState<Extract<SourceTarget, { type: 'notebook' | 'quiz' | 'research' }> | null>(null)
  const [bookFocusTarget, setBookFocusTarget] = useState<Extract<SourceTarget, { type: 'book' }> | null>(null)
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
          setMessages((prev) => [
            ...dropTrailingTransientStatus(prev),
            {
              role: 'assistant',
              text: finalText,
              citations,
              deepSolve: deepSolve.length > 0 ? deepSolve : undefined,
            },
          ])
          pendingCitationsRef.current = []
          pendingDeepSolveRef.current = []
          streamingRef.current = ''
          setStreamingText('')
          setRunning(false)
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

  const handleSend = useCallback(async (text: string, attachments: ChatAttachment[] = []) => {
    try {
      const content = buildMessageContentWithAttachments(text, attachments)
      const displayText = text.trim() || `发送了 ${attachments.length} 个附件`
      let sid = sessionId
      if (!sid) {
        const kb = selectedKnowledgeBaseId || null
        const res = await fetch('/api/sessions', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            capability,
            kb,
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

      setMessages((prev) => [...prev, { role: 'user', text: displayText, attachments }])
      setRunning(true)
      pushStatus({ kind: 'thinking', label: 'Thinking', detail: capabilityLabel(capability) })
      if (capability === 'quiz') {
        const attachmentSource = attachmentSourceText(attachments)
        const conversationSource = [quizSourceFromMessages(messages), attachmentSource].filter(Boolean).join('\n\n')
        if (!selectedKnowledgeBaseId && !conversationSource.trim()) {
          throw new Error('当前还没有可用于出题的对话内容。请先提供一段材料，或关联知识库。')
        }
        pushStatus({
          kind: 'tool',
          label: 'Generating quiz',
          detail: selectedKnowledgeBaseId ? 'Knowledge base' : 'Conversation',
        })
        const res = await fetch('/api/quizzes', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            kb_id: selectedKnowledgeBaseId || null,
            source_text: selectedKnowledgeBaseId ? null : conversationSource,
            source_label: selectedKnowledgeBaseId ? null : '当前对话',
            topic: text.trim() || (attachments.length > 0 ? '附件内容' : null),
            difficulty: 'medium',
            question_count: 5,
            llm: settingsForSession(llmSettings),
          }),
        })
        const data = await safeJson(res)
        if (!res.ok) {
          throw new Error(errorMessage(data, res.status))
        }
        const quiz = data.quiz as QuizSession
        const assistantText = `已根据“${displayText}”生成 Quiz。`
        await persistQuizMessage(sid, content, assistantText, quiz.id)
        setMessages((prev) => [
          ...dropTrailingTransientStatus(prev),
          {
            role: 'assistant',
            text: assistantText,
            quiz,
          },
        ])
        setRunning(false)
        pushStatus({ kind: 'done', label: 'Done', detail: 'Quiz generated' })
        void refreshSessions()
        return
      }
      send({ type: 'message', content })
      void refreshSessions()
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      pushStatus({ kind: 'error', label: 'Error', detail: message })
      setMessages((prev) => [...prev, { role: 'assistant', text: `Error: ${message}` }])
      setRunning(false)
    }
  }, [sessionId, capability, llmSettings, selectedKnowledgeBaseId, send, pushStatus, refreshSessions, messages])

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
    pendingDeepSolveRef.current = []
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
    if (!sessionId) return

    try {
      const res = await fetch(`/api/sessions/${sessionId}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ capability: nextCapability }),
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
    if (!sessionId) return

    try {
      const res = await fetch(`/api/sessions/${sessionId}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ kb: nextKb }),
      })
      if (!res.ok) {
        throw new Error(`failed to update session knowledge base: HTTP ${res.status}`)
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
      pendingDeepSolveRef.current = []
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
          })),
          restoredTrace,
        )
        const restored = attachRestoredDeepSolve(withCitations, restoredTrace)
        setMessages(restored)
        void attachRestoredQuizzes(restored, restoredTrace).then(setMessages)
        setTraceEntries(restoredTrace)
        setLatestUsage(data.latest_usage ?? null)
        if (data.capability && isCapability(data.capability)) {
          setCapability(data.capability)
        }
        setSelectedKnowledgeBaseId(data.kb ?? '')
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

    if (target.type === 'book') {
      setBookFocusTarget(target)
      setView('books')
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

  return (
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
                <h1 className="text-lg font-semibold text-gray-900">聊天</h1>
                <p className="text-xs text-gray-500">Ask questions, run code, and inspect traces.</p>
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
                  onSend={handleSend}
                  onAskDeepSolveStep={handleAskDeepSolveStep}
                  onCapabilityChange={handleCapabilityChange}
                  onKnowledgeBaseChange={handleKnowledgeBaseChange}
                  onLlmConfigChange={handleLlmConfigChange}
                  onSaveToNotebook={handleSaveToNotebook}
                  onQuizAnswer={handleQuizAnswer}
                  onQuizFinish={handleQuizFinish}
                  onSourceNavigate={handleSourceNavigate}
                  disabled={running}
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
            title="辅导机器人"
            description="面向学习过程的专题辅导入口，后续可以承接题目讲解、错因分析、分步追问和学习路径推荐。"
          />
        )}

        {view === 'writing' && (
          <PlaceholderPage
            title="智能写作"
            description="用于作文、读书笔记、报告和表达训练，后续可以加入大纲、润色、批改和评分流程。"
          />
        )}

        {view === 'books' && (
          <BooksPage focusTarget={bookFocusTarget} />
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

async function attachRestoredQuizzes(messages: Message[], traceEntries: TraceEntry[]): Promise<Message[]> {
  const quizIds = traceEntries
    .filter((entry) => entry.kind === 'quiz_created')
    .map((entry) => {
      const payload = entry.payload as Record<string, unknown>
      return typeof payload.quiz_id === 'string' ? payload.quiz_id : null
    })
    .filter((id): id is string => Boolean(id))

  if (quizIds.length === 0) return messages

  const quizzes = await Promise.all(
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

  let quizIndex = 0
  return messages.map((message) => {
    if (message.role !== 'assistant' || !message.text.includes('生成 Quiz')) return message
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
  if (!normalized) return '新的会话'
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

function quizSourceFromMessages(messages: Message[]) {
  return messages
    .filter((message) => message.role === 'user' || message.role === 'assistant')
    .filter((message) => !message.quiz && message.text.trim())
    .map((message) => `${message.role === 'user' ? 'User' : 'Assistant'}: ${message.text.trim()}`)
    .join('\n\n')
    .slice(-12000)
}

function buildMessageContentWithAttachments(text: string, attachments: ChatAttachment[]) {
  const baseText = text.trim()
  const source = attachmentSourceText(attachments)
  if (!source) return baseText
  return `${baseText || '请根据附件内容继续。'}\n\n${source}`
}

function attachmentSourceText(attachments: ChatAttachment[]) {
  const readable = attachments.filter((attachment) => attachment.text?.trim())
  if (readable.length === 0) return ''
  return [
    '[附件上下文]',
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
  return value === 'chat' || value === 'deep_solve' || value === 'code_exec' || value === 'quiz' || value === 'research'
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

async function persistQuizMessage(sessionId: string, user: string, assistant: string, quizId: string) {
  const res = await fetch(`/api/sessions/${sessionId}/messages`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      user,
      assistant,
      quiz_id: quizId,
    }),
  })
  const data = await safeJson(res)
  if (!res.ok) {
    throw new Error(errorMessage(data, res.status))
  }
}
