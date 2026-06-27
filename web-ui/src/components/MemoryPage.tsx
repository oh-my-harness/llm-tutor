import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  ArrowLeft,
  Bot,
  Brain,
  CheckCircle2,
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
  edits?: MemoryEdit[]
  trace?: {
    input_json: string
    output_json: string
  } | null
  changed: boolean
}

const l2Paths = ['L2/chat.md', 'L2/quiz.md', 'L2/notebook.md', 'L2/knowledge.md', 'L2/research.md']
const l3Paths = ['L3/recent.md', 'L3/profile.md', 'L3/scope.md', 'L3/preferences.md', 'L3/teaching_strategy.md']

export function MemoryPage({ settings }: { settings: LlmSettings }) {
  const [files, setFiles] = useState<MemoryFile[]>([])
  const [layer, setLayer] = useState<Layer>('overview')
  const [activePath, setActivePath] = useState<string | null>(null)
  const [draft, setDraft] = useState('')
  const [viewMode, setViewMode] = useState<ViewMode>('rendered')
  const [assistResult, setAssistResult] = useState<AssistResult | null>(null)
  const [status, setStatus] = useState('Loading memory...')
  const [loading, setLoading] = useState(false)

  const activeFile = useMemo(
    () => files.find((file) => file.path === activePath) ?? null,
    [activePath, files],
  )
  const l2Files = useMemo(() => l2Paths.map((path) => files.find((file) => file.path === path)).filter(Boolean) as MemoryFile[], [files])
  const l3Files = useMemo(() => l3Paths.map((path) => files.find((file) => file.path === path)).filter(Boolean) as MemoryFile[], [files])

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

  const runAssist = async (action: AssistAction) => {
    if (!activeFile) return
    const llm = settingsForSession(settings)
    if (!llm.model || !llm.api_key) {
      setStatus('请先在设置中配置可用的 LLM，记忆维护需要模型参与。')
      return
    }
    setLoading(true)
    try {
      const res = await fetch('/api/memory/assist', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          target_path: activeFile.path,
          action,
          markdown: draft,
          llm,
        }),
      })
      const data = await safeJson(res)
      if (!res.ok) throw new Error(errorMessage(data, res.status))
      const result = data.result as AssistResult
      setAssistResult(result)
      setStatus(`${assistLabel(action)} complete`)
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
            onBack={() => setLayer('overview')}
            onSelectFile={setActivePath}
            onDraftChange={setDraft}
            onViewModeChange={setViewMode}
            onSave={() => void saveActiveFile()}
            onUndo={() => void undoActiveFile()}
            onRunAssist={(action) => void runAssist(action)}
            onApplyProposal={(markdown) => {
              setDraft(markdown)
              setStatus('Memory draft updated from agent proposal')
            }}
            onReset={() => {
              setDraft(activeFile?.markdown ?? '')
              setAssistResult(null)
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
  onBack,
  onSelectFile,
  onDraftChange,
  onViewModeChange,
  onSave,
  onUndo,
  onRunAssist,
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
  onBack: () => void
  onSelectFile: (path: string) => void
  onDraftChange: (value: string) => void
  onViewModeChange: (mode: ViewMode) => void
  onSave: () => void
  onUndo: () => void
  onRunAssist: (action: AssistAction) => void
  onApplyProposal: (markdown: string) => void
  onReset: () => void
}) {
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <header className="flex items-center gap-3 px-8 py-5">
        <button className="rounded p-2 text-gray-500 hover:bg-gray-100" type="button" onClick={onBack}>
          <ArrowLeft size={18} />
        </button>
        <div className="text-sm text-gray-500">记忆</div>
        <span className="text-gray-300">/</span>
        <div className="text-sm font-medium text-gray-900">{layer} · {layer === 'L2' ? '各模块摘要' : '跨模块知识'}</div>
        <div className="ml-auto inline-flex rounded-full border border-gray-200 bg-white p-1">
          {(['L2', 'L3'] as const).map((item) => (
            <button
              key={item}
              className={`rounded-full px-3 py-1.5 text-sm ${item === layer ? 'bg-blue-50 text-blue-700' : 'text-gray-500'}`}
              type="button"
              disabled
            >
              {item}
            </button>
          ))}
        </div>
      </header>

      <div className="grid min-h-0 flex-1 grid-cols-[260px_minmax(0,1fr)_360px] gap-6 px-8 pb-8">
        <aside className="min-h-0 rounded-lg border border-gray-200 bg-white p-3">
          <div className="space-y-1">
            {files.map((file) => (
              <button
                key={file.path}
                className={`flex w-full items-center gap-3 rounded-lg px-3 py-3 text-left text-sm ${
                  activeFile?.path === file.path ? 'bg-blue-50 text-blue-700' : 'text-gray-700 hover:bg-gray-50'
                }`}
                type="button"
                onClick={() => onSelectFile(file.path)}
              >
                <FileText size={17} />
                <span className="min-w-0 flex-1">
                  <span className="block truncate font-medium">{memoryFileLabel(file.path)}</span>
                  <span className="block truncate text-xs text-gray-400">{lineCount(file.markdown)} lines</span>
                </span>
                {hasRealMemory(file.markdown) && <span className="rounded-full bg-gray-100 px-2 py-0.5 text-xs text-gray-500">on</span>}
              </button>
            ))}
          </div>
        </aside>

        <section className="flex min-h-0 flex-col rounded-lg border border-gray-200 bg-white">
          <div className="flex items-center gap-3 border-b border-gray-100 px-5 py-3">
            <div className="min-w-0">
              <h2 className="truncate text-lg font-semibold text-gray-950">{activeFile ? memoryFileLabel(activeFile.path) : 'Memory file'}</h2>
              <p className="text-xs text-gray-500">{activeFile?.path ?? status}</p>
            </div>
            <div className="ml-auto inline-flex rounded-lg border border-gray-200 bg-gray-50 p-1">
              <button className={modeButtonClassName(viewMode === 'rendered')} type="button" onClick={() => onViewModeChange('rendered')}>渲染视图</button>
              <button className={modeButtonClassName(viewMode === 'source')} type="button" onClick={() => onViewModeChange('source')}>带行号</button>
            </div>
            <button className={secondaryButtonClassName} type="button" disabled={loading || !activeFile || draft === activeFile.markdown} onClick={onSave}>
              <Save size={16} />
              保存
            </button>
            <button className={secondaryButtonClassName} type="button" disabled={loading || !activeFile} onClick={onUndo}>
              <RotateCcw size={16} />
              撤销
            </button>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto p-5">
            {viewMode === 'source' ? (
              <textarea
                className="min-h-full w-full resize-none rounded-lg border border-gray-100 bg-gray-50 p-4 font-mono text-sm leading-7 text-gray-800 outline-none focus:border-blue-300 focus:ring-2 focus:ring-blue-50"
                value={draft}
                onChange={(event) => onDraftChange(event.target.value)}
              />
            ) : (
              <article className="min-h-full rounded-lg border border-gray-100 bg-gray-50 p-5">
                <MarkdownMessage text={draft || ' '} />
              </article>
            )}
          </div>
        </section>

        <AgentWorkspace
          loading={loading}
          assistResult={assistResult}
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
  onRunAssist,
  onApplyProposal,
  onReset,
}: {
  loading: boolean
  assistResult: AssistResult | null
  onRunAssist: (action: AssistAction) => void
  onApplyProposal: (markdown: string) => void
  onReset: () => void
}) {
  return (
    <aside className="flex min-h-0 flex-col rounded-lg border border-gray-200 bg-white">
      <div className="flex items-center gap-3 border-b border-gray-100 px-4 py-3">
        <Bot size={18} className="text-blue-600" />
        <div className="font-semibold text-gray-950">Agent 工作区</div>
        <button className="ml-auto rounded-lg border border-gray-200 px-3 py-1.5 text-sm text-gray-600 hover:bg-gray-50" type="button" onClick={onReset}>
          Reset
        </button>
      </div>
      <div className="border-b border-gray-100 p-4">
        <div className="grid grid-cols-3 gap-2">
          <AssistButton icon={Wand2} label="更新记忆" disabled={loading} onClick={() => onRunAssist('update')} />
          <AssistButton icon={CheckCircle2} label="检查记忆" disabled={loading} onClick={() => onRunAssist('check')} />
          <AssistButton icon={GitBranch} label="去重" disabled={loading} onClick={() => onRunAssist('dedupe')} />
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto p-5">
        {assistResult ? (
          <article className="rounded-lg border border-blue-100 bg-blue-50/40 p-4">
            <MarkdownMessage text={assistResult.report_markdown} />
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
            <Sparkles size={30} className="mb-4 text-gray-300" />
            选择一个模式并运行。报告会显示在这里；更新和去重会把草稿写入中间编辑区，保存前仍可手动检查。
          </div>
        )}
      </div>
      <div className="border-t border-gray-100 p-4">
        <button className="inline-flex h-10 w-full items-center justify-center gap-2 rounded-lg bg-blue-600 text-sm font-medium text-white hover:bg-blue-700 disabled:bg-gray-200" type="button" disabled={loading}>
          <Send size={16} />
          {loading ? '运行中' : '等待操作'}
        </button>
      </div>
    </aside>
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

function AssistButton({ icon: Icon, label, disabled, onClick }: { icon: typeof Brain; label: string; disabled: boolean; onClick: () => void }) {
  return (
    <button className="inline-flex h-10 items-center justify-center gap-1.5 rounded-lg border border-gray-200 bg-gray-50 px-2 text-sm font-medium text-gray-700 hover:bg-blue-50 hover:text-blue-700 disabled:opacity-50" disabled={disabled} type="button" onClick={onClick}>
      <Icon size={15} />
      {label}
    </button>
  )
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
  return `rounded-md px-3 py-1.5 text-sm ${active ? 'bg-white text-gray-950 shadow-sm' : 'text-gray-500 hover:text-gray-900'}`
}

function assistLabel(action: AssistAction) {
  if (action === 'update') return 'Update memory'
  if (action === 'check') return 'Check memory'
  return 'Dedupe memory'
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
