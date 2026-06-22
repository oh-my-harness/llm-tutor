import { useEffect, useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import {
  CheckCircle2,
  Database,
  FileText,
  Layers,
  PanelLeftClose,
  PanelLeftOpen,
  Plus,
  RefreshCw,
  Search,
  Settings,
  Star,
  Trash2,
  Upload,
} from 'lucide-react'
import { activeEmbeddingConfig, embeddingForSession } from '../settings'
import type { EmbeddingModelConfig, LlmSettings } from '../settings'

interface Props {
  settings: LlmSettings
  onChanged?: () => void
}

interface KnowledgeBaseItem {
  id: string
  name: string
  status: 'ready' | 'draft'
  embedding: {
    provider: string
    model: string
    base_url: string | null
    embeddings_path: string | null
    dimensions: number | null
    send_dimensions: boolean
    api_key_configured: boolean
  }
  documents: KnowledgeDocument[]
  created_at: string
  updated_at: string
}

interface KnowledgeDocument {
  id: string
  name: string
  source: string
  size_bytes: number
  chunks: number
  created_at: string
}

interface SearchHit {
  id: string
  source: string
  text: string
  score?: number | null
}

type TabKey = 'files' | 'upload' | 'indexes' | 'settings'

export function KnowledgePage({ settings, onChanged }: Props) {
  const [knowledgeBases, setKnowledgeBases] = useState<KnowledgeBaseItem[]>([])
  const [activeKbId, setActiveKbId] = useState<string | null>(null)
  const [selectedDocId, setSelectedDocId] = useState<string | null>(null)
  const [tab, setTab] = useState<TabKey>('files')
  const [kbSearch, setKbSearch] = useState('')
  const [source, setSource] = useState('pasted-text')
  const [text, setText] = useState('')
  const [query, setQuery] = useState('')
  const [hits, setHits] = useState<SearchHit[]>([])
  const [status, setStatus] = useState('所有更改已保存')
  const [busy, setBusy] = useState(false)
  const [creating, setCreating] = useState(false)
  const [knowledgeListCollapsed, setKnowledgeListCollapsed] = useState(false)
  const [fileListCollapsed, setFileListCollapsed] = useState(false)
  const [newKbName, setNewKbName] = useState('')
  const [newKbEmbeddingId, setNewKbEmbeddingId] = useState(settings.activeEmbeddingConfigId ?? '')
  const [previewTextByDocId, setPreviewTextByDocId] = useState<Record<string, string>>({})

  const activeKb = knowledgeBases.find((item) => item.id === activeKbId) ?? knowledgeBases[0] ?? null
  const selectedDoc = activeKb?.documents.find((doc) => doc.id === selectedDocId) ?? null
  const selectedPreviewLoaded = selectedDoc
    ? Object.prototype.hasOwnProperty.call(previewTextByDocId, selectedDoc.id)
    : false
  const selectedPreviewText = selectedDoc && selectedPreviewLoaded ? previewTextByDocId[selectedDoc.id] : ''
  const activeEmbedding = activeEmbeddingConfig(settings)
  const selectedNewEmbedding =
    settings.embeddingConfigs.find((config) => config.id === newKbEmbeddingId) ?? activeEmbedding

  const filteredKnowledgeBases = useMemo(
    () =>
      knowledgeBases.filter((item) =>
        item.name.toLowerCase().includes(kbSearch.trim().toLowerCase()),
      ),
    [knowledgeBases, kbSearch],
  )

  useEffect(() => {
    void loadKnowledgeBases()
  }, [])

  useEffect(() => {
    const first = knowledgeBases[0]
    if (!activeKbId && first) setActiveKbId(first.id)
  }, [activeKbId, knowledgeBases])

  useEffect(() => {
    if (!activeKb || !selectedDoc || selectedPreviewLoaded) return
    void loadDocumentPreview(activeKb.id, selectedDoc.id)
  }, [activeKb, selectedDoc, selectedPreviewLoaded])

  const loadKnowledgeBases = async () => {
    try {
      const res = await fetch('/api/knowledge-bases')
      const data = await res.json()
      if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`)
      setKnowledgeBases((data.knowledge_bases ?? []) as KnowledgeBaseItem[])
      setStatus('所有更改已保存')
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    }
  }

  const loadDocumentPreview = async (kbId: string, documentId: string) => {
    try {
      const res = await fetch(
        `/api/knowledge-bases/${encodeURIComponent(kbId)}/documents/${encodeURIComponent(documentId)}/content`,
      )
      const data = await res.json()
      if (!res.ok) return
      setPreviewTextByDocId((prev) => ({ ...prev, [documentId]: data.text ?? '' }))
    } catch {
      // Preview is best-effort; indexing/search should not fail because of it.
    }
  }

  const createKnowledgeBase = async () => {
    if (!selectedNewEmbedding || busy) return
    setBusy(true)
    setStatus('正在创建知识库...')
    try {
      const name = newKbName.trim() || `knowledge-${knowledgeBases.length + 1}`
      const res = await fetch('/api/knowledge-bases', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name, embedding: embeddingForSession(selectedNewEmbedding) }),
      })
      const data = await res.json()
      if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`)
      const created = data.knowledge_base as KnowledgeBaseItem
      setKnowledgeBases((prev) => [...prev, created])
      setActiveKbId(created.id)
      setSelectedDocId(null)
      setNewKbName('')
      setCreating(false)
      setTab('upload')
      setStatus('知识库已创建')
      onChanged?.()
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const deleteKnowledgeBase = async (id: string) => {
    if (busy) return
    setBusy(true)
    try {
      const res = await fetch(`/api/knowledge-bases/${encodeURIComponent(id)}`, { method: 'DELETE' })
      if (!res.ok && res.status !== 404) {
        const data = await safeJson(res)
        throw new Error(data.error || `HTTP ${res.status}`)
      }
      setKnowledgeBases((prev) => prev.filter((item) => item.id !== id))
      if (activeKbId === id) {
        setActiveKbId(null)
        setSelectedDocId(null)
      }
      setStatus('知识库已删除')
      onChanged?.()
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const ingest = async () => {
    if (!activeKb || !text.trim() || busy) return
    setBusy(true)
    setStatus('正在入库...')
    try {
      const res = await fetch(`/api/knowledge-bases/${encodeURIComponent(activeKb.id)}/documents`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ source: source.trim() || 'pasted-text', text }),
      })
      const data = await res.json()
      if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`)
      const updated = data.knowledge_base as KnowledgeBaseItem
      const doc = updated.documents[0]
      setKnowledgeBases((prev) => prev.map((item) => (item.id === updated.id ? updated : item)))
      if (doc) {
        setSelectedDocId(doc.id)
        setPreviewTextByDocId((prev) => ({ ...prev, [doc.id]: text }))
      }
      setTab('files')
      setText('')
      setStatus(`已入库 ${data.chunks ?? 0} 个片段`)
      onChanged?.()
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  const search = async () => {
    if (!activeKb || !query.trim() || busy) return
    setBusy(true)
    setStatus('正在检索...')
    try {
      const res = await fetch(`/api/knowledge-bases/${encodeURIComponent(activeKb.id)}/search`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ query, top_k: 5 }),
      })
      const data = await res.json()
      if (!res.ok) throw new Error(data.error || `HTTP ${res.status}`)
      setHits(data.hits ?? [])
      setStatus(`找到 ${(data.hits ?? []).length} 条结果`)
    } catch (err) {
      setStatus(err instanceof Error ? err.message : String(err))
    } finally {
      setBusy(false)
    }
  }

  return (
    <main className="flex min-h-0 flex-1 bg-white">
      <KnowledgeListPanel
        collapsed={knowledgeListCollapsed}
        items={filteredKnowledgeBases}
        totalCount={knowledgeBases.length}
        activeId={activeKb?.id ?? null}
        search={kbSearch}
        canCreate={settings.embeddingConfigs.length > 0}
        busy={busy}
        onSearch={setKbSearch}
        onCollapse={() => setKnowledgeListCollapsed(true)}
        onExpand={() => setKnowledgeListCollapsed(false)}
        onCreate={() => {
          setCreating(true)
          setTab('settings')
        }}
        onSelect={(item) => {
          setActiveKbId(item.id)
          setSelectedDocId(item.documents[0]?.id ?? null)
          setCreating(false)
        }}
        onDelete={deleteKnowledgeBase}
      />

      <section className="flex min-w-0 flex-1 flex-col">
        <header className="border-b border-stone-200 px-5 pt-4">
          <div className="flex flex-wrap items-start gap-3">
            <div className="min-w-0 flex-1">
              <div className="flex flex-wrap items-center gap-2">
                <h1 className="truncate text-xl font-semibold text-gray-950">
                  {creating ? '新建知识库' : activeKb?.name ?? '知识库'}
                </h1>
                {activeKb && !creating && (
                  <>
                    <Badge tone="amber" icon={<Star size={14} className="fill-amber-500" />}>
                      默认
                    </Badge>
                    <Badge tone="green" icon={<CheckCircle2 size={14} />}>
                      {statusLabel(activeKb.status)}
                    </Badge>
                  </>
                )}
              </div>
              <p className="mt-1.5 text-xs text-stone-600">
                {creating
                  ? '创建时绑定嵌入模型，后续入库和检索都固定使用该配置。'
                  : activeKb
                    ? `${activeKb.embedding.model} · ${activeKb.embedding.dimensions ?? '未知'}维 · 最近更新 ${formatTime(activeKb.updated_at)}`
                    : '请先创建一个知识库。'}
              </p>
            </div>
            <div className="text-xs text-stone-600">{status}</div>
          </div>

          <nav className="mt-4 flex gap-5">
            <TabButton active={tab === 'files'} icon={<FileText size={17} />} onClick={() => setTab('files')}>
              文件
            </TabButton>
            <TabButton active={tab === 'upload'} icon={<Upload size={17} />} onClick={() => setTab('upload')}>
              添加文档
            </TabButton>
            <TabButton active={tab === 'indexes'} icon={<Layers size={17} />} onClick={() => setTab('indexes')}>
              索引版本
            </TabButton>
            <TabButton active={tab === 'settings'} icon={<Settings size={17} />} onClick={() => setTab('settings')}>
              设置
            </TabButton>
          </nav>
        </header>

        {creating ? (
          <CreatePanel
            settings={settings}
            name={newKbName}
            embeddingId={newKbEmbeddingId}
            busy={busy}
            onNameChange={setNewKbName}
            onEmbeddingChange={setNewKbEmbeddingId}
            onCreate={createKnowledgeBase}
            selectedEmbedding={selectedNewEmbedding}
          />
        ) : activeKb ? (
          <div className="flex min-h-0 flex-1">
            {tab === 'files' && (
              <FileListPanel
                activeKb={activeKb}
                selectedDocId={selectedDocId}
                collapsed={fileListCollapsed}
                onSelectDoc={setSelectedDocId}
                onUpload={() => setTab('upload')}
                onReload={loadKnowledgeBases}
                onToggleCollapsed={() => setFileListCollapsed((value) => !value)}
              />
            )}
            <section className="min-w-0 flex-1 overflow-y-auto">
              {tab === 'upload' && (
                <Panel>
                  <div className="max-w-3xl">
                    <h3 className="text-lg font-semibold text-gray-950">添加文档</h3>
                    <p className="mt-1 text-sm text-stone-600">
                      将使用 {activeKb.embedding.model} 为文档生成向量。
                    </p>
                    <div className="mt-5 grid gap-4">
                      <Field label="文件名">
                        <input
                          className={inputClassName}
                          value={source}
                          onChange={(event) => setSource(event.target.value)}
                        />
                      </Field>
                      <Field label="内容">
                        <textarea
                          className={`${inputClassName} min-h-72 resize-none`}
                          value={text}
                          onChange={(event) => setText(event.target.value)}
                          placeholder="粘贴课程资料、讲义片段或笔记..."
                        />
                      </Field>
                      <button className={primaryButtonClassName} type="button" onClick={ingest} disabled={!text.trim() || busy}>
                        <Database size={17} />
                        写入索引
                      </button>
                    </div>
                  </div>
                </Panel>
              )}

              {tab === 'indexes' && (
                <Panel>
                  <div className="max-w-3xl">
                    <h3 className="text-lg font-semibold text-gray-950">索引版本</h3>
                    <p className="mt-1 text-sm text-stone-600">
                      当前版本绑定 {activeKb.embedding.model}，切换模型需要新建索引版本。
                    </p>
                    <div className="mt-5 flex gap-3">
                      <input
                        className={inputClassName}
                        value={query}
                        onChange={(event) => setQuery(event.target.value)}
                        placeholder="输入检索问题..."
                      />
                      <button className={primaryButtonClassName} type="button" onClick={search} disabled={!query.trim() || busy}>
                        <Search size={17} />
                        检索
                      </button>
                    </div>
                    <div className="mt-5 space-y-3">
                      {hits.map((hit) => (
                        <article key={hit.id} className="rounded-lg border border-stone-200 bg-stone-50 p-4">
                          <div className="mb-2 flex items-center justify-between text-xs text-stone-500">
                            <span className="truncate">{hit.source}</span>
                            <span>{typeof hit.score === 'number' ? hit.score.toFixed(4) : 'n/a'}</span>
                          </div>
                          <p className="text-sm leading-6 text-gray-800">{hit.text}</p>
                        </article>
                      ))}
                    </div>
                  </div>
                </Panel>
              )}

              {tab === 'settings' && (
                <Panel>
                  <div className="max-w-2xl">
                    <h3 className="text-lg font-semibold text-gray-950">设置</h3>
                    <dl className="mt-5 divide-y divide-stone-200 rounded-lg border border-stone-200">
                      <InfoRow label="嵌入模型" value={activeKb.embedding.model} />
                      <InfoRow label="Base URL" value={activeKb.embedding.base_url ?? '-'} />
                      <InfoRow label="端点" value={activeKb.embedding.embeddings_path ?? '-'} />
                      <InfoRow label="维度" value={String(activeKb.embedding.dimensions ?? '-')} />
                      <InfoRow label="文档数" value={String(activeKb.documents.length)} />
                    </dl>
                  </div>
                </Panel>
              )}

              {tab === 'files' && <FilePreview selectedDoc={selectedDoc} previewText={selectedPreviewText} />}
            </section>
          </div>
        ) : (
          <CreatePanel
            settings={settings}
            name={newKbName}
            embeddingId={newKbEmbeddingId}
            busy={busy}
            onNameChange={setNewKbName}
            onEmbeddingChange={setNewKbEmbeddingId}
            onCreate={createKnowledgeBase}
            selectedEmbedding={selectedNewEmbedding}
          />
        )}
      </section>
    </main>
  )
}

function KnowledgeListPanel({
  collapsed,
  items,
  totalCount,
  activeId,
  search,
  canCreate,
  busy,
  onSearch,
  onCollapse,
  onExpand,
  onCreate,
  onSelect,
  onDelete,
}: {
  collapsed: boolean
  items: KnowledgeBaseItem[]
  totalCount: number
  activeId: string | null
  search: string
  canCreate: boolean
  busy: boolean
  onSearch: (value: string) => void
  onCollapse: () => void
  onExpand: () => void
  onCreate: () => void
  onSelect: (item: KnowledgeBaseItem) => void
  onDelete: (id: string) => void
}) {
  if (collapsed) {
    return (
      <section className="flex w-12 shrink-0 flex-col items-center border-r border-stone-200 bg-stone-50/70 py-3">
        <button className={iconButtonClassName} type="button" title="展开知识库列表" onClick={onExpand}>
          <PanelLeftOpen size={17} />
        </button>
        <span className="mt-3 [writing-mode:vertical-rl] text-xs font-medium tracking-wide text-stone-500">
          知识库
        </span>
      </section>
    )
  }

  return (
    <section className="flex w-72 shrink-0 flex-col border-r border-stone-200 bg-stone-50/70">
      <div className="flex items-center justify-between px-4 py-3">
        <div className="flex items-center gap-2">
          <h2 className="text-base font-semibold text-gray-950">知识库</h2>
          <span className="rounded-full bg-stone-200 px-2 py-0.5 text-xs text-stone-700">{totalCount}</span>
        </div>
        <button className={iconButtonClassName} type="button" title="收起" onClick={onCollapse}>
          <PanelLeftClose size={17} />
        </button>
      </div>

      <div className="px-4">
        <button
          className="flex h-10 w-full items-center justify-center gap-2 rounded-lg bg-amber-700 text-sm font-medium text-white hover:bg-amber-800 disabled:bg-stone-300"
          type="button"
          onClick={onCreate}
          disabled={!canCreate || busy}
        >
          <Plus size={17} />
          新建知识库
        </button>
        <div className="relative mt-3">
          <Search size={18} className="absolute left-3 top-1/2 -translate-y-1/2 text-stone-500" />
          <input
            className="h-10 w-full rounded-lg border border-stone-200 bg-white pl-9 pr-3 text-sm outline-none placeholder:text-stone-500 focus:border-stone-400"
            value={search}
            onChange={(event) => onSearch(event.target.value)}
            placeholder="搜索知识库..."
          />
        </div>
      </div>

      <div className="mt-2 flex-1 space-y-1.5 overflow-y-auto px-3 pb-4">
        {items.map((item) => (
          <button
            key={item.id}
            className={`group flex w-full items-start gap-2.5 rounded-lg border p-2.5 text-left ${
              item.id === activeId ? 'border-stone-200 bg-white shadow-sm' : 'border-transparent hover:bg-white'
            }`}
            type="button"
            onClick={() => onSelect(item)}
          >
            <span className="mt-1 h-2.5 w-2.5 rounded-full bg-emerald-500" />
            <span className="min-w-0 flex-1">
              <span className="flex items-center gap-2 font-medium text-gray-900">
                <Star size={16} className="fill-amber-400 text-amber-500" />
                <span className="truncate">{item.name}</span>
              </span>
              <span className="mt-1 block text-xs text-stone-500">
                {statusLabel(item.status)} · {item.documents.length} 个文档
              </span>
            </span>
            <span
              className="rounded p-1 text-stone-400 opacity-0 hover:bg-stone-100 hover:text-stone-700 group-hover:opacity-100"
              onClick={(event) => {
                event.stopPropagation()
                onDelete(item.id)
              }}
            >
              <Trash2 size={16} />
            </span>
          </button>
        ))}
      </div>
    </section>
  )
}

function CreatePanel({
  settings,
  name,
  embeddingId,
  busy,
  selectedEmbedding,
  onNameChange,
  onEmbeddingChange,
  onCreate,
}: {
  settings: LlmSettings
  name: string
  embeddingId: string
  busy: boolean
  selectedEmbedding: EmbeddingModelConfig | null
  onNameChange: (value: string) => void
  onEmbeddingChange: (value: string) => void
  onCreate: () => void
}) {
  return (
    <Panel>
      <div className="max-w-2xl">
        <h3 className="text-lg font-semibold text-gray-950">新建知识库</h3>
        <div className="mt-5 grid gap-4">
          <Field label="知识库名称">
            <input className={inputClassName} value={name} onChange={(event) => onNameChange(event.target.value)} placeholder="例如：高等数学教材" />
          </Field>
          <Field label="嵌入模型">
            <select className={inputClassName} value={embeddingId} onChange={(event) => onEmbeddingChange(event.target.value)}>
              <option value="">选择嵌入模型</option>
              {settings.embeddingConfigs.map((config) => (
                <option key={config.id} value={config.id}>
                  {config.name} · {config.model} · {config.dimensions}维
                </option>
              ))}
            </select>
          </Field>
          {selectedEmbedding && (
            <div className="rounded-lg border border-stone-200 bg-stone-50 p-3 text-sm text-stone-700">
              创建后将固定使用 {selectedEmbedding.model}，维度 {selectedEmbedding.dimensions}。
            </div>
          )}
          <button className={primaryButtonClassName} type="button" onClick={onCreate} disabled={!selectedEmbedding || busy}>
            <Plus size={17} />
            创建
          </button>
        </div>
      </div>
    </Panel>
  )
}

function FileListPanel({
  activeKb,
  selectedDocId,
  collapsed,
  onSelectDoc,
  onUpload,
  onReload,
  onToggleCollapsed,
}: {
  activeKb: KnowledgeBaseItem
  selectedDocId: string | null
  collapsed: boolean
  onSelectDoc: (id: string | null) => void
  onUpload: () => void
  onReload: () => void
  onToggleCollapsed: () => void
}) {
  if (collapsed) {
    return (
      <aside className="flex w-12 shrink-0 flex-col items-center border-r border-stone-200 py-3">
        <button className={iconButtonClassName} type="button" title="展开文件列表" onClick={onToggleCollapsed}>
          <PanelLeftOpen size={16} />
        </button>
        <span className="mt-3 [writing-mode:vertical-rl] text-xs font-medium tracking-wide text-stone-500">文件</span>
      </aside>
    )
  }

  return (
    <aside className="flex w-72 shrink-0 flex-col border-r border-stone-200">
      <div className="flex h-12 items-center justify-between border-b border-stone-200 px-4">
        <div className="flex items-center gap-2">
          <span className="font-medium text-gray-900">文件</span>
          <span className="rounded-full bg-stone-100 px-2 py-0.5 text-xs text-stone-600">
            {activeKb.documents.length}
          </span>
        </div>
        <div className="flex items-center gap-1">
          <button className={iconButtonClassName} type="button" title="刷新" onClick={onReload}>
            <RefreshCw size={16} />
          </button>
          <button className={iconButtonClassName} type="button" title="收起" onClick={onToggleCollapsed}>
            <PanelLeftClose size={16} />
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto p-3">
        {activeKb.documents.length === 0 ? (
          <button
            className="flex w-full items-center gap-2.5 rounded-lg border border-dashed border-stone-300 p-3 text-left text-sm text-stone-600 hover:bg-stone-50"
            type="button"
            onClick={onUpload}
          >
            <Upload size={18} />
            添加第一个文档
          </button>
        ) : (
          activeKb.documents.map((doc) => (
            <button
              key={doc.id}
              className={`flex w-full items-start gap-2.5 rounded-lg p-2.5 text-left hover:bg-stone-50 ${
                doc.id === selectedDocId ? 'bg-stone-100' : ''
              }`}
              type="button"
              onClick={() => onSelectDoc(doc.id)}
            >
              <FileText size={18} className="mt-0.5 shrink-0 text-stone-600" />
              <span className="min-w-0">
                <span className="block truncate text-sm font-medium text-gray-900">{doc.name}</span>
                <span className="mt-1 block text-xs text-stone-500">
                  {formatSize(doc.size_bytes)} · {formatTime(doc.created_at)}
                </span>
              </span>
            </button>
          ))
        )}
      </div>
    </aside>
  )
}

function FilePreview({ selectedDoc, previewText }: { selectedDoc: KnowledgeDocument | null; previewText?: string }) {
  if (!selectedDoc) {
    return (
      <div className="flex h-full min-h-[460px] items-center justify-center px-8 text-center">
        <div>
          <div className="mx-auto flex h-14 w-14 items-center justify-center rounded-2xl bg-stone-100 text-stone-700">
            <FileText size={24} />
          </div>
          <h3 className="mt-5 text-base font-semibold text-gray-950">请选择一个文件以预览</h3>
          <p className="mt-2 max-w-sm text-sm leading-6 text-stone-600">从左侧列表选择任意文档，可在此处直接预览。</p>
        </div>
      </div>
    )
  }

  return (
    <Panel>
      <div className="mx-auto max-w-3xl">
        <div className="mb-4 flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-stone-100 text-stone-700">
            <FileText size={20} />
          </div>
          <div className="min-w-0">
            <h3 className="truncate text-lg font-semibold text-gray-950">{selectedDoc.name}</h3>
            <p className="mt-0.5 text-xs text-stone-500">
              {formatSize(selectedDoc.size_bytes)} · {formatTime(selectedDoc.created_at)}
            </p>
          </div>
        </div>
        {previewText ? (
          <pre className="whitespace-pre-wrap rounded-lg border border-stone-200 bg-stone-50 p-4 font-sans text-sm leading-6 text-gray-800">
            {previewText}
          </pre>
        ) : (
          <div className="rounded-lg border border-stone-200 bg-stone-50 p-4 text-sm text-stone-600">
            该文档已入库。当前版本只保存文档元数据和向量索引，刷新后不保留原文预览。
          </div>
        )}
      </div>
    </Panel>
  )
}

function Badge({ tone, icon, children }: { tone: 'amber' | 'green'; icon: ReactNode; children: ReactNode }) {
  const toneClass = tone === 'amber' ? 'bg-amber-100 text-amber-800' : 'bg-emerald-100 text-emerald-800'
  return <span className={`inline-flex h-6 items-center gap-1 rounded-full px-2 text-xs font-medium ${toneClass}`}>{icon}{children}</span>
}

function TabButton({ active, icon, children, onClick }: { active: boolean; icon: ReactNode; children: ReactNode; onClick: () => void }) {
  return (
    <button
      className={`flex h-10 items-center gap-1.5 border-b-2 text-sm font-medium ${
        active ? 'border-amber-700 text-gray-950' : 'border-transparent text-stone-600 hover:text-gray-950'
      }`}
      type="button"
      onClick={onClick}
    >
      {icon}
      {children}
    </button>
  )
}

function Panel({ children }: { children: ReactNode }) {
  return <div className="p-5">{children}</div>
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-sm font-medium text-stone-700">{label}</span>
      {children}
    </label>
  )
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-center justify-between gap-4 px-4 py-3 text-sm">
      <dt className="text-stone-500">{label}</dt>
      <dd className="truncate font-medium text-gray-900">{value}</dd>
    </div>
  )
}

async function safeJson(res: Response): Promise<Record<string, string>> {
  try {
    return await res.json()
  } catch {
    return {}
  }
}

function statusLabel(status: KnowledgeBaseItem['status']) {
  return status === 'ready' ? '就绪' : '草稿'
}

function formatSize(value: number | string) {
  const bytes = typeof value === 'number' ? value : new Blob([value]).size
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`
}

function formatTime(value: string) {
  if (!value) return '暂无'
  const date = new Date(value)
  if (Number.isNaN(date.getTime())) return value
  return date.toLocaleString()
}

const iconButtonClassName =
  'inline-flex h-7 w-7 items-center justify-center rounded text-stone-500 hover:bg-stone-100 hover:text-stone-900'

const inputClassName =
  'w-full rounded-lg border border-stone-200 bg-white px-3 py-1.5 text-sm text-gray-900 outline-none placeholder:text-stone-400 focus:border-stone-400 disabled:bg-stone-50'

const primaryButtonClassName =
  'inline-flex h-9 items-center justify-center gap-2 rounded-lg bg-gray-900 px-3.5 text-sm font-medium text-white disabled:bg-stone-200 disabled:text-stone-400'
