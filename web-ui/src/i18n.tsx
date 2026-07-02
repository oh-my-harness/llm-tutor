import { createContext, useContext, type ReactNode } from 'react'

export type UiLanguage = 'zh-CN' | 'en-US'

export type TranslationKey =
  | 'app.subtitle'
  | 'nav.chat'
  | 'nav.tutor'
  | 'nav.writing'
  | 'nav.books'
  | 'nav.knowledge'
  | 'nav.space'
  | 'nav.memory'
  | 'nav.settings'
  | 'nav.recent'
  | 'nav.noRecent'
  | 'nav.collapse'
  | 'nav.expand'
  | 'chat.title'
  | 'chat.subtitle'
  | 'chat.new'
  | 'chat.input.placeholder'
  | 'chat.attachments'
  | 'chat.knowledge.none'
  | 'chat.knowledge.none.description'
  | 'chat.knowledge.use.description'
  | 'chat.notebook.description'
  | 'chat.space.searchPlaceholder'
  | 'chat.space.updating'
  | 'chat.space.noMatching'
  | 'chat.model.select'
  | 'chat.model.none'
  | 'chat.model.configureFirst'
  | 'chat.send'
  | 'chat.stop'
  | 'mention.filter.all'
  | 'mention.filter.notes'
  | 'mention.filter.quizzes'
  | 'mention.filter.questions'
  | 'settings.title'
  | 'settings.subtitle'
  | 'settings.saved'
  | 'settings.tabs.appearance'
  | 'settings.tabs.llm'
  | 'settings.tabs.embedding'
  | 'settings.tabs.search'
  | 'settings.tabs.governance'
  | 'settings.appearance.title'
  | 'settings.appearance.description'
  | 'settings.llm.description'
  | 'settings.embedding.description'
  | 'settings.search.description'
  | 'settings.governance.description'
  | 'settings.language.title'
  | 'settings.language.description.zh'
  | 'settings.language.description.en'
  | 'settings.language.english'
  | 'settings.language.chinese'
  | 'cap.chat'
  | 'cap.chat.description'
  | 'cap.deepSolve'
  | 'cap.deepSolve.description'
  | 'cap.codeExec'
  | 'cap.codeExec.description'
  | 'cap.quiz'
  | 'cap.quiz.description'
  | 'cap.research'
  | 'cap.research.description'
  | 'cap.organize'
  | 'cap.organize.description'

const translations: Record<UiLanguage, Record<TranslationKey, string>> = {
  'zh-CN': {
    'app.subtitle': 'AI 学习工作区',
    'nav.chat': '聊天',
    'nav.tutor': '辅导机器人',
    'nav.writing': '智能写作',
    'nav.books': '书籍',
    'nav.knowledge': '知识库',
    'nav.space': '空间',
    'nav.memory': '记忆',
    'nav.settings': '设置',
    'nav.recent': '最近',
    'nav.noRecent': '暂无历史会话',
    'nav.collapse': '收起侧边栏',
    'nav.expand': '展开侧边栏',
    'chat.title': '聊天',
    'chat.subtitle': '提问、运行工具、查看轨迹。',
    'chat.new': '新对话',
    'chat.input.placeholder': '今天我能帮您什么？',
    'chat.attachments': '附件',
    'chat.knowledge.none': '不关联知识库',
    'chat.knowledge.none.description': '仅使用当前对话上下文',
    'chat.knowledge.use.description': '关联此知识库进行检索',
    'chat.notebook.description': '以纯 Markdown 文本搜索 Notebook',
    'chat.space.searchPlaceholder': '搜索笔记和测验...',
    'chat.space.updating': '更新中...',
    'chat.space.noMatching': '没有匹配内容。',
    'chat.model.select': '选择模型',
    'chat.model.none': '暂无模型配置',
    'chat.model.configureFirst': '请先到设置中添加 LLM 配置',
    'chat.send': '发送',
    'chat.stop': '停止生成',
    'mention.filter.all': '全部',
    'mention.filter.notes': '笔记',
    'mention.filter.quizzes': '测验',
    'mention.filter.questions': '题目',
    'settings.title': '设置',
    'settings.subtitle': '调整外观、配置模型服务、查看内置工具。',
    'settings.saved': '所有更改已保存',
    'settings.tabs.appearance': '外观',
    'settings.tabs.llm': 'LLM',
    'settings.tabs.embedding': '嵌入模型',
    'settings.tabs.search': '搜索',
    'settings.tabs.governance': '能力',
    'settings.appearance.title': '界面外观',
    'settings.appearance.description': '调整界面语言和视觉偏好。',
    'settings.llm.description': '配置对话模型服务，可新增多个服务配置。',
    'settings.embedding.description': '配置知识库检索使用的嵌入模型。',
    'settings.search.description': '配置 agent web_search 工具使用的搜索服务。',
    'settings.governance.description': '配置预算和工具执行策略。',
    'settings.language.title': '界面语言',
    'settings.language.description.zh': '当前使用中文界面。',
    'settings.language.description.en': '当前使用英文界面。',
    'settings.language.english': 'English',
    'settings.language.chinese': '中文',
    'cap.chat': '聊天',
    'cap.chat.description': '灵活对话，可使用任意工具',
    'cap.deepSolve': '解题',
    'cap.deepSolve.description': '多步推理与问题求解',
    'cap.codeExec': '代码',
    'cap.codeExec.description': '运行代码并验证结果',
    'cap.quiz': 'Quiz',
    'cap.quiz.description': '基于对话或知识库生成测验',
    'cap.research': '研究',
    'cap.research.description': '搜索、阅读并生成带引用的研究报告',
    'cap.organize': '整理',
    'cap.organize.description': '搜索并整理 Notebook 笔记',
  },
  'en-US': {
    'app.subtitle': 'AI learning workspace',
    'nav.chat': 'Chat',
    'nav.tutor': 'Tutor Bot',
    'nav.writing': 'Writing',
    'nav.books': 'Books',
    'nav.knowledge': 'Knowledge',
    'nav.space': 'Space',
    'nav.memory': 'Memory',
    'nav.settings': 'Settings',
    'nav.recent': 'Recent',
    'nav.noRecent': 'No recent sessions',
    'nav.collapse': 'Collapse sidebar',
    'nav.expand': 'Expand sidebar',
    'chat.title': 'Chat',
    'chat.subtitle': 'Ask questions, run tools, and inspect traces.',
    'chat.new': 'New chat',
    'chat.input.placeholder': 'How can I help today?',
    'chat.attachments': 'Attachments',
    'chat.knowledge.none': 'No knowledge base',
    'chat.knowledge.none.description': 'Use only the current conversation context',
    'chat.knowledge.use.description': 'Search this knowledge base for context',
    'chat.notebook.description': 'Search Notebook as plain Markdown text',
    'chat.space.searchPlaceholder': 'Search notebook and quizzes...',
    'chat.space.updating': 'Updating...',
    'chat.space.noMatching': 'No matching content.',
    'chat.model.select': 'Select model',
    'chat.model.none': 'No model profiles',
    'chat.model.configureFirst': 'Add an LLM profile in Settings first',
    'chat.send': 'Send',
    'chat.stop': 'Stop generation',
    'mention.filter.all': 'All',
    'mention.filter.notes': 'Notes',
    'mention.filter.quizzes': 'Quizzes',
    'mention.filter.questions': 'Questions',
    'settings.title': 'Settings',
    'settings.subtitle': 'Adjust appearance, configure model services, and inspect built-in tools.',
    'settings.saved': 'All changes saved',
    'settings.tabs.appearance': 'Appearance',
    'settings.tabs.llm': 'LLM',
    'settings.tabs.embedding': 'Embedding',
    'settings.tabs.search': 'Search',
    'settings.tabs.governance': 'Capabilities',
    'settings.appearance.title': 'Appearance',
    'settings.appearance.description': 'Adjust interface language and visual preferences.',
    'settings.llm.description': 'Configure chat model services and add multiple service profiles.',
    'settings.embedding.description': 'Configure embedding models used by knowledge-base retrieval.',
    'settings.search.description': 'Configure search services used by the agent web_search tool.',
    'settings.governance.description': 'Configure budget and tool execution policies.',
    'settings.language.title': 'Interface language',
    'settings.language.description.zh': 'Chinese interface is active.',
    'settings.language.description.en': 'English interface is active.',
    'settings.language.english': 'English',
    'settings.language.chinese': '中文',
    'cap.chat': 'Chat',
    'cap.chat.description': 'Flexible conversation with any available tool',
    'cap.deepSolve': 'Deep Solve',
    'cap.deepSolve.description': 'Multi-step reasoning and problem solving',
    'cap.codeExec': 'Code',
    'cap.codeExec.description': 'Run code and verify results',
    'cap.quiz': 'Quiz',
    'cap.quiz.description': 'Generate quizzes from conversations or knowledge bases',
    'cap.research': 'Research',
    'cap.research.description': 'Search, read, and produce cited research reports',
    'cap.organize': 'Organize',
    'cap.organize.description': 'Search and organize Notebook notes',
  },
}

const I18nContext = createContext<{
  language: UiLanguage
  t: (key: TranslationKey) => string
}>({
  language: 'zh-CN',
  t: (key) => translations['zh-CN'][key],
})

export function translate(language: UiLanguage, key: TranslationKey) {
  const normalizedLanguage = language === 'en-US' ? 'en-US' : 'zh-CN'
  return translations[normalizedLanguage][key]
}

export function I18nProvider({
  language,
  children,
}: {
  language: UiLanguage
  children: ReactNode
}) {
  const normalizedLanguage = language === 'en-US' ? 'en-US' : 'zh-CN'
  return (
    <I18nContext.Provider
      value={{
        language: normalizedLanguage,
        t: (key) => translations[normalizedLanguage][key],
      }}
    >
      {children}
    </I18nContext.Provider>
  )
}

export function useI18n() {
  return useContext(I18nContext)
}
