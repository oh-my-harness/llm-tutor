import { useState, type ReactNode } from 'react'
import {
  BookOpen,
  Bot,
  Brain,
  Check,
  Edit3,
  FileQuestion,
  FileText,
  Grid2X2,
  Library,
  MessageSquare,
  PanelLeftClose,
  PanelLeftOpen,
  PencilLine,
  Settings,
  Sparkles,
  Trash2,
  X,
} from 'lucide-react'

export type AppView =
  | 'chat'
  | 'tutor'
  | 'writing'
  | 'books'
  | 'knowledge'
  | 'quiz'
  | 'space'
  | 'memory'
  | 'settings'

interface RecentSession {
  id: string
  title: string
}

interface Props {
  activeView: AppView
  collapsed: boolean
  recentSessions: RecentSession[]
  onNavigate: (view: AppView) => void
  onSelectSession: (id: string) => void
  onRenameSession: (id: string, title: string) => void
  onDeleteSession: (id: string) => void
  onToggleCollapsed: () => void
}

const navItems: Array<{
  key: AppView
  label: string
  icon: typeof MessageSquare
}> = [
  { key: 'chat', label: '聊天', icon: MessageSquare },
  { key: 'tutor', label: '辅导机器人', icon: Bot },
  { key: 'writing', label: '智能写作', icon: PencilLine },
  { key: 'books', label: '书籍', icon: Library },
  { key: 'knowledge', label: '知识库', icon: BookOpen },
  { key: 'quiz', label: 'Quiz', icon: FileQuestion },
  { key: 'space', label: '空间', icon: Grid2X2 },
  { key: 'memory', label: '记忆', icon: Brain },
]

export function Sidebar({
  activeView,
  collapsed,
  recentSessions,
  onNavigate,
  onSelectSession,
  onRenameSession,
  onDeleteSession,
  onToggleCollapsed,
}: Props) {
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null)
  const [editingTitle, setEditingTitle] = useState('')

  const startEditing = (session: RecentSession) => {
    setEditingSessionId(session.id)
    setEditingTitle(session.title)
  }

  const submitEditing = () => {
    if (!editingSessionId) return
    const nextTitle = editingTitle.trim()
    if (nextTitle) {
      onRenameSession(editingSessionId, nextTitle)
    }
    setEditingSessionId(null)
    setEditingTitle('')
  }

  const cancelEditing = () => {
    setEditingSessionId(null)
    setEditingTitle('')
  }

  return (
    <aside
      className={`flex h-screen shrink-0 flex-col border-r border-gray-200 bg-white transition-[width] duration-200 ${
        collapsed ? 'w-16' : 'w-72'
      }`}
    >
      <div className={`flex items-center ${collapsed ? 'justify-center px-2 py-4' : 'justify-between px-5 py-5'}`}>
        <button
          className={`flex items-center gap-2 text-left ${collapsed ? 'justify-center' : ''}`}
          onClick={() => onNavigate('chat')}
          title="Tutor Agent"
        >
          <div className="flex h-9 w-9 items-center justify-center rounded-lg border border-gray-200 bg-gray-50 text-blue-600">
            <Sparkles size={20} />
          </div>
          {!collapsed && <div>
            <div className="text-lg font-semibold text-gray-900">Tutor Agent</div>
            <div className="text-xs text-gray-500">AI learning workspace</div>
          </div>}
        </button>
        {!collapsed && (
          <button
            className="rounded p-2 text-gray-500 hover:bg-gray-100"
            title="Collapse sidebar"
            onClick={onToggleCollapsed}
          >
            <PanelLeftClose size={18} />
          </button>
        )}
      </div>

      {collapsed && (
        <button
          className="mx-auto mb-2 rounded p-2 text-gray-500 hover:bg-gray-100"
          title="Expand sidebar"
          onClick={onToggleCollapsed}
        >
          <PanelLeftOpen size={18} />
        </button>
      )}

      <nav className={`space-y-1 ${collapsed ? 'px-2' : 'px-3'}`}>
        {navItems.map((item) => {
          const Icon = item.icon
          const active = activeView === item.key
          return (
            <button
              key={item.key}
              title={item.label}
              className={`flex w-full items-center rounded-lg py-2.5 text-left text-sm ${
                collapsed ? 'justify-center px-2' : 'gap-3 px-3'
              } ${
                active
                  ? 'bg-gray-900 text-white'
                  : 'text-gray-700 hover:bg-gray-100 hover:text-gray-900'
              }`}
              onClick={() => onNavigate(item.key)}
            >
              <Icon size={19} />
              {!collapsed && <span>{item.label}</span>}
            </button>
          )
        })}
      </nav>

      <div className={`mt-6 flex-1 overflow-y-auto ${collapsed ? 'px-2' : 'px-3'}`}>
        {collapsed ? (
          <div className="border-t border-gray-100 pt-3" />
        ) : (
          <>
        <div className="mb-2 px-1 text-xs font-medium uppercase tracking-wide text-gray-500">
          最近
        </div>
        <div className="space-y-1">
          {recentSessions.length === 0 ? (
            <div className="rounded-lg px-3 py-2 text-sm text-gray-400">暂无历史会话</div>
          ) : (
            recentSessions.map((session) => {
              const editing = editingSessionId === session.id
              return (
                <div
                  key={session.id}
                  className="group flex w-full items-center gap-2 rounded-lg px-3 py-2 text-sm text-gray-700 hover:bg-gray-100"
                >
                  <FileText size={16} className="shrink-0 text-gray-500" />
                  {editing ? (
                    <>
                      <input
                        className="min-w-0 flex-1 rounded border border-gray-300 bg-white px-2 py-1 text-sm outline-none focus:border-gray-900"
                        value={editingTitle}
                        autoFocus
                        onChange={(event) => setEditingTitle(event.target.value)}
                        onKeyDown={(event) => {
                          if (event.key === 'Enter') submitEditing()
                          if (event.key === 'Escape') cancelEditing()
                        }}
                      />
                      <IconButton label="Save session name" onClick={submitEditing}>
                        <Check size={15} />
                      </IconButton>
                      <IconButton label="Cancel rename" onClick={cancelEditing}>
                        <X size={15} />
                      </IconButton>
                    </>
                  ) : (
                    <>
                      <button
                        type="button"
                        className="min-w-0 flex-1 truncate text-left"
                        onClick={() => onSelectSession(session.id)}
                      >
                        {session.title}
                      </button>
                      <div className="flex shrink-0 items-center gap-1 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
                        <IconButton label="Rename session" onClick={() => startEditing(session)}>
                          <Edit3 size={15} />
                        </IconButton>
                        <IconButton label="Delete session" onClick={() => onDeleteSession(session.id)}>
                          <Trash2 size={15} />
                        </IconButton>
                      </div>
                    </>
                  )}
                </div>
              )
            })
          )}
        </div>
          </>
        )}
      </div>

      <div className={`border-t border-gray-200 ${collapsed ? 'p-2' : 'p-3'}`}>
        <button
          title="设置"
          className={`flex w-full items-center rounded-lg py-2.5 text-left text-sm ${
            collapsed ? 'justify-center px-2' : 'gap-3 px-3'
          } ${
            activeView === 'settings'
              ? 'bg-gray-900 text-white'
              : 'text-gray-700 hover:bg-gray-100 hover:text-gray-900'
          }`}
          onClick={() => onNavigate('settings')}
        >
          <Settings size={19} />
          {!collapsed && <span>设置</span>}
        </button>
        {!collapsed && <div className="mt-3 px-3 text-xs text-gray-500">v0.1.0</div>}
      </div>
    </aside>
  )
}

function IconButton({ label, onClick, children }: { label: string; onClick: () => void; children: ReactNode }) {
  return (
    <button
      type="button"
      title={label}
      className="rounded p-1 text-gray-500 hover:bg-gray-200 hover:text-gray-900"
      onClick={(event) => {
        event.stopPropagation()
        onClick()
      }}
    >
      {children}
    </button>
  )
}
