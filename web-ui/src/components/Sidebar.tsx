import {
  BookOpen,
  Bot,
  Brain,
  FileText,
  Grid2X2,
  Library,
  MessageSquare,
  PanelLeftClose,
  PanelLeftOpen,
  PencilLine,
  Settings,
  Sparkles,
} from 'lucide-react'

export type AppView =
  | 'chat'
  | 'tutor'
  | 'writing'
  | 'books'
  | 'knowledge'
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
  { key: 'space', label: '空间', icon: Grid2X2 },
  { key: 'memory', label: '记忆', icon: Brain },
]

export function Sidebar({
  activeView,
  collapsed,
  recentSessions,
  onNavigate,
  onSelectSession,
  onToggleCollapsed,
}: Props) {
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
            recentSessions.map((session) => (
              <button
                key={session.id}
                className="flex w-full items-center gap-2 rounded-lg px-3 py-2 text-left text-sm text-gray-700 hover:bg-gray-100"
                onClick={() => onSelectSession(session.id)}
              >
                <FileText size={16} className="shrink-0 text-gray-500" />
                <span className="truncate">{session.title}</span>
              </button>
            ))
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
