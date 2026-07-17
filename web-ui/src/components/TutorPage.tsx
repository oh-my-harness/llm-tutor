import { useEffect, useState } from 'react'
import { Bot, Check, Eye, History, ListTodo, MessageSquare, Pencil, Plus, RotateCcw, Save, Trash2, X } from 'lucide-react'
import {
  archiveTutor,
  createTutorMemory,
  createTutor,
  deleteTutorMemory,
  fetchTutorMemory,
  resetTutorMemory,
  resolveTutorMemory,
  tutorCapabilities,
  tutorSoulSummary,
  updateTutorMemory,
  updateTutor,
  type TutorDraft,
  type TutorMemoryDraft,
  type TutorMemoryEntry,
  type TutorMemoryKind,
  type TutorProfile,
} from '../tutorTypes'
import { MarkdownMessage } from './MarkdownMessage'

interface Props {
  tutors: TutorProfile[]
  modelConfigs: Array<{ id: string; name: string; model: string }>
  knowledgeBases: Array<{ id: string; name: string }>
  onChanged: () => Promise<void>
  onStartConversation: (tutorId: string) => void
  onReturnToOnboarding?: () => void
}

const emptyDraft: TutorDraft = {
  name: '',
  soul_markdown: `# 核心身份

你是一位帮助学习者持续进步的导师。

# 教学风格

- 先建立直觉，再展开细节。
- 根据学习者反馈调整讲解方式。

# 教学原则

- 区分事实、推测和建议。
- 不假装学习者已经理解。

# 边界

- 不记录敏感个人信息。
- 不在证据不足时评价学习者的能力。`,
  default_model_config_id: null,
  default_capability: 'chat',
  allowed_capabilities: ['chat', 'quiz', 'research', 'organize'],
  learner_memory_access: true,
  autonomous_memory: true,
  resource_permissions: { knowledge_base_ids: [], notebook: true, space: true },
}

const emptyMemoryDraft: TutorMemoryDraft = {
  kind: 'open_loop',
  text: '',
  next_action: '',
}

export function TutorPage({ tutors, modelConfigs, knowledgeBases, onChanged, onStartConversation, onReturnToOnboarding }: Props) {
  const [selectedId, setSelectedId] = useState<string | null>(tutors[0]?.id ?? null)
  const [creating, setCreating] = useState(false)
  const [draft, setDraft] = useState<TutorDraft>(emptyDraft)
  const [busy, setBusy] = useState(false)
  const [status, setStatus] = useState('')
  const [soulView, setSoulView] = useState<'edit' | 'preview'>('edit')
  const [workspaceView, setWorkspaceView] = useState<'profile' | 'memory'>('profile')
  const [memoryEntries, setMemoryEntries] = useState<TutorMemoryEntry[]>([])
  const [memoryDraft, setMemoryDraft] = useState<TutorMemoryDraft>(emptyMemoryDraft)
  const [editingMemoryId, setEditingMemoryId] = useState<string | null>(null)
  const [includeResolved, setIncludeResolved] = useState(true)
  const [memoryBusy, setMemoryBusy] = useState(false)
  const [memoryStatus, setMemoryStatus] = useState('')
  const selected = tutors.find((item) => item.id === selectedId) ?? null

  useEffect(() => {
    if (creating) return
    if (selected) {
      setDraft(profileToDraft(selected))
    } else if (tutors[0]) {
      setSelectedId(tutors[0].id)
      setDraft(profileToDraft(tutors[0]))
    }
  }, [creating, selected, tutors])

  useEffect(() => {
    if (!selectedId || creating) {
      setMemoryEntries([])
      return
    }
    let active = true
    setMemoryBusy(true)
    void fetchTutorMemory(selectedId, includeResolved)
      .then((entries) => {
        if (active) setMemoryEntries(entries)
      })
      .catch((error) => {
        if (active) setMemoryStatus(error instanceof Error ? error.message : String(error))
      })
      .finally(() => {
        if (active) setMemoryBusy(false)
      })
    return () => { active = false }
  }, [creating, includeResolved, selectedId])

  const choose = (tutor: TutorProfile) => {
    setCreating(false)
    setSelectedId(tutor.id)
    setDraft(profileToDraft(tutor))
    setStatus('')
    setSoulView('edit')
    setWorkspaceView('profile')
    setEditingMemoryId(null)
    setMemoryStatus('')
  }

  const startCreate = () => {
    setCreating(true)
    setSelectedId(null)
    setDraft({ ...emptyDraft, resource_permissions: { ...emptyDraft.resource_permissions } })
    setStatus('')
    setSoulView('edit')
    setWorkspaceView('profile')
  }

  const save = async () => {
    if (!draft.name.trim() || !draft.soul_markdown.trim()) {
      setStatus('请填写导师名称和 Soul。')
      return
    }
    setBusy(true)
    setStatus('')
    try {
      const saved = creating || !selected
        ? await createTutor(draft)
        : await updateTutor(selected.id, draft)
      await onChanged()
      setCreating(false)
      setSelectedId(saved.id)
      setStatus('导师配置已保存。')
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error))
    } finally {
      setBusy(false)
    }
  }

  const remove = async (target: TutorProfile | null = selected) => {
    if (!target || target.built_in || !window.confirm(`删除“${target.name}”？该导师将从列表中移除，已有会话和历史记录仍会保留。`)) return
    setBusy(true)
    try {
      await archiveTutor(target.id)
      await onChanged()
      if (selectedId === target.id) setSelectedId(null)
      setStatus('导师已删除。')
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error))
    } finally {
      setBusy(false)
    }
  }

  const reloadMemory = async () => {
    if (!selected) return
    setMemoryEntries(await fetchTutorMemory(selected.id, includeResolved))
  }

  const addMemory = async () => {
    if (!selected || !memoryDraft.text.trim()) {
      setMemoryStatus('请填写记忆内容。')
      return
    }
    setMemoryBusy(true)
    setMemoryStatus('')
    try {
      await createTutorMemory(selected.id, memoryDraft)
      setMemoryDraft(emptyMemoryDraft)
      await reloadMemory()
      setMemoryStatus('导师记忆已添加。')
    } catch (error) {
      setMemoryStatus(error instanceof Error ? error.message : String(error))
    } finally {
      setMemoryBusy(false)
    }
  }

  const beginMemoryEdit = (entry: TutorMemoryEntry) => {
    setEditingMemoryId(entry.id)
    setMemoryDraft({ kind: entry.kind, text: entry.text, next_action: entry.next_action ?? '' })
    setMemoryStatus('')
  }

  const saveMemoryEdit = async () => {
    if (!selected || !editingMemoryId || !memoryDraft.text.trim()) return
    setMemoryBusy(true)
    try {
      await updateTutorMemory(selected.id, editingMemoryId, memoryDraft)
      setEditingMemoryId(null)
      setMemoryDraft(emptyMemoryDraft)
      await reloadMemory()
      setMemoryStatus('导师记忆已更新。')
    } catch (error) {
      setMemoryStatus(error instanceof Error ? error.message : String(error))
    } finally {
      setMemoryBusy(false)
    }
  }

  const toggleMemoryStatus = async (entry: TutorMemoryEntry) => {
    if (!selected) return
    setMemoryBusy(true)
    try {
      if (entry.status === 'active') {
        await resolveTutorMemory(selected.id, entry.id)
      } else {
        await updateTutorMemory(selected.id, entry.id, { status: 'active' })
      }
      await reloadMemory()
    } catch (error) {
      setMemoryStatus(error instanceof Error ? error.message : String(error))
    } finally {
      setMemoryBusy(false)
    }
  }

  const removeMemory = async (entry: TutorMemoryEntry) => {
    if (!selected || !window.confirm('删除这条导师私有记忆？')) return
    setMemoryBusy(true)
    try {
      await deleteTutorMemory(selected.id, entry.id)
      await reloadMemory()
    } catch (error) {
      setMemoryStatus(error instanceof Error ? error.message : String(error))
    } finally {
      setMemoryBusy(false)
    }
  }

  const resetMemory = async () => {
    if (!selected || !window.confirm(`清空“${selected.name}”的全部私有记忆？学习者记忆不会受到影响。`)) return
    setMemoryBusy(true)
    try {
      await resetTutorMemory(selected.id)
      await reloadMemory()
      setMemoryStatus('导师私有记忆已清空。')
    } catch (error) {
      setMemoryStatus(error instanceof Error ? error.message : String(error))
    } finally {
      setMemoryBusy(false)
    }
  }

  return (
    <div className="flex min-h-0 flex-1 overflow-hidden bg-white">
      <aside className="w-64 shrink-0 overflow-y-auto border-r border-gray-200 p-3">
        <div className="mb-3 flex items-center justify-between px-1">
          <div>
            <h1 className="text-base font-semibold text-gray-900">辅导机器人</h1>
            <p className="mt-0.5 text-xs text-gray-500">持久导师身份</p>
          </div>
          <button
            type="button"
            className="inline-flex h-8 w-8 items-center justify-center rounded-md text-gray-500 hover:bg-gray-100 hover:text-gray-900"
            title="新建导师"
            aria-label="新建导师"
            onClick={startCreate}
          >
            <Plus size={17} />
          </button>
        </div>
        <div className="space-y-1">
          {tutors.map((tutor) => (
            <div
              key={tutor.id}
              className={`flex items-center rounded-md ${
                !creating && tutor.id === selectedId ? 'bg-gray-100 text-gray-900' : 'text-gray-700 hover:bg-gray-50'
              }`}
            >
              <button type="button" className="flex min-w-0 flex-1 items-center gap-3 px-3 py-2 text-left" onClick={() => choose(tutor)}>
                <Bot size={17} className="shrink-0 text-blue-600" />
                <span className="min-w-0">
                  <span className="block truncate text-sm font-medium">{tutor.name}</span>
                  <span className="block truncate text-xs text-gray-500">{tutorSoulSummary(tutor.soul_markdown)}</span>
                </span>
              </button>
              {!tutor.built_in && (
                <button
                  type="button"
                  disabled={busy}
                  className="mr-1 flex h-8 w-8 shrink-0 items-center justify-center rounded-md text-gray-400 hover:bg-red-50 hover:text-red-600 disabled:opacity-50"
                  title={`删除“${tutor.name}”`}
                  aria-label={`删除“${tutor.name}”`}
                  onClick={() => void remove(tutor)}
                >
                  <Trash2 size={15} />
                </button>
              )}
            </div>
          ))}
          {tutors.length === 0 && (
            <div className="px-2 py-5 text-center">
              <p className="text-xs leading-5 text-gray-500">创建一位导师，并用 Soul Markdown 定义其教学方式。</p>
              <button
                type="button"
                className="mt-3 inline-flex h-8 items-center gap-1.5 rounded-md bg-blue-600 px-3 text-xs font-medium text-white hover:bg-blue-700"
                onClick={startCreate}
              >
                <Plus size={14} />
                新建导师
              </button>
            </div>
          )}
        </div>
      </aside>

      <main className="min-w-0 flex-1 overflow-y-auto px-8 py-6">
        <div className="mx-auto max-w-4xl">
          <div className="mb-6 flex items-start justify-between gap-4 border-b border-gray-200 pb-5">
            <div>
              <h2 className="text-xl font-semibold text-gray-900">{creating ? '新建导师' : selected?.name ?? '选择导师'}</h2>
              <p className="mt-1 text-sm text-gray-500">设置导师的长期 Soul、默认行为和可使用的能力。</p>
            </div>
            {(onReturnToOnboarding || (selected && !creating)) && (
              <div className="ml-auto flex items-center gap-3">
                {onReturnToOnboarding && (
                  <button
                    type="button"
                    className="inline-flex h-9 items-center rounded-md border border-gray-300 bg-white px-3 text-sm font-medium text-gray-700 hover:bg-gray-50"
                    onClick={onReturnToOnboarding}
                  >
                    返回使用引导
                  </button>
                )}
                {selected && !creating && (
                  <>
                    <div className="flex rounded-md bg-gray-100 p-0.5">
                      <button type="button" className={`h-8 rounded px-3 text-xs ${workspaceView === 'profile' ? 'bg-white text-gray-900 shadow-sm' : 'text-gray-500'}`} onClick={() => setWorkspaceView('profile')}>配置</button>
                      <button type="button" className={`h-8 rounded px-3 text-xs ${workspaceView === 'memory' ? 'bg-white text-gray-900 shadow-sm' : 'text-gray-500'}`} onClick={() => setWorkspaceView('memory')}>连续性</button>
                    </div>
                    <button
                      type="button"
                      className="inline-flex items-center gap-2 rounded-md bg-gray-900 px-3 py-2 text-sm text-white hover:bg-gray-800"
                      onClick={() => onStartConversation(selected.id)}
                    >
                      <MessageSquare size={16} />
                      开始对话
                    </button>
                  </>
                )}
              </div>
            )}
          </div>

          {(creating || selected) && (creating || workspaceView === 'profile') && (
            <div className="space-y-5">
              <div className="grid gap-4 md:grid-cols-3">
                <Field label="导师名称">
                  <input className={inputClass} value={draft.name} onChange={(event) => setDraft({ ...draft, name: event.target.value })} />
                </Field>
                <Field label="默认模式">
                  <select className={inputClass} value={draft.default_capability} onChange={(event) => setDraft({ ...draft, default_capability: event.target.value })}>
                    {tutorCapabilities.filter((item) => draft.allowed_capabilities.includes(item)).map((item) => (
                      <option key={item} value={item}>{capabilityLabel(item)}</option>
                    ))}
                  </select>
                </Field>
                <Field label="默认模型">
                  <select
                    className={inputClass}
                    value={draft.default_model_config_id ?? ''}
                    onChange={(event) => setDraft({ ...draft, default_model_config_id: event.target.value || null })}
                  >
                    <option value="">跟随全局默认</option>
                    {modelConfigs.map((config) => (
                      <option key={config.id} value={config.id}>{config.name} · {config.model}</option>
                    ))}
                  </select>
                </Field>
              </div>
              <div>
                <div className="mb-2 flex items-end justify-between gap-4">
                  <div>
                    <div className="text-sm font-medium text-gray-800">导师 Soul</div>
                    <p className="mt-1 text-xs text-gray-500">用 Markdown 定义稳定身份、教学风格、原则和边界。当前学习目标由导师记忆维护。</p>
                  </div>
                  <div className="flex shrink-0 rounded-md bg-gray-100 p-0.5">
                    <button
                      type="button"
                      className={`inline-flex h-8 items-center gap-1.5 rounded px-2.5 text-xs ${soulView === 'edit' ? 'bg-white text-gray-900 shadow-sm' : 'text-gray-500'}`}
                      onClick={() => setSoulView('edit')}
                    >
                      <Pencil size={14} />
                      编辑
                    </button>
                    <button
                      type="button"
                      className={`inline-flex h-8 items-center gap-1.5 rounded px-2.5 text-xs ${soulView === 'preview' ? 'bg-white text-gray-900 shadow-sm' : 'text-gray-500'}`}
                      onClick={() => setSoulView('preview')}
                    >
                      <Eye size={14} />
                      预览
                    </button>
                  </div>
                </div>
                {soulView === 'edit' ? (
                  <textarea
                    className={`${inputClass} min-h-80 resize-y font-mono leading-6`}
                    value={draft.soul_markdown}
                    onChange={(event) => setDraft({ ...draft, soul_markdown: event.target.value })}
                  />
                ) : (
                  <div className="min-h-80 rounded-md border border-gray-200 bg-gray-50 px-5 py-4">
                    <MarkdownMessage text={draft.soul_markdown} />
                  </div>
                )}
              </div>

              <div>
                <div className="mb-2 text-sm font-medium text-gray-800">可用能力</div>
                <div className="flex flex-wrap gap-2">
                  {tutorCapabilities.map((capability) => {
                    const checked = draft.allowed_capabilities.includes(capability)
                    return (
                      <label key={capability} className={`flex items-center gap-2 rounded-md border px-3 py-2 text-sm ${checked ? 'border-blue-300 bg-blue-50 text-blue-800' : 'border-gray-200 text-gray-600'}`}>
                        <input
                          type="checkbox"
                          checked={checked}
                          onChange={() => setDraft(toggleCapability(draft, capability))}
                        />
                        {capabilityLabel(capability)}
                      </label>
                    )
                  })}
                </div>
              </div>

              <div className="flex flex-wrap gap-6 border-t border-gray-200 pt-5 text-sm text-gray-700">
                <label className="flex items-center gap-2">
                  <input type="checkbox" checked={draft.learner_memory_access} onChange={(event) => setDraft({ ...draft, learner_memory_access: event.target.checked })} />
                  允许读取学习者记忆
                </label>
                <label className="flex items-center gap-2">
                  <input type="checkbox" checked={draft.autonomous_memory} onChange={(event) => setDraft({ ...draft, autonomous_memory: event.target.checked })} />
                  允许自主维护导师记忆
                </label>
                <label className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={draft.resource_permissions.notebook}
                    onChange={(event) => setDraft({
                      ...draft,
                      resource_permissions: { ...draft.resource_permissions, notebook: event.target.checked },
                    })}
                  />
                  允许访问 Notebook
                </label>
                <label className="flex items-center gap-2">
                  <input
                    type="checkbox"
                    checked={draft.resource_permissions.space}
                    onChange={(event) => setDraft({
                      ...draft,
                      resource_permissions: { ...draft.resource_permissions, space: event.target.checked },
                    })}
                  />
                  允许访问空间
                </label>
              </div>

              <div>
                <div className="mb-2 text-sm font-medium text-gray-800">允许访问的知识库</div>
                {knowledgeBases.length === 0 ? (
                  <p className="text-sm text-gray-500">当前没有知识库。</p>
                ) : (
                  <div className="grid gap-2 sm:grid-cols-2">
                    {knowledgeBases.map((knowledgeBase) => {
                      const checked = draft.resource_permissions.knowledge_base_ids.includes(knowledgeBase.id)
                      return (
                        <label key={knowledgeBase.id} className={`flex items-center gap-2 rounded-md border px-3 py-2 text-sm ${checked ? 'border-blue-300 bg-blue-50 text-blue-800' : 'border-gray-200 text-gray-600'}`}>
                          <input
                            type="checkbox"
                            checked={checked}
                            onChange={() => setDraft(toggleKnowledgeBase(draft, knowledgeBase.id))}
                          />
                          <span className="truncate">{knowledgeBase.name}</span>
                        </label>
                      )
                    })}
                  </div>
                )}
              </div>

              <div className="flex items-center gap-3">
                <button type="button" disabled={busy} className="inline-flex items-center gap-2 rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:opacity-50" onClick={() => void save()}>
                  <Save size={16} />
                  保存
                </button>
                {status && <span className="text-sm text-gray-600">{status}</span>}
              </div>
            </div>
          )}

          {selected && !creating && workspaceView === 'memory' && (
            <div className="space-y-6">
              <section className="border-b border-gray-200 pb-5">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <h3 className="text-base font-semibold text-gray-900">导师私有记忆</h3>
                    <p className="mt-1 text-sm text-gray-500">只属于这位导师的承诺、未完成事项、课程计划、反思和教学策略。</p>
                  </div>
                  <div className="flex items-center gap-2">
                    <label className="flex items-center gap-2 text-xs text-gray-600">
                      <input type="checkbox" checked={includeResolved} onChange={(event) => setIncludeResolved(event.target.checked)} />
                      显示已解决
                    </label>
                    <button type="button" disabled={memoryBusy || memoryEntries.length === 0} className="inline-flex h-8 w-8 items-center justify-center rounded-md text-gray-500 hover:bg-red-50 hover:text-red-600 disabled:opacity-40" title="清空导师记忆" aria-label="清空导师记忆" onClick={() => void resetMemory()}>
                      <Trash2 size={15} />
                    </button>
                  </div>
                </div>
                <div className="mt-4 flex gap-6 text-sm text-gray-600">
                  <span><strong className="text-gray-900">{memoryEntries.filter((entry) => entry.status === 'active').length}</strong> 项进行中</span>
                  <span><strong className="text-gray-900">{memoryEntries.filter((entry) => entry.status === 'resolved').length}</strong> 项已解决</span>
                </div>
              </section>

              {!editingMemoryId && (
                <section>
                  <h3 className="mb-3 text-sm font-medium text-gray-800">添加连续性记忆</h3>
                  <MemoryEditor draft={memoryDraft} onChange={setMemoryDraft} />
                  <div className="mt-3 flex items-center gap-3">
                    <button type="button" disabled={memoryBusy || !memoryDraft.text.trim()} className="inline-flex items-center gap-2 rounded-md bg-blue-600 px-3 py-2 text-sm text-white hover:bg-blue-700 disabled:opacity-50" onClick={() => void addMemory()}>
                      <Plus size={15} />
                      添加
                    </button>
                    {memoryStatus && <span className="text-sm text-gray-600">{memoryStatus}</span>}
                  </div>
                </section>
              )}

              <section>
                <div className="mb-2 flex items-center gap-2 text-sm font-medium text-gray-800">
                  <History size={16} />
                  记忆记录
                </div>
                {memoryBusy && memoryEntries.length === 0 ? (
                  <p className="py-8 text-center text-sm text-gray-500">正在加载...</p>
                ) : memoryEntries.length === 0 ? (
                  <p className="py-8 text-center text-sm text-gray-500">这位导师还没有私有记忆。</p>
                ) : (
                  <div className="divide-y divide-gray-200 border-y border-gray-200">
                    {memoryEntries.map((entry) => (
                      <div key={entry.id} className="py-4">
                        {editingMemoryId === entry.id ? (
                          <>
                            <MemoryEditor draft={memoryDraft} onChange={setMemoryDraft} />
                            <div className="mt-3 flex gap-2">
                              <button type="button" disabled={memoryBusy} className="inline-flex h-8 items-center gap-1.5 rounded-md bg-blue-600 px-3 text-xs text-white" onClick={() => void saveMemoryEdit()}><Save size={14} />保存</button>
                              <button type="button" className="inline-flex h-8 items-center gap-1.5 rounded-md px-3 text-xs text-gray-600 hover:bg-gray-100" onClick={() => { setEditingMemoryId(null); setMemoryDraft(emptyMemoryDraft) }}><X size={14} />取消</button>
                            </div>
                          </>
                        ) : (
                          <div className="flex items-start gap-4">
                            <ListTodo size={17} className={`mt-0.5 shrink-0 ${entry.status === 'active' ? 'text-blue-600' : 'text-gray-400'}`} />
                            <div className="min-w-0 flex-1">
                              <div className="flex flex-wrap items-center gap-2">
                                <span className="text-xs font-medium text-blue-700">{memoryKindLabel(entry.kind)}</span>
                                <span className="text-xs text-gray-400">{entry.status === 'active' ? '进行中' : '已解决'}</span>
                              </div>
                              <p className={`mt-1 text-sm leading-6 ${entry.status === 'resolved' ? 'text-gray-500 line-through' : 'text-gray-900'}`}>{entry.text}</p>
                              {entry.next_action && <p className="mt-1 text-xs text-gray-500">下一步：{entry.next_action}</p>}
                              {entry.source_session_id && <p className="mt-1 text-xs text-gray-400">来源会话：{entry.source_session_id.slice(0, 8)}</p>}
                            </div>
                            <div className="flex shrink-0 items-center gap-1">
                              <button type="button" className="inline-flex h-8 w-8 items-center justify-center rounded-md text-gray-500 hover:bg-gray-100" title="编辑" aria-label="编辑" onClick={() => beginMemoryEdit(entry)}><Pencil size={14} /></button>
                              <button type="button" disabled={memoryBusy} className="inline-flex h-8 w-8 items-center justify-center rounded-md text-gray-500 hover:bg-gray-100" title={entry.status === 'active' ? '标记已解决' : '重新打开'} aria-label={entry.status === 'active' ? '标记已解决' : '重新打开'} onClick={() => void toggleMemoryStatus(entry)}>{entry.status === 'active' ? <Check size={15} /> : <RotateCcw size={15} />}</button>
                              <button type="button" disabled={memoryBusy} className="inline-flex h-8 w-8 items-center justify-center rounded-md text-gray-500 hover:bg-red-50 hover:text-red-600" title="删除" aria-label="删除" onClick={() => void removeMemory(entry)}><Trash2 size={14} /></button>
                            </div>
                          </div>
                        )}
                      </div>
                    ))}
                  </div>
                )}
              </section>
            </div>
          )}
        </div>
      </main>
    </div>
  )
}

const inputClass = 'w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-900 outline-none focus:border-blue-500 focus:ring-2 focus:ring-blue-100'

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return <label className="block"><span className="mb-1.5 block text-sm font-medium text-gray-800">{label}</span>{children}</label>
}

function MemoryEditor({
  draft,
  onChange,
}: {
  draft: TutorMemoryDraft
  onChange: (draft: TutorMemoryDraft) => void
}) {
  return (
    <div className="grid gap-3 md:grid-cols-[160px_minmax(0,1fr)]">
      <select className={inputClass} value={draft.kind} onChange={(event) => onChange({ ...draft, kind: event.target.value as TutorMemoryKind })}>
        <option value="commitment">承诺</option>
        <option value="open_loop">未完成事项</option>
        <option value="lesson_plan">课程计划</option>
        <option value="reflection">教学反思</option>
        <option value="strategy">教学策略</option>
      </select>
      <input className={inputClass} value={draft.text} placeholder="记录这位导师需要延续的事项" onChange={(event) => onChange({ ...draft, text: event.target.value })} />
      <div className="md:col-start-2">
        <input className={inputClass} value={draft.next_action ?? ''} placeholder="下一步行动（可选）" onChange={(event) => onChange({ ...draft, next_action: event.target.value })} />
      </div>
    </div>
  )
}

function profileToDraft(profile: TutorProfile): TutorDraft {
  return {
    name: profile.name,
    soul_markdown: profile.soul_markdown,
    default_model_config_id: profile.default_model_config_id ?? null,
    default_capability: profile.default_capability,
    allowed_capabilities: [...profile.allowed_capabilities],
    learner_memory_access: profile.learner_memory_access,
    autonomous_memory: profile.autonomous_memory,
    resource_permissions: {
      knowledge_base_ids: [...profile.resource_permissions.knowledge_base_ids],
      notebook: profile.resource_permissions.notebook,
      space: profile.resource_permissions.space,
    },
  }
}

function toggleKnowledgeBase(draft: TutorDraft, knowledgeBaseId: string): TutorDraft {
  const selected = draft.resource_permissions.knowledge_base_ids.includes(knowledgeBaseId)
  return {
    ...draft,
    resource_permissions: {
      ...draft.resource_permissions,
      knowledge_base_ids: selected
        ? draft.resource_permissions.knowledge_base_ids.filter((id) => id !== knowledgeBaseId)
        : [...draft.resource_permissions.knowledge_base_ids, knowledgeBaseId],
    },
  }
}

function toggleCapability(draft: TutorDraft, capability: string): TutorDraft {
  const enabled = draft.allowed_capabilities.includes(capability)
  if (enabled && draft.allowed_capabilities.length === 1) return draft
  const allowed = enabled
    ? draft.allowed_capabilities.filter((item) => item !== capability)
    : [...draft.allowed_capabilities, capability]
  return {
    ...draft,
    allowed_capabilities: allowed,
    default_capability: allowed.includes(draft.default_capability) ? draft.default_capability : (allowed[0] ?? 'chat'),
  }
}

function capabilityLabel(capability: string) {
  return ({ chat: '聊天', quiz: '测验', research: '调研', organize: '整理' } as Record<string, string>)[capability] ?? capability
}

function memoryKindLabel(kind: TutorMemoryKind) {
  return ({
    commitment: '承诺',
    open_loop: '未完成事项',
    lesson_plan: '课程计划',
    reflection: '教学反思',
    strategy: '教学策略',
  } as Record<TutorMemoryKind, string>)[kind]
}
