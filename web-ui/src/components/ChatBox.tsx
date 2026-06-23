import { useEffect, useRef, useState } from 'react'
import type { ReactNode } from 'react'
import {
  ArrowUp,
  AtSign,
  Brain,
  CheckCircle2,
  ChevronDown,
  Code2,
  Database,
  MessageSquare,
  Paperclip,
  Sparkles,
} from 'lucide-react'
import type { LlmModelConfig } from '../settings'
import { DeepSolveMessage, type DeepSolveTraceEntry } from './DeepSolveMessage'
import { MarkdownMessage } from './MarkdownMessage'

type Capability = 'chat' | 'deep_solve' | 'code_exec'
type OpenMenu = 'mode' | 'knowledge' | 'model' | null

interface Message {
  role: 'user' | 'assistant' | 'status'
  text: string
  kind?: 'idle' | 'thinking' | 'tool' | 'done' | 'error'
  citations?: Citation[]
  deepSolve?: DeepSolveTraceEntry[]
}

interface Citation {
  index: number
  source: string
  text: string
  score?: number | null
}

interface Props {
  messages: Message[]
  streamingText: string
  capability: Capability
  llmConfigs: LlmModelConfig[]
  activeLlmConfigId: string | null
  knowledgeBases: Array<{ id: string; name: string }>
  selectedKnowledgeBaseId: string
  onSend: (text: string) => void
  onAskDeepSolveStep?: (step: { id: string; title: string; summary?: string }) => void
  onCapabilityChange: (capability: Capability) => void
  onKnowledgeBaseChange: (id: string) => void
  onLlmConfigChange: (id: string) => void
  disabled: boolean
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
]

export function ChatBox({
  messages,
  streamingText,
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
              llmConfigs={llmConfigs}
              activeLlmConfigId={activeLlmConfigId}
              knowledgeBases={knowledgeBases}
              selectedKnowledgeBaseId={selectedKnowledgeBaseId}
              onCapabilityChange={onCapabilityChange}
              onKnowledgeBaseChange={onKnowledgeBaseChange}
              onLlmConfigChange={onLlmConfigChange}
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
                  msg.deepSolve && msg.deepSolve.length > 0 ? (
                    <DeepSolveMessage
                      text={msg.text}
                      events={msg.deepSolve}
                      citations={msg.citations}
                      citationList={(citations) => <CitationList citations={citations} />}
                      onAskStep={onAskDeepSolveStep}
                    />
                  ) : (
                    <>
                      <MarkdownMessage text={msg.text} />
                      {msg.citations && msg.citations.length > 0 && (
                        <CitationList citations={msg.citations} />
                      )}
                    </>
                  )
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
              llmConfigs={llmConfigs}
              activeLlmConfigId={activeLlmConfigId}
              knowledgeBases={knowledgeBases}
              selectedKnowledgeBaseId={selectedKnowledgeBaseId}
              onCapabilityChange={onCapabilityChange}
              onKnowledgeBaseChange={onKnowledgeBaseChange}
              onLlmConfigChange={onLlmConfigChange}
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

function CitationList({ citations }: { citations: Citation[] }) {
  return (
    <div className="mt-3 border-t border-gray-200 pt-3">
      <div className="mb-2 text-xs font-medium text-gray-500">引用来源</div>
      <div className="space-y-2">
        {citations.map((citation, index) => (
          <details key={`${citation.source}-${index}`} className="rounded-md border border-blue-100 bg-white/70 p-2">
            <summary className="cursor-pointer text-xs font-medium text-blue-700">
              [{citation.index || index + 1}] {citation.source}
              {typeof citation.score === 'number' ? ` · ${citation.score.toFixed(4)}` : ''}
            </summary>
            <p className="mt-2 max-h-20 overflow-hidden text-xs leading-5 text-gray-600">{citation.text}</p>
          </details>
        ))}
      </div>
    </div>
  )
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
  disabled: boolean
  variant: 'center' | 'bottom'
}) {
  const [openMenu, setOpenMenu] = useState<OpenMenu>(null)
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
          className="inline-flex h-9 items-center gap-2 rounded-full px-3 text-sm text-gray-600 hover:bg-blue-50"
          type="button"
        >
          <Paperclip size={18} />
          附件
        </button>

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
          disabled={disabled || !input.trim()}
          type="button"
          title="发送"
        >
          <ArrowUp size={20} />
        </button>
      </div>
    </div>
  )
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
