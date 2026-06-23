import { useState, useCallback, useEffect, useRef } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import { ChatBox } from './components/ChatBox'
import { TracePanel, TraceEntry } from './components/TracePanel'
import { BudgetPanel } from './components/BudgetPanel'
import { ApprovalDialog } from './components/ApprovalDialog'
import { SettingsPage } from './components/SettingsPage'
import { KnowledgePage } from './components/KnowledgePage'
import { PlaceholderPage } from './components/PlaceholderPage'
import { AppView, Sidebar } from './components/Sidebar'
import { AgentStatus } from './agentStatus'
import { useWebSocket } from './hooks/useWebSocket'
import { loadLlmSettings, saveLlmSettings, settingsForSession } from './settings'

type Capability = 'chat' | 'deep_solve' | 'code_exec'

interface Message {
  role: 'user' | 'assistant' | 'status'
  text: string
  kind?: AgentStatus['kind']
  transient?: boolean
  citations?: Citation[]
}

interface Citation {
  index: number
  source: string
  text: string
  score?: number | null
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
  metadata?: {
    name?: string | null
  }
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
  const [budgetSpent, setBudgetSpent] = useState(0)
  const [budgetWarning, setBudgetWarning] = useState(false)
  const [pendingApproval, setPendingApproval] = useState<{ tool: string; args: Record<string, unknown>; requestId: string } | null>(null)
  const [running, setRunning] = useState(false)
  const [recentSessions, setRecentSessions] = useState<RecentSession[]>([])
  const [knowledgeBases, setKnowledgeBases] = useState<KnowledgeBaseOption[]>([])
  const [selectedKnowledgeBaseId, setSelectedKnowledgeBaseId] = useState<string>('')
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false)
  const [traceCollapsed, setTraceCollapsed] = useState(false)

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
          setMessages((prev) => [
            ...dropTrailingTransientStatus(prev),
            { role: 'assistant', text: finalText, citations },
          ])
          pendingCitationsRef.current = []
          streamingRef.current = ''
          setStreamingText('')
          setRunning(false)
          void refreshSessions()
        }
      } else if (event.type === 'trace') {
        const citations = citationsFromTrace(event.payload as Record<string, unknown>)
        if (citations.length > 0) {
          pendingCitationsRef.current = citations
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

  const handleSend = useCallback(async (text: string) => {
    try {
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
          }),
        })
        if (!res.ok) {
          throw new Error(`failed to create session: HTTP ${res.status}`)
        }
        const data = await res.json()
        const createdSessionId = data.id as string
        sid = createdSessionId
        setSessionId(createdSessionId)
        upsertRecentSession(setRecentSessions, createdSessionId, sessionTitleFromMessage(text))
      }

      setMessages((prev) => [...prev, { role: 'user', text }])
      setRunning(true)
      pushStatus({ kind: 'thinking', label: 'Thinking', detail: capabilityLabel(capability) })
      send({ type: 'message', content: text })
      void refreshSessions()
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err)
      pushStatus({ kind: 'error', label: 'Error', detail: message })
      setMessages((prev) => [...prev, { role: 'assistant', text: `Error: ${message}` }])
      setRunning(false)
    }
  }, [sessionId, capability, llmSettings, selectedKnowledgeBaseId, send, pushStatus, refreshSessions])

  const handleSettingsChange = (nextSettings: typeof llmSettings) => {
    setLlmSettings(nextSettings)
    saveLlmSettings(nextSettings)
    setSessionId(null)
  }

  const startNewChat = useCallback(() => {
    setSessionId(null)
    setMessages([])
    setStreamingText('')
    streamingRef.current = ''
    setTraceEntries([])
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
    saveLlmSettings(nextSettings)
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
  }, [llmSettings, running, sessionId])

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
      setSessionId(id)
      try {
        const res = await fetch(`/api/sessions/${id}`)
        if (!res.ok) {
          throw new Error(`failed to load session: HTTP ${res.status}`)
        }
        const data = await res.json() as SessionDetailResponse
        const restoredTrace = restoreTraceEntries(data.trace ?? [], data.compact_summary ?? null)
        const restored = attachRestoredCitations(
          (data.messages ?? []).map((message) => ({
          role: message.role,
          text: message.text,
          })),
          restoredTrace,
        )
        setMessages(restored)
        setTraceEntries(restoredTrace)
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
                  capability={capability}
                  llmConfigs={llmSettings.llmConfigs}
                  activeLlmConfigId={llmSettings.activeLlmConfigId}
                  knowledgeBases={knowledgeBases}
                  selectedKnowledgeBaseId={selectedKnowledgeBaseId}
                  onSend={handleSend}
                  onCapabilityChange={handleCapabilityChange}
                  onKnowledgeBaseChange={handleKnowledgeBaseChange}
                  onLlmConfigChange={handleLlmConfigChange}
                  disabled={running}
                />
              </main>
              {!chatIsEmpty && (
                <aside
                  className={`shrink-0 border-l bg-white transition-[width] duration-200 ${
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
          <PlaceholderPage
            title="书籍"
            description="书籍与教材阅读空间，后续可以展示章节、阅读进度、摘要和与当前书籍相关的问答。"
          />
        )}

        {view === 'knowledge' && (
          <KnowledgePage settings={llmSettings} onChanged={refreshKnowledgeBases} />
        )}

        {view === 'space' && (
          <PlaceholderPage
            title="空间"
            description="用于组织项目、课程、班级或个人学习空间，后续可以承载权限、资料集合和任务面板。"
          />
        )}

        {view === 'memory' && (
          <PlaceholderPage
            title="记忆"
            description="展示 agent 对用户偏好、学习进度和长期上下文的记忆。当前会话历史仍是后端内存态。"
          />
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
  const isRagCitationEvent = payload.kind === 'rag_citations'
  if (!isRagToolResult && !isRagCitationEvent) return []
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
        score: typeof item.score === 'number' ? item.score : null,
      }
    })
    .filter((source): source is Citation => Boolean(source && source.text))
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
    .filter((entry) => entry.kind === 'rag_citations')
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

function capabilityLabel(value: string): string {
  if (value === 'deep_solve') return 'Deep Solve'
  if (value === 'code_exec') return 'Code Exec'
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

function isCapability(value: string): value is Capability {
  return value === 'chat' || value === 'deep_solve' || value === 'code_exec'
}
