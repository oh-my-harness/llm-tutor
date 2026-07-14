import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  ArrowLeft,
  Bot,
  Brain,
  Check,
  CheckCircle2,
  ChevronDown,
  FileText,
  GitBranch,
  Layers3,
  RefreshCw,
  RotateCcw,
  Save,
  Send,
  Sparkles,
  Split,
  Wand2,
} from 'lucide-react'
import { MarkdownMessage } from './MarkdownMessage'
import type { SourceReference, SourceTarget } from './MarkdownMessage'
import { settingsForSession, type LlmSettings } from '../settings'

type Layer = 'overview' | 'L2' | 'L3'
type AssistAction = 'update' | 'check' | 'dedupe'
type ViewMode = 'rendered' | 'source'
type MemoryEdit = {
  op: 'replace' | 'delete' | 'insert'
  start_line: number
  end_line?: number | null
  text?: string | null
  refs?: string[]
  reason?: string | null
}
type MemoryFact = {
  text: string
  section: string
  refs: string[]
}
type TraceChunk = {
  index: number
  total: number
  citeableRefs?: string[]
  status: string
}
type MemoryModelOption = {
  id: string
  label: string
  model: string
  configured: boolean
}

interface MemoryFile {
  path: string
  level: string
  name: string
  markdown: string
}

interface AssistResult {
  target_path: string
  action: AssistAction
  report_markdown: string
  proposed_markdown?: string | null
  facts?: MemoryFact[]
  edits?: MemoryEdit[]
  trace?: {
    input_json: string
    output_json: string
    chunks?: TraceChunk[]
  } | null
  changed: boolean
}

const l2Paths = ['L2/chat.md', 'L2/quiz.md', 'L2/notebook.md', 'L2/knowledge.md', 'L2/research.md']
const l3Paths = ['L3/recent.md', 'L3/profile.md', 'L3/scope.md', 'L3/preferences.md', 'L3/teaching_strategy.md']

export function MemoryPage({
  settings,
  onSourceNavigate,
}: {
  settings: LlmSettings
  onSourceNavigate?: (target: SourceTarget, reference: SourceReference) => void
}) {
  const [files, setFiles] = useState<MemoryFile[]>([])
  const [layer, setLayer] = useState<Layer>('overview')
  const [activePath, setActivePath] = useState<string | null>(null)
  const [draft, setDraft] = useState('')
  const [viewMode, setViewMode] = useState<ViewMode>('rendered')
  const [assistAction, setAssistAction] = useState<AssistAction>('update')
  const [selectedModelId, setSelectedModelId] = useState(
    settings.activeLlmConfigId ?? '__default__',
  )
  const [assistResult, setAssistResult] = useState<AssistResult | null>(null)
  const [status, setStatus] = useState('Loading memory...')
  const [loading, setLoading] = useState(false)

  const activeFile = useMemo(
    () => files.find((file) => file.path === activePath) ?? null,
    [activePath, files],
  )
  const l2Files = useMemo(() => l2Paths.map((path) => files.find((file) => file.path === path)).filter(Boolean) as MemoryFile[], [files])
  const l3Files = useMemo(() => l3Paths.map((path) => files.find((file) => file.path === path)).filter(Boolean) as MemoryFile[], [files])
  const modelOptions = useMemo(() => memoryModelOptions(settings), [settings])

  useEffect(() => {
    if (modelOptions.some((option) => option.id === selectedModelId)) return
    const activeId = settings.activeLlmConfigId
    const preferredId = activeId && modelOptions.some((option) => option.id === activeId)
      ? activeId
      : modelOptions[0]?.id ?? '__default__'
    setSelectedModelId(preferredId)
  }, [modelOptions, selectedModelId, settings.activeLlmConfigId])

  const loadFiles = useCallback(async () => {
    setLoading(true)
    try {
      const res = await fetch('/api/memory/files')
      const data = await safeJson(res)
      if (!res.ok) throw new Error(errorMessage(data, res.status))
      const nextFiles = (data.files ?? []) as MemoryFile[]
      setFiles(nextFiles)
      setStatus('Memory files loaded')
      setActivePath((current) => current && nextFiles.some((file) => file.path === current) ? current : null)
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    void loadFiles()
  }, [loadFiles])

  useEffect(() => {
    if (!activeFile) return
    setDraft(activeFile.markdown)
    setAssistResult(null)
  }, [activeFile?.path, activeFile?.markdown])

  const openLayer = (nextLayer: 'L2' | 'L3') => {
    const firstPath = (nextLayer === 'L2' ? l2Paths[0] : l3Paths[0]) ?? null
    setLayer(nextLayer)
    setActivePath((current) => current && current.startsWith(`${nextLayer}/`) ? current : firstPath)
    setAssistResult(null)
  }

  const saveActiveFile = async () => {
    if (!activeFile) return
    if (!draft.trim()) {
      setStatus('Memory markdown is empty')
      return
    }
    setLoading(true)
    try {
      const res = await fetch(`/api/memory/file?path=${encodeURIComponent(activeFile.path)}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ markdown: draft }),
      })
      const data = await safeJson(res)
      if (!res.ok) throw new Error(errorMessage(data, res.status))
      const updated = data.file as MemoryFile
      setFiles((items) => items.map((item) => item.path === updated.path ? updated : item))
      setStatus('Memory saved')
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }

  const undoActiveFile = async () => {
    if (!activeFile) return
    setLoading(true)
    try {
      const res = await fetch('/api/memory/undo', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ target_path: activeFile.path }),
      })
      const data = await safeJson(res)
      if (!res.ok) throw new Error(errorMessage(data, res.status))
      const updated = (data.result as { file?: MemoryFile }).file
      if (!updated) throw new Error('Undo response did not include a memory file')
      setFiles((items) => items.map((item) => item.path === updated.path ? updated : item))
      setDraft(updated.markdown)
      setAssistResult(null)
      setStatus('Memory restored from latest snapshot')
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }

  const runAssist = async () => {
    if (!activeFile) return
    const configId = selectedModelId === '__default__' ? null : selectedModelId
    const llm = settingsForSession(settings, configId)
    if (!llm.model || !llm.api_key) {
      setStatus('请先在设置中配置可用的 LLM，记忆维护需要模型参与。')
      return
    }
    setStatus(`正在${assistActionLabel(assistAction)}记忆…`)
    setLoading(true)
    try {
      const res = await fetch('/api/memory/assist', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          target_path: activeFile.path,
          action: assistAction,
          markdown: draft,
          llm,
        }),
      })
      const data = await safeJson(res)
      if (!res.ok) throw new Error(errorMessage(data, res.status))
      const result = data.result as AssistResult
      setAssistResult(result)
      setStatus(`${assistActionLabel(assistAction)}记忆完成`)
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    } finally {
      setLoading(false)
    }
  }

  return (
    <main className="flex h-full min-h-0 bg-white">
      <section className="flex min-w-0 flex-1 flex-col">
        {layer === 'overview' ? (
          <Overview
            files={files}
            status={status}
            loading={loading}
            onRefresh={() => void loadFiles()}
            onOpenLayer={openLayer}
          />
        ) : (
          <LayerWorkspace
            layer={layer}
            files={layer === 'L2' ? l2Files : l3Files}
            activeFile={activeFile}
            draft={draft}
            viewMode={viewMode}
            loading={loading}
            status={status}
            assistResult={assistResult}
            assistAction={assistAction}
            modelOptions={modelOptions}
            selectedModelId={selectedModelId}
            onBack={() => setLayer('overview')}
            onSelectFile={setActivePath}
            onDraftChange={setDraft}
            onViewModeChange={setViewMode}
            onSave={() => void saveActiveFile()}
            onUndo={() => void undoActiveFile()}
            onAssistActionChange={setAssistAction}
            onSelectedModelChange={setSelectedModelId}
            onRunAssist={() => void runAssist()}
            onSourceNavigate={onSourceNavigate}
            onApplyProposal={(markdown) => {
              setDraft(markdown)
              setStatus('Memory draft updated from agent proposal')
            }}
            onReset={() => {
              setDraft(activeFile?.markdown ?? '')
              setAssistResult(null)
              setAssistAction('update')
              setSelectedModelId(settings.activeLlmConfigId ?? modelOptions[0]?.id ?? '__default__')
            }}
          />
        )}
      </section>
    </main>
  )
}

function Overview({
  files,
  status,
  loading,
  onRefresh,
  onOpenLayer,
}: {
  files: MemoryFile[]
  status: string
  loading: boolean
  onRefresh: () => void
  onOpenLayer: (layer: 'L2' | 'L3') => void
}) {
  const l2Count = files.filter((file) => file.level === 'L2' && hasRealMemory(file.markdown)).length
  const l3Count = files.filter((file) => file.level === 'L3' && hasRealMemory(file.markdown)).length
  return (
    <div className="flex-1 overflow-y-auto px-12 py-10">
      <div className="flex items-start gap-4">
        <div className="mt-1 text-blue-600">
          <Brain size={32} />
        </div>
        <div>
          <h1 className="text-4xl font-semibold tracking-tight text-gray-950">记忆</h1>
          <p className="mt-4 text-base text-gray-500">
            DeepTutor 关于你的可见记忆。L1 保存在工作区事件中，这里主要整理 L2 和 L3。
          </p>
          <div className="mt-5 flex items-center gap-3">
            <button className={secondaryButtonClassName} disabled={loading} onClick={onRefresh} type="button">
              <RefreshCw size={16} className={loading ? 'animate-spin' : ''} />
              刷新
            </button>
            <span className="text-sm text-gray-500">{status}</span>
          </div>
        </div>
      </div>

      <div className="mt-12 grid gap-5 lg:grid-cols-3">
        <MemoryLayerCard
          icon={Layers3}
          title="L1 · 工作区镜像"
          badge="实时"
          countLabel="条 workspace 事件"
          count="本地"
          description="聊天、测验、笔记本和研究事件已存储在工作区中，不在这里单独可视化。"
        />
        <MemoryLayerCard
          icon={GitBranch}
          title="L2 · 各模块摘要"
          badge="整理后"
          count={String(l2Count)}
          countLabel={`个活跃摘要，共 ${l2Paths.length} 个 surface`}
          description="按聊天、测验、笔记本、研究分层整理。每个文档支持更新、检查和去重。"
          actionLabel="进入 L2"
          onClick={() => onOpenLayer('L2')}
        />
        <MemoryLayerCard
          icon={Split}
          title="L3 · 跨模块知识"
          badge="综合"
          count={String(l3Count)}
          countLabel={`个活跃 slot，共 ${l3Paths.length} 个 slot`}
          description="综合最近状态、学生画像、范围、偏好和教学策略。Agent 会读取这里做个性化。"
          actionLabel="进入 L3"
          onClick={() => onOpenLayer('L3')}
        />
      </div>
    </div>
  )
}

function MemoryLayerCard({
  icon: Icon,
  title,
  badge,
  count,
  countLabel,
  description,
  actionLabel,
  onClick,
}: {
  icon: typeof Brain
  title: string
  badge: string
  count: string
  countLabel: string
  description: string
  actionLabel?: string
  onClick?: () => void
}) {
  return (
    <button
      className={`min-h-72 rounded-lg border border-gray-200 bg-white p-6 text-left shadow-sm transition ${
        onClick ? 'hover:border-blue-200 hover:bg-blue-50/40' : 'cursor-default'
      }`}
      type="button"
      onClick={onClick}
    >
      <div className="flex items-center justify-between">
        <Icon size={22} className="text-blue-600" />
        <span className="rounded-full border border-gray-200 bg-gray-50 px-2.5 py-1 text-xs text-gray-500">{badge}</span>
      </div>
      <h2 className="mt-10 text-xl font-semibold text-gray-950">{title}</h2>
      <div className="mt-8 flex items-end gap-3">
        <span className="text-4xl font-semibold text-gray-950">{count}</span>
        <span className="pb-1 text-sm text-gray-500">{countLabel}</span>
      </div>
      <p className="mt-8 text-sm leading-6 text-gray-500">{description}</p>
      {actionLabel && <div className="mt-8 text-sm font-medium text-blue-700">{actionLabel} →</div>}
    </button>
  )
}

function LayerWorkspace({
  layer,
  files,
  activeFile,
  draft,
  viewMode,
  loading,
  status,
  assistResult,
  assistAction,
  modelOptions,
  selectedModelId,
  onBack,
  onSelectFile,
  onDraftChange,
  onViewModeChange,
  onSave,
  onUndo,
  onAssistActionChange,
  onSelectedModelChange,
  onRunAssist,
  onSourceNavigate,
  onApplyProposal,
  onReset,
}: {
  layer: 'L2' | 'L3'
  files: MemoryFile[]
  activeFile: MemoryFile | null
  draft: string
  viewMode: ViewMode
  loading: boolean
  status: string
  assistResult: AssistResult | null
  assistAction: AssistAction
  modelOptions: MemoryModelOption[]
  selectedModelId: string
  onBack: () => void
  onSelectFile: (path: string) => void
  onDraftChange: (value: string) => void
  onViewModeChange: (mode: ViewMode) => void
  onSave: () => void
  onUndo: () => void
  onAssistActionChange: (action: AssistAction) => void
  onSelectedModelChange: (id: string) => void
  onRunAssist: () => void
  onSourceNavigate?: (target: SourceTarget, reference: SourceReference) => void
  onApplyProposal: (markdown: string) => void
  onReset: () => void
}) {
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex items-center gap-2 px-5 py-3">
        <button className="rounded p-1.5 text-gray-500 hover:bg-gray-100" type="button" onClick={onBack} aria-label="返回记忆概览" title="返回记忆概览">
          <ArrowLeft size={18} />
        </button>
        <div className="text-sm text-gray-500">记忆</div>
        <span className="text-gray-300">/</span>
        <div className="text-sm font-medium text-gray-900">{layer} · {layer === 'L2' ? '各模块摘要' : '跨模块知识'}</div>
        <div className="ml-auto inline-flex rounded-md border border-gray-200 bg-white p-0.5">
          {(['L2', 'L3'] as const).map((item) => (
            <button
              key={item}
              className={`rounded px-2.5 py-1 text-xs ${item === layer ? 'bg-blue-50 text-blue-700' : 'text-gray-500'}`}
              type="button"
              disabled
            >
              {item}
            </button>
          ))}
        </div>
      </header>

      <div className="grid min-h-0 flex-1 grid-cols-[180px_minmax(0,1fr)_280px] gap-3 px-5 pb-5">
        <aside className="min-h-0 overflow-y-auto rounded-md border border-gray-200 bg-white p-2">
          <div className="space-y-0.5">
            {files.map((file) => (
              <button
                key={file.path}
                className={`flex w-full items-center gap-2 rounded-md px-2 py-2 text-left text-sm ${
                  activeFile?.path === file.path ? 'bg-blue-50 text-blue-700' : 'text-gray-700 hover:bg-gray-50'
                }`}
                type="button"
                onClick={() => onSelectFile(file.path)}
              >
                <FileText size={15} className="shrink-0" />
                <span className="min-w-0 flex-1">
                  <span className="block truncate font-medium">{memoryFileLabel(file.path)}</span>
                  <span className="block truncate text-[11px] text-gray-400">{lineCount(file.markdown)} 行</span>
                </span>
                {hasRealMemory(file.markdown) && <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-blue-500" title="包含记忆内容" />}
              </button>
            ))}
          </div>
        </aside>

        <section className="flex min-h-0 flex-col rounded-md border border-gray-200 bg-white">
          <div className="flex flex-wrap items-center gap-2 border-b border-gray-100 px-4 py-2.5">
            <div className="mr-auto min-w-36">
              <h2 className="truncate text-base font-semibold text-gray-950">{activeFile ? memoryFileLabel(activeFile.path) : 'Memory file'}</h2>
              <p className="text-xs text-gray-500">{activeFile?.path ?? status}</p>
            </div>
            <div className="inline-flex rounded-md border border-gray-200 bg-gray-50 p-0.5">
              <button className={modeButtonClassName(viewMode === 'rendered')} type="button" onClick={() => onViewModeChange('rendered')}>渲染视图</button>
              <button className={modeButtonClassName(viewMode === 'source')} type="button" onClick={() => onViewModeChange('source')}>带行号</button>
            </div>
            <button className={compactIconButtonClassName} type="button" disabled={loading || !activeFile || draft === activeFile.markdown} onClick={onSave} aria-label="保存记忆" title="保存记忆">
              <Save size={16} />
            </button>
            <button className={compactIconButtonClassName} type="button" disabled={loading || !activeFile} onClick={onUndo} aria-label="撤销上次保存" title="撤销上次保存">
              <RotateCcw size={16} />
            </button>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto p-3">
            {viewMode === 'source' ? (
              <textarea
                className="min-h-full w-full resize-none rounded-md border border-gray-100 bg-gray-50 p-5 font-mono text-sm leading-7 text-gray-800 outline-none focus:border-blue-300 focus:ring-2 focus:ring-blue-50"
                value={draft}
                onChange={(event) => onDraftChange(event.target.value)}
              />
            ) : (
              <article className="mx-auto min-h-full w-full max-w-5xl px-5 py-3">
                <MarkdownMessage text={draft || ' '} onSourceNavigate={onSourceNavigate} />
              </article>
            )}
          </div>
        </section>

        <AgentWorkspace
          loading={loading}
          assistResult={assistResult}
          assistAction={assistAction}
          modelOptions={modelOptions}
          selectedModelId={selectedModelId}
          canRun={Boolean(activeFile)}
          status={status}
          onAssistActionChange={onAssistActionChange}
          onSelectedModelChange={onSelectedModelChange}
          onRunAssist={onRunAssist}
          onApplyProposal={onApplyProposal}
          onReset={onReset}
        />
      </div>
    </div>
  )
}

function AgentWorkspace({
  loading,
  assistResult,
  assistAction,
  modelOptions,
  selectedModelId,
  canRun,
  status,
  onAssistActionChange,
  onSelectedModelChange,
  onRunAssist,
  onApplyProposal,
  onReset,
}: {
  loading: boolean
  assistResult: AssistResult | null
  assistAction: AssistAction
  modelOptions: MemoryModelOption[]
  selectedModelId: string
  canRun: boolean
  status: string
  onAssistActionChange: (action: AssistAction) => void
  onSelectedModelChange: (id: string) => void
  onRunAssist: () => void
  onApplyProposal: (markdown: string) => void
  onReset: () => void
}) {
  const selectedModel = modelOptions.find((option) => option.id === selectedModelId)
  return (
    <aside className="flex min-h-0 flex-col rounded-md border border-gray-200 bg-white">
      <div className="flex items-center gap-2 border-b border-gray-100 px-3 py-2.5">
        <Bot size={17} className="text-blue-600" />
        <div className="text-sm font-semibold text-gray-950">LLM 工作区</div>
        <button className="ml-auto inline-flex items-center gap-1 rounded-md px-2 py-1 text-xs text-gray-500 hover:bg-gray-50 hover:text-gray-900" type="button" onClick={onReset} disabled={loading}>
          <RotateCcw size={13} />
          重置
        </button>
      </div>
      <div className="space-y-2.5 border-b border-gray-100 p-3">
        <div className="grid grid-cols-3 gap-1">
          <AssistModeButton icon={Wand2} label="更新" active={assistAction === 'update'} disabled={loading} onClick={() => onAssistActionChange('update')} />
          <AssistModeButton icon={CheckCircle2} label="检查" active={assistAction === 'check'} disabled={loading} onClick={() => onAssistActionChange('check')} />
          <AssistModeButton icon={GitBranch} label="去重" active={assistAction === 'dedupe'} disabled={loading} onClick={() => onAssistActionChange('dedupe')} />
        </div>
        <ModelPicker
          options={modelOptions}
          selectedId={selectedModelId}
          disabled={loading}
          onSelect={onSelectedModelChange}
        />
        <button
          className="inline-flex h-9 w-full items-center justify-center gap-2 rounded-md bg-blue-600 text-sm font-medium text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:bg-gray-200"
          type="button"
          disabled={loading || !canRun || !selectedModel?.configured}
          onClick={onRunAssist}
        >
          <Send size={15} />
          {loading ? '运行中…' : `运行${assistActionLabel(assistAction)}`}
        </button>
        <div className="truncate text-[11px] text-gray-400" title={status} aria-live="polite">
          {selectedModel?.configured ? status : '该模型缺少名称或 API Key，请先到设置中完善'}
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto p-3">
        {assistResult ? (
          <article className="rounded-md border border-blue-100 bg-blue-50/40 p-3">
            <MarkdownMessage text={assistResult.report_markdown} />
            {assistResult.facts && assistResult.facts.length > 0 && (
              <FactPreview facts={assistResult.facts} />
            )}
            {assistResult.edits && assistResult.edits.length > 0 && (
              <EditPreview edits={assistResult.edits} />
            )}
            {assistResult.trace && <TracePreview trace={assistResult.trace} />}
            {assistResult.changed && (
              <p className="mt-4 text-sm text-blue-700">已生成可预览的记忆变更，请检查 edits 和报告后再应用到草稿。</p>
            )}
            {assistResult.proposed_markdown && (
              <button
                className="mt-4 inline-flex h-9 items-center justify-center gap-2 rounded-lg bg-blue-600 px-3 text-sm font-medium text-white hover:bg-blue-700"
                type="button"
                onClick={() => onApplyProposal(assistResult.proposed_markdown ?? '')}
              >
                <Wand2 size={15} />
                应用到草稿
              </button>
            )}
          </article>
        ) : (
          <div className="flex h-full flex-col items-center justify-center text-center text-sm leading-6 text-gray-500">
            <Sparkles size={24} className="mb-3 text-gray-300" />
            选择模式和模型后运行。结果会显示在这里，变更保存前仍可检查。
          </div>
        )}
      </div>
    </aside>
  )
}

function ModelPicker({
  options,
  selectedId,
  disabled,
  onSelect,
}: {
  options: MemoryModelOption[]
  selectedId: string
  disabled: boolean
  onSelect: (id: string) => void
}) {
  const [open, setOpen] = useState(false)
  const selected = options.find((option) => option.id === selectedId) ?? options[0]

  return (
    <div
      className="relative"
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) setOpen(false)
      }}
      onKeyDown={(event) => {
        if (event.key === 'Escape') setOpen(false)
      }}
    >
      <button
        className="flex min-h-11 w-full items-center gap-2 rounded-md border border-gray-200 bg-white px-2.5 py-2 text-left outline-none transition hover:border-blue-300 hover:bg-blue-50/40 focus:border-blue-400 focus:ring-2 focus:ring-blue-100 disabled:cursor-not-allowed disabled:opacity-60"
        type="button"
        aria-haspopup="listbox"
        aria-expanded={open}
        disabled={disabled}
        onClick={() => setOpen((value) => !value)}
      >
        <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-md bg-gray-100 text-gray-600">
          <Bot size={14} />
        </span>
        <span className="min-w-0 flex-1">
          <span className="block truncate text-xs font-medium text-gray-900">
            {selected?.label ?? '选择模型'}
          </span>
          <span className={`mt-0.5 block truncate text-[11px] ${selected?.configured ? 'text-gray-400' : 'text-red-600'}`}>
            {selected?.configured ? selected.model : '配置不完整'}
          </span>
        </span>
        <ChevronDown
          size={15}
          className={`shrink-0 text-gray-400 transition-transform ${open ? 'rotate-180' : ''}`}
        />
      </button>

      {open && (
        <div
          className="absolute left-0 right-0 top-full z-30 mt-1 max-h-56 overflow-y-auto rounded-md border border-gray-200 bg-white p-1 shadow-lg"
          role="listbox"
          aria-label="运行模型"
        >
          {options.map((option) => {
            const active = option.id === selectedId
            return (
              <button
                key={option.id}
                className={`flex w-full items-center gap-2 rounded px-2 py-2 text-left ${
                  active ? 'bg-blue-50' : 'hover:bg-gray-50'
                }`}
                type="button"
                role="option"
                aria-selected={active}
                onClick={() => {
                  onSelect(option.id)
                  setOpen(false)
                }}
              >
                <span className="min-w-0 flex-1">
                  <span className={`block truncate text-xs font-medium ${active ? 'text-blue-700' : 'text-gray-800'}`}>
                    {option.label}
                  </span>
                  <span className={`mt-0.5 block truncate text-[11px] ${option.configured ? 'text-gray-400' : 'text-red-600'}`}>
                    {option.configured ? option.model : `${option.model || '未配置模型'} · 缺少 API Key`}
                  </span>
                </span>
                {active && <Check size={14} className="shrink-0 text-blue-600" />}
              </button>
            )
          })}
        </div>
      )}
    </div>
  )
}

function FactPreview({ facts }: { facts: MemoryFact[] }) {
  return (
    <div className="mt-4 rounded-lg border border-blue-100 bg-white p-3">
      <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-gray-500">Proposed facts</div>
      <div className="space-y-2">
        {facts.map((fact, index) => (
          <div key={`${fact.section}-${index}`} className="rounded-md border border-gray-100 bg-gray-50 p-2 text-xs text-gray-700">
            <div className="mb-1 font-medium text-gray-900">{fact.section}</div>
            <div className="leading-5">{fact.text}</div>
            {fact.refs.length > 0 && (
              <div className="mt-2 flex flex-wrap gap-1">
                {fact.refs.map((ref) => (
                  <span key={ref} className="rounded bg-blue-50 px-1.5 py-0.5 font-mono text-[11px] text-blue-700">{ref}</span>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  )
}

function EditPreview({ edits }: { edits: MemoryEdit[] }) {
  return (
    <div className="mt-4 rounded-lg border border-blue-100 bg-white p-3">
      <div className="mb-2 text-xs font-semibold uppercase tracking-wide text-gray-500">Line edits</div>
      <div className="space-y-2">
        {edits.map((edit, index) => (
          <div key={`${edit.op}-${edit.start_line}-${index}`} className="rounded-md border border-gray-100 bg-gray-50 p-2 text-xs text-gray-700">
            <div className="flex items-center gap-2">
              <span className={`rounded px-1.5 py-0.5 font-medium ${editBadgeClassName(edit.op)}`}>{edit.op}</span>
              <span>{formatEditRange(edit)}</span>
            </div>
            {edit.reason && (
              <div className="mt-2 text-gray-500">{edit.reason}</div>
            )}
            {edit.refs && edit.refs.length > 0 && (
              <div className="mt-2 flex flex-wrap gap-1">
                {edit.refs.map((ref) => (
                  <span key={ref} className="rounded bg-blue-50 px-1.5 py-0.5 font-mono text-[11px] text-blue-700">{ref}</span>
                ))}
              </div>
            )}
            {edit.text && (
              <pre className="mt-2 max-h-28 overflow-auto whitespace-pre-wrap rounded bg-white p-2 font-mono text-[11px] leading-5 text-gray-700">
                {edit.text}
              </pre>
            )}
          </div>
        ))}
      </div>
    </div>
  )
}

function TracePreview({ trace }: { trace: NonNullable<AssistResult['trace']> }) {
  return (
    <details className="mt-4 rounded-lg border border-gray-200 bg-white p-3">
      <summary className="cursor-pointer text-xs font-semibold uppercase tracking-wide text-gray-500">
        LLM trace
      </summary>
      <div className="mt-3 space-y-3">
        {trace.chunks && trace.chunks.length > 0 && (
          <div className="rounded-md border border-blue-100 bg-blue-50/60 p-2">
            <div className="mb-2 text-xs font-medium text-blue-700">Chunks</div>
            <div className="space-y-1">
              {trace.chunks.map((chunk) => (
                <div key={`${chunk.index}-${chunk.total}`} className="text-xs text-gray-600">
                  <span className="font-medium text-gray-800">
                    {chunk.index}/{chunk.total}
                  </span>
                  <span className="ml-2">{chunk.status}</span>
                  {chunk.citeableRefs && chunk.citeableRefs.length > 0 && (
                    <span className="ml-2 font-mono text-[11px] text-blue-700">
                      {chunk.citeableRefs.join(', ')}
                    </span>
                  )}
                </div>
              ))}
            </div>
          </div>
        )}
        <TraceBlock title="Input" value={trace.input_json} />
        <TraceBlock title="Output" value={trace.output_json} />
      </div>
    </details>
  )
}

function TraceBlock({ title, value }: { title: string; value: string }) {
  return (
    <div>
      <div className="mb-1 text-xs font-medium text-gray-500">{title}</div>
      <pre className="max-h-56 overflow-auto rounded-md bg-gray-950 p-3 font-mono text-[11px] leading-5 text-gray-100">
        {value}
      </pre>
    </div>
  )
}

function AssistModeButton({
  icon: Icon,
  label,
  active,
  disabled,
  onClick,
}: {
  icon: typeof Brain
  label: string
  active: boolean
  disabled: boolean
  onClick: () => void
}) {
  return (
    <button
      className={`inline-flex h-8 items-center justify-center gap-1 rounded-md border px-1 text-xs font-medium disabled:opacity-50 ${
        active
          ? 'border-blue-300 bg-blue-50 text-blue-700'
          : 'border-gray-200 bg-white text-gray-600 hover:bg-gray-50 hover:text-gray-900'
      }`}
      disabled={disabled}
      type="button"
      aria-pressed={active}
      onClick={onClick}
    >
      <Icon size={13} />
      {label}
    </button>
  )
}

function memoryModelOptions(settings: LlmSettings): MemoryModelOption[] {
  if (settings.llmConfigs.length > 0) {
    return settings.llmConfigs.map((config) => ({
      id: config.id,
      label: config.name || config.model || '未命名模型',
      model: config.model,
      configured: Boolean(config.model.trim() && config.apiKey.trim()),
    }))
  }
  return [{
    id: '__default__',
    label: settings.model || '默认模型',
    model: settings.model,
    configured: Boolean(settings.model.trim() && settings.apiKey.trim()),
  }]
}

function formatEditRange(edit: MemoryEdit) {
  const end = edit.end_line ?? edit.start_line
  return end === edit.start_line ? `line ${edit.start_line}` : `lines ${edit.start_line}-${end}`
}

function editBadgeClassName(op: MemoryEdit['op']) {
  if (op === 'delete') return 'bg-red-50 text-red-700'
  if (op === 'replace') return 'bg-amber-50 text-amber-700'
  return 'bg-green-50 text-green-700'
}

function memoryFileLabel(path: string) {
  const labels: Record<string, string> = {
    'L2/chat.md': '聊天',
    'L2/quiz.md': '测验',
    'L2/notebook.md': '笔记本',
    'L2/research.md': '研究',
    'L3/recent.md': '近期状态',
    'L3/profile.md': '学生画像',
    'L3/scope.md': '学习范围',
    'L3/preferences.md': '学习偏好',
    'L3/teaching_strategy.md': '教学策略',
  }
  return labels[path] ?? path
}

function hasRealMemory(markdown: string) {
  return markdown
    .split(/\r?\n/)
    .some((line) => line.trim().startsWith('- '))
}

function lineCount(markdown: string) {
  return markdown.split(/\r?\n/).length
}

function modeButtonClassName(active: boolean) {
  return `rounded px-2 py-1 text-xs ${active ? 'bg-white text-gray-950 shadow-sm' : 'text-gray-500 hover:text-gray-900'}`
}

function assistActionLabel(action: AssistAction) {
  if (action === 'update') return '更新'
  if (action === 'check') return '检查'
  return '去重'
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

const secondaryButtonClassName = 'inline-flex h-9 items-center justify-center gap-2 rounded-lg border border-gray-200 px-3.5 text-sm font-medium text-gray-700 hover:bg-blue-50 hover:text-blue-700 disabled:opacity-50'
const compactIconButtonClassName = 'inline-flex h-8 w-8 items-center justify-center rounded-md border border-gray-200 text-gray-600 hover:bg-blue-50 hover:text-blue-700 disabled:opacity-40'
