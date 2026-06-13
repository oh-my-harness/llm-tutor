import { useState, useCallback, useRef } from 'react'
import { CapabilitySelector } from './components/CapabilitySelector'
import { ChatBox } from './components/ChatBox'
import { TracePanel, TraceEntry } from './components/TracePanel'
import { BudgetPanel } from './components/BudgetPanel'
import { ApprovalDialog } from './components/ApprovalDialog'
import { useWebSocket } from './hooks/useWebSocket'

type Capability = 'chat' | 'deep_solve' | 'code_exec'

interface Message {
  role: 'user' | 'assistant'
  text: string
}

export default function App() {
  const [capability, setCapability] = useState<Capability>('chat')
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
        body: JSON.stringify({ capability }),
      })
      const data = await res.json()
      sid = data.id as string
      setSessionId(sid)
    }

    setMessages((prev) => [...prev, { role: 'user', text }])
    setRunning(true)
    send({ type: 'message', content: text })
  }, [sessionId, capability, send])

  const handleApproval = (requestId: string, approved: boolean) => {
    send({ type: 'approval_response', request_id: requestId, approved })
    setPendingApproval(null)
  }

  return (
    <div className="flex flex-col h-screen bg-gray-50">
      {/* Header */}
      <header className="bg-white border-b px-6 py-3 flex items-center gap-4">
        <h1 className="text-lg font-semibold text-gray-900">Tutor Agent</h1>
        <CapabilitySelector value={capability} onChange={(c) => { setCapability(c); setSessionId(null) }} />
        <div className="ml-auto">
          <BudgetPanel spent={budgetSpent} limit={2.0} warning={budgetWarning} />
        </div>
      </header>

      {/* Main */}
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

      <ApprovalDialog request={pendingApproval} onDecision={handleApproval} />
    </div>
  )
}
