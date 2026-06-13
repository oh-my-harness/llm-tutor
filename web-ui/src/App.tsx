import { useState, useCallback, useRef } from 'react'
import { CapabilitySelector } from './components/CapabilitySelector'
import { ChatBox } from './components/ChatBox'
import { TracePanel, TraceEntry } from './components/TracePanel'
import { BudgetPanel } from './components/BudgetPanel'
import { ApprovalDialog } from './components/ApprovalDialog'
import { SettingsPage } from './components/SettingsPage'
import { useWebSocket } from './hooks/useWebSocket'
import { loadLlmSettings, saveLlmSettings, settingsForSession } from './settings'

type Capability = 'chat' | 'deep_solve' | 'code_exec'
type View = 'chat' | 'settings'

interface Message {
  role: 'user' | 'assistant'
  text: string
}

export default function App() {
  const [view, setView] = useState<View>('chat')
  const [capability, setCapability] = useState<Capability>('chat')
  const [llmSettings, setLlmSettings] = useState(loadLlmSettings)
  const [sessionId, setSessionId] = useState<string | null>(null)
  const [messages, setMessages] = useState<Message[]>([])
  const [streamingText, setStreamingText] = useState('')
  const streamingRef = useRef('')
  const [traceEntries, setTraceEntries] = useState<TraceEntry[]>([])
  const [budgetSpent, setBudgetSpent] = useState(0)
  const [budgetWarning, setBudgetWarning] = useState(false)
  const [pendingApproval, setPendingApproval] = useState<{ tool: string; args: Record<string, unknown>; requestId: string } | null>(null)
  const [running, setRunning] = useState(false)

  const { send } = useWebSocket(sessionId, {
    onEvent: (event) => {
      if (event.type === 'content') {
        if (event.payload.chunk) {
          streamingRef.current += event.payload.text
          setStreamingText(streamingRef.current)
        } else {
          const finalText = streamingRef.current + event.payload.text
          setMessages((prev) => [
            ...prev,
            { role: 'assistant', text: finalText },
          ])
          streamingRef.current = ''
          setStreamingText('')
          setRunning(false)
        }
      } else if (event.type === 'trace') {
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
        } else if (kind === 'approval_request') {
          setPendingApproval({
            tool: payload.tool as string,
            args: payload.args as Record<string, unknown>,
            requestId: payload.request_id as string,
          })
        }
      }
    },
    onClose: () => setRunning(false),
  })

  const handleSend = useCallback(async (text: string) => {
    let sid = sessionId
    if (!sid) {
      const res = await fetch('/api/sessions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          capability,
          llm: settingsForSession(llmSettings),
        }),
      })
      const data = await res.json()
      sid = data.id as string
      setSessionId(sid)
    }

    setMessages((prev) => [...prev, { role: 'user', text }])
    setRunning(true)
    send({ type: 'message', content: text })
  }, [sessionId, capability, llmSettings, send])

  const handleSettingsChange = (nextSettings: typeof llmSettings) => {
    setLlmSettings(nextSettings)
    saveLlmSettings(nextSettings)
    setSessionId(null)
  }

  const handleApproval = (requestId: string, approved: boolean) => {
    send({ type: 'approval_response', request_id: requestId, approved })
    setPendingApproval(null)
  }

  return (
    <div className="flex flex-col h-screen bg-gray-50">
      {/* Header */}
      <header className="bg-white border-b px-6 py-3 flex items-center gap-4">
        <h1 className="text-lg font-semibold text-gray-900">Tutor Agent</h1>
        <nav className="flex border border-gray-200 text-sm">
          <button
            className={`px-4 py-2 ${view === 'chat' ? 'bg-gray-900 text-white' : 'bg-white text-gray-700 hover:bg-gray-50'}`}
            onClick={() => setView('chat')}
          >
            Chat
          </button>
          <button
            className={`px-4 py-2 ${view === 'settings' ? 'bg-gray-900 text-white' : 'bg-white text-gray-700 hover:bg-gray-50'}`}
            onClick={() => setView('settings')}
          >
            Settings
          </button>
        </nav>
        {view === 'chat' && (
          <CapabilitySelector value={capability} onChange={(c) => { setCapability(c); setSessionId(null) }} />
        )}
        <div className="ml-auto">
          <BudgetPanel spent={budgetSpent} limit={llmSettings.budgetLimitUsd} warning={budgetWarning} />
        </div>
      </header>

      {view === 'settings' ? (
        <SettingsPage settings={llmSettings} onChange={handleSettingsChange} />
      ) : (
        <div className="flex flex-1 min-h-0">
          <main className="flex-1">
            <ChatBox
              messages={messages}
              streamingText={streamingText}
              onSend={handleSend}
              disabled={running}
            />
          </main>
          <aside className="w-72 border-l bg-white">
            <TracePanel entries={traceEntries} />
          </aside>
        </div>
      )}

      <ApprovalDialog request={pendingApproval} onDecision={handleApproval} />
    </div>
  )
}
