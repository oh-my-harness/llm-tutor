import { useEffect, useState } from 'react'
import { Bot, MessageSquare, Plus, Save, Trash2 } from 'lucide-react'
import {
  archiveTutor,
  createTutor,
  tutorCapabilities,
  updateTutor,
  type TutorDraft,
  type TutorProfile,
} from '../tutorTypes'

interface Props {
  tutors: TutorProfile[]
  onChanged: () => Promise<void>
  onStartConversation: (tutorId: string) => void
}

const emptyDraft: TutorDraft = {
  name: '',
  role: '',
  goal: '',
  default_capability: 'chat',
  allowed_capabilities: ['chat', 'deep_solve', 'quiz', 'research', 'organize'],
  learner_memory_access: true,
  autonomous_memory: true,
  resource_permissions: { knowledge_base_ids: [], notebook: true, space: true },
}

export function TutorPage({ tutors, onChanged, onStartConversation }: Props) {
  const [selectedId, setSelectedId] = useState<string | null>(tutors[0]?.id ?? null)
  const [creating, setCreating] = useState(false)
  const [draft, setDraft] = useState<TutorDraft>(emptyDraft)
  const [busy, setBusy] = useState(false)
  const [status, setStatus] = useState('')
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

  const choose = (tutor: TutorProfile) => {
    setCreating(false)
    setSelectedId(tutor.id)
    setDraft(profileToDraft(tutor))
    setStatus('')
  }

  const startCreate = () => {
    setCreating(true)
    setSelectedId(null)
    setDraft({ ...emptyDraft, resource_permissions: { ...emptyDraft.resource_permissions } })
    setStatus('')
  }

  const save = async () => {
    if (!draft.name.trim() || !draft.role.trim()) {
      setStatus('请填写导师名称和角色说明。')
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

  const remove = async () => {
    if (!selected || selected.built_in || !window.confirm(`归档“${selected.name}”？已有会话会继续保留。`)) return
    setBusy(true)
    try {
      await archiveTutor(selected.id)
      await onChanged()
      setSelectedId(null)
      setStatus('导师已归档。')
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error))
    } finally {
      setBusy(false)
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
            <button
              key={tutor.id}
              type="button"
              className={`flex w-full items-center gap-3 rounded-md px-3 py-2 text-left ${
                !creating && tutor.id === selectedId ? 'bg-gray-100 text-gray-900' : 'text-gray-700 hover:bg-gray-50'
              }`}
              onClick={() => choose(tutor)}
            >
              <Bot size={17} className="shrink-0 text-blue-600" />
              <span className="min-w-0">
                <span className="block truncate text-sm font-medium">{tutor.name}</span>
                <span className="block truncate text-xs text-gray-500">{tutor.goal || '尚未设置目标'}</span>
              </span>
            </button>
          ))}
        </div>
      </aside>

      <main className="min-w-0 flex-1 overflow-y-auto px-8 py-6">
        <div className="mx-auto max-w-4xl">
          <div className="mb-6 flex items-start justify-between gap-4 border-b border-gray-200 pb-5">
            <div>
              <h2 className="text-xl font-semibold text-gray-900">{creating ? '新建导师' : selected?.name ?? '选择导师'}</h2>
              <p className="mt-1 text-sm text-gray-500">设置导师的长期角色、学习目标和可使用的能力。</p>
            </div>
            {selected && !creating && (
              <button
                type="button"
                className="inline-flex items-center gap-2 rounded-md bg-gray-900 px-3 py-2 text-sm text-white hover:bg-gray-800"
                onClick={() => onStartConversation(selected.id)}
              >
                <MessageSquare size={16} />
                开始对话
              </button>
            )}
          </div>

          {(creating || selected) && (
            <div className="space-y-5">
              <div className="grid gap-4 md:grid-cols-2">
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
              </div>
              <Field label="角色说明">
                <textarea className={`${inputClass} min-h-24 resize-y`} value={draft.role} onChange={(event) => setDraft({ ...draft, role: event.target.value })} />
              </Field>
              <Field label="当前学习目标">
                <textarea className={`${inputClass} min-h-20 resize-y`} value={draft.goal} onChange={(event) => setDraft({ ...draft, goal: event.target.value })} />
              </Field>

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
              </div>

              <div className="flex items-center gap-3">
                <button type="button" disabled={busy} className="inline-flex items-center gap-2 rounded-md bg-blue-600 px-4 py-2 text-sm text-white hover:bg-blue-700 disabled:opacity-50" onClick={() => void save()}>
                  <Save size={16} />
                  保存
                </button>
                {selected && !selected.built_in && !creating && (
                  <button type="button" disabled={busy} className="inline-flex h-9 w-9 items-center justify-center rounded-md text-gray-500 hover:bg-red-50 hover:text-red-600 disabled:opacity-50" title="归档导师" aria-label="归档导师" onClick={() => void remove()}>
                    <Trash2 size={16} />
                  </button>
                )}
                {status && <span className="text-sm text-gray-600">{status}</span>}
              </div>
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

function profileToDraft(profile: TutorProfile): TutorDraft {
  return {
    name: profile.name,
    role: profile.role,
    goal: profile.goal,
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
  return ({ chat: '聊天', deep_solve: '深度解题', quiz: '测验', research: '调研', organize: '整理' } as Record<string, string>)[capability] ?? capability
}
