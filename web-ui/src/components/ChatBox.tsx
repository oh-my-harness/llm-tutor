import { useState } from 'react'

interface Message {
  role: 'user' | 'assistant'
  text: string
}

interface Props {
  messages: Message[]
  streamingText: string
  onSend: (text: string) => void
  disabled: boolean
}

export function ChatBox({ messages, streamingText, onSend, disabled }: Props) {
  const [input, setInput] = useState('')

  const handleSend = () => {
    if (!input.trim() || disabled) return
    onSend(input.trim())
    setInput('')
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex-1 space-y-3 overflow-y-auto p-4">
        {messages.map((msg, i) => (
          <div
            key={i}
            className={`max-w-3xl rounded-lg p-3 ${
              msg.role === 'user' ? 'ml-auto bg-blue-100' : 'bg-gray-100'
            }`}
          >
            <pre className="whitespace-pre-wrap font-sans text-sm">{msg.text}</pre>
          </div>
        ))}
        {streamingText && (
          <div className="max-w-3xl rounded-lg bg-gray-100 p-3">
            <pre className="whitespace-pre-wrap font-sans text-sm">{streamingText}</pre>
            <span className="animate-pulse">|</span>
          </div>
        )}
      </div>
      <div className="flex gap-2 border-t p-4">
        <input
          className="flex-1 rounded border px-3 py-2 text-sm"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && !e.shiftKey && handleSend()}
          placeholder="Ask a question..."
          disabled={disabled}
        />
        <button
          className="rounded bg-blue-600 px-4 py-2 text-sm text-white disabled:opacity-50"
          onClick={handleSend}
          disabled={disabled}
        >
          Send
        </button>
      </div>
    </div>
  )
}
