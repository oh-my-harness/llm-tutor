import { useEffect, useRef, useState } from 'react'
import { ArrowUp, AtSign, Brain, ChevronDown, Database, Paperclip, Sparkles } from 'lucide-react'
import { MarkdownMessage } from './MarkdownMessage'

type Capability = 'chat' | 'deep_solve' | 'code_exec'

interface Message {
  role: 'user' | 'assistant' | 'status'
  text: string
  kind?: 'idle' | 'thinking' | 'tool' | 'done' | 'error'
}

interface Props {
  messages: Message[]
  streamingText: string
  capability: Capability
  modelLabel: string
  knowledgeBases: Array<{ id: string; name: string }>
  selectedKnowledgeBaseId: string
  onSend: (text: string) => void
  onCapabilityChange: (capability: Capability) => void
  onKnowledgeBaseChange: (id: string) => void
  disabled: boolean
}

const modeOptions: Array<{ value: Capability; label: string }> = [
  { value: 'chat', label: '聊天' },
  { value: 'deep_solve', label: '解题' },
  { value: 'code_exec', label: '代码' },
]

export function ChatBox({
  messages,
  streamingText,
  capability,
  modelLabel,
  knowledgeBases,
  selectedKnowledgeBaseId,
  onSend,
  onCapabilityChange,
  onKnowledgeBaseChange,
  disabled,
}: Props) {
  const [input, setInput] = useState('')
  const scrollRef = useRef<HTMLDivElement>(null)
  const shouldStickToBottomRef = useRef(true)
  const empty = messages.length === 0 && !streamingText

  const handleSend = () => {
    if (!input.trim() || disabled) return
    onSend(input.trim())
    setInput('')
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
              modelLabel={modelLabel}
              knowledgeBases={knowledgeBases}
              selectedKnowledgeBaseId={selectedKnowledgeBaseId}
              onCapabilityChange={onCapabilityChange}
              onKnowledgeBaseChange={onKnowledgeBaseChange}
              onSend={handleSend}
              disabled={disabled}
              variant="center"
            />
          </div>
        </div>
      ) : (
        <>
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
                  <MarkdownMessage text={msg.text} />
                ) : (
                  <pre className="whitespace-pre-wrap font-sans text-sm">{msg.text}</pre>
                )}
              </div>
            ))}
            {streamingText && (
              <div className="max-w-3xl rounded-lg bg-gray-100 p-3">
                <MarkdownMessage text={streamingText} />
                <span className="animate-pulse">|</span>
              </div>
            )}
          </div>
          <div className="bg-gray-50 p-4">
            <Composer
              input={input}
              setInput={setInput}
              capability={capability}
              modelLabel={modelLabel}
              knowledgeBases={knowledgeBases}
              selectedKnowledgeBaseId={selectedKnowledgeBaseId}
              onCapabilityChange={onCapabilityChange}
              onKnowledgeBaseChange={onKnowledgeBaseChange}
              onSend={handleSend}
              disabled={disabled}
              variant="bottom"
            />
          </div>
        </>
      )}
    </div>
  )
}

function Composer({
  input,
  setInput,
  capability,
  modelLabel,
  knowledgeBases,
  selectedKnowledgeBaseId,
  onCapabilityChange,
  onKnowledgeBaseChange,
  onSend,
  disabled,
  variant,
}: {
  input: string
  setInput: (value: string) => void
  capability: Capability
  modelLabel: string
  knowledgeBases: Array<{ id: string; name: string }>
  selectedKnowledgeBaseId: string
  onCapabilityChange: (capability: Capability) => void
  onKnowledgeBaseChange: (id: string) => void
  onSend: () => void
  disabled: boolean
  variant: 'center' | 'bottom'
}) {
  return (
    <div
      className={`overflow-hidden rounded-3xl border border-gray-200 bg-white shadow-sm ${
        variant === 'center' ? 'shadow-xl' : ''
      }`}
    >
      <textarea
        className={`${
          variant === 'center' ? 'min-h-36 text-base' : 'min-h-16 text-sm'
        } w-full resize-none px-5 py-4 outline-none placeholder:text-gray-400 disabled:bg-white`}
        value={input}
        onChange={(event) => setInput(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === 'Enter' && !event.shiftKey) {
            event.preventDefault()
            onSend()
          }
        }}
        placeholder="今天我能帮您什么？"
        disabled={disabled}
      />
      <div className="flex flex-wrap items-center gap-2 border-t border-gray-100 px-4 py-2">
        <label className="relative inline-flex items-center">
          <Brain size={18} className="pointer-events-none absolute left-3 text-gray-700" />
          <select
            className="h-9 appearance-none rounded-full border border-gray-200 bg-white pl-9 pr-8 text-sm text-gray-800 outline-none hover:bg-gray-50 disabled:opacity-50"
            value={capability}
            onChange={(event) => onCapabilityChange(event.target.value as Capability)}
            disabled={disabled}
          >
            {modeOptions.map((mode) => (
              <option key={mode.value} value={mode.value}>
                {mode.label}
              </option>
            ))}
          </select>
          <ChevronDown size={16} className="pointer-events-none absolute right-3 text-gray-500" />
        </label>

        <button
          className="inline-flex h-9 items-center gap-2 rounded-full px-3 text-sm text-gray-600 hover:bg-gray-100"
          type="button"
        >
          <Paperclip size={18} />
          附件
        </button>

        <label className="relative inline-flex items-center">
          <Database size={18} className="pointer-events-none absolute left-3 text-gray-600" />
          <select
            className="h-9 max-w-52 appearance-none rounded-full px-9 pr-8 text-sm text-gray-700 outline-none hover:bg-gray-100 disabled:text-gray-400"
            value={selectedKnowledgeBaseId}
            onChange={(event) => onKnowledgeBaseChange(event.target.value)}
            disabled={disabled}
          >
            <option value="">不关联知识库</option>
            {knowledgeBases.map((item) => (
              <option key={item.id} value={item.id}>
                {item.name}
              </option>
            ))}
          </select>
          <ChevronDown size={16} className="pointer-events-none absolute right-3 text-gray-500" />
        </label>

        <button
          className="inline-flex h-9 items-center gap-2 rounded-full px-3 text-sm text-gray-600 hover:bg-gray-100"
          type="button"
        >
          <AtSign size={18} />
          空间
          <ChevronDown size={16} />
        </button>

        <div className="ml-auto flex items-center gap-3">
          <button
            className="inline-flex h-9 max-w-52 items-center gap-2 rounded-full border border-gray-200 px-3 text-sm text-gray-700 hover:bg-gray-50"
            type="button"
          >
            <Brain size={16} />
            <span className="truncate">{modelLabel}</span>
            <ChevronDown size={16} />
          </button>
          <button
            className="flex h-9 w-9 items-center justify-center rounded-full bg-gray-900 text-white disabled:bg-gray-200 disabled:text-gray-400"
            onClick={onSend}
            disabled={disabled || !input.trim()}
            type="button"
            title="发送"
          >
            <ArrowUp size={20} />
          </button>
        </div>
      </div>
    </div>
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
