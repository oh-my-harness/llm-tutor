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
    <div className="flex flex-col h-full">
      <div className="flex-1 overflow-y-auto p-4 space-y-3">
        {messages.map((msg, i) => (
          <div
            key={i}
            className={`rounded-lg p-3 max-w-3xl ${
              msg.role === 'user'
                ? 'bg-blue-100 ml-auto'
                : 'bg-gray-100'
            }`}
          >
            <pre className="whitespace-pre-wrap text-sm font-sans">{msg.text}</pre>
          </div>
        ))}
        {streamingText && (
          <div className="bg-gray-100 rounded-lg p-3 max-w-3xl">
            <pre className="whitespace-pre-wrap text-sm font-sans">{streamingText}</pre>
            <span className="animate-pulse">▌</span>
          </div>
        )}
      </div>
      <div className="border-t p-4 flex gap-2">
        <input
          className="flex-1 border rounded px-3 py-2 text-sm"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && !e.shiftKey && handleSend()}
          placeholder="Ask a question..."
          disabled={disabled}
        />
        <button
          className="bg-blue-600 text-white px-4 py-2 rounded text-sm disabled:opacity-50"
          onClick={handleSend}
          disabled={disabled}
        >
          Send
        </button>
      </div>
    </div>
  )
}
