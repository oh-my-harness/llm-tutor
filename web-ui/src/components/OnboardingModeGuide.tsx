import { useState, type ReactNode } from 'react'
import { ArrowRight, FileQuestion, FileText, LockKeyhole, MessageSquare, SearchCheck } from 'lucide-react'
import { useI18n } from '../i18n'
import {
  initialOnboardingMode,
  onboardingModeBlock,
  onboardingModes,
  onboardingStarterPrompt,
  type OnboardingMode,
} from '../onboardingModes'
import type { ComposerGuideControl } from '../productGuide'
import type { TutorProfile } from '../tutorTypes'
import { ComposerGuidePreview } from './ComposerGuidePreview'

interface Props {
  selectedTutor: TutorProfile | null
  onStart: (mode: OnboardingMode) => void
}

const modeIcons: Record<OnboardingMode, ReactNode> = {
  chat: <MessageSquare size={17} />,
  research: <SearchCheck size={17} />,
  quiz: <FileQuestion size={17} />,
  organize: <FileText size={17} />,
}

export function OnboardingModeGuide({ selectedTutor, onStart }: Props) {
  const { language } = useI18n()
  const copy = language === 'en-US' ? englishCopy : chineseCopy
  const [guideView, setGuideView] = useState<'composer' | 'modes'>('composer')
  const [composerControl, setComposerControl] = useState<ComposerGuideControl>('mode')
  const [selectedMode, setSelectedMode] = useState<OnboardingMode>(() => initialOnboardingMode(selectedTutor))
  const detail = copy.modes[selectedMode]
  const block = onboardingModeBlock(selectedMode, selectedTutor)
  const unavailableReason = block === 'capability'
    ? copy.unavailable.mode.replace('{name}', selectedTutor?.name ?? '')
    : block === 'notebook'
      ? copy.unavailable.notebook.replace('{name}', selectedTutor?.name ?? '')
      : null
  const starterPrompt = onboardingStarterPrompt(selectedMode, language)

  return (
    <div className="mt-4">
      <div className="mb-3 flex h-9 rounded-md bg-gray-100 p-1" role="tablist" aria-label={copy.guideSelectorLabel}>
        {(['composer', 'modes'] as const).map((view) => (
          <button key={view} type="button" role="tab" aria-selected={guideView === view} className={`flex-1 rounded text-sm font-medium ${guideView === view ? 'bg-white text-gray-900 shadow-sm' : 'text-gray-500 hover:text-gray-900'}`} onClick={() => setGuideView(view)}>
            {copy.guideViews[view]}
          </button>
        ))}
      </div>

      {guideView === 'composer' ? (
        <div>
          <ComposerGuidePreview control={composerControl} onControlChange={setComposerControl} compact />
          <div className="mt-3 flex justify-end">
            <button type="button" className="inline-flex h-9 items-center gap-2 rounded-md bg-blue-600 px-4 text-sm font-medium text-white hover:bg-blue-700" onClick={() => onStart('chat')}>
              {copy.startChat}
              <ArrowRight size={15} />
            </button>
          </div>
        </div>
      ) : (
        <>
          <div className="grid h-10 grid-cols-4 gap-1 rounded-md bg-gray-100 p-1" role="tablist" aria-label={copy.modeSelectorLabel}>
            {onboardingModes.map((mode) => (
              <button
                key={mode}
                type="button"
                role="tab"
                aria-selected={mode === selectedMode}
                className={`inline-flex min-w-0 items-center justify-center gap-2 rounded px-2 text-sm font-medium transition-colors ${
                  mode === selectedMode
                    ? 'bg-white text-gray-900 shadow-sm'
                    : 'text-gray-500 hover:bg-gray-50 hover:text-gray-900'
                }`}
                onClick={() => setSelectedMode(mode)}
              >
                <span className="shrink-0">{modeIcons[mode]}</span>
                <span className="truncate">{detailLabel(copy, mode)}</span>
              </button>
            ))}
          </div>

          <div className="mt-3">
            <h3 className="text-base font-semibold text-gray-950">{detail.title}</h3>
            <p className="mt-1 text-sm leading-5 text-gray-500">{detail.summary}</p>
          </div>

          <dl className="mt-3 divide-y divide-gray-100 border-y border-gray-100 text-sm">
            <ModeDetail label={copy.labels.bestFor} value={detail.bestFor} />
            <ModeDetail label={copy.labels.behavior} value={detail.behavior} />
            <ModeDetail label={copy.labels.material} value={detail.material} />
            <ModeDetail label={copy.labels.output} value={detail.output} />
          </dl>

          <div className="mt-3 flex items-center gap-4 rounded-md bg-gray-50 px-4 py-2.5">
            <div className="min-w-0 flex-1">
              <div className="text-xs font-medium text-gray-500">{copy.starterLabel}</div>
              <p className="mt-1 line-clamp-2 text-sm leading-5 text-gray-700">{starterPrompt}</p>
              {unavailableReason && (
                <div className="mt-2 flex items-center gap-1.5 text-xs text-amber-700">
                  <LockKeyhole size={13} className="shrink-0" />
                  <span>{unavailableReason}</span>
                </div>
              )}
            </div>
            <button
              type="button"
              className="inline-flex h-9 shrink-0 items-center gap-2 rounded-md bg-blue-600 px-4 text-sm font-medium text-white hover:bg-blue-700 disabled:cursor-not-allowed disabled:bg-gray-200 disabled:text-gray-500"
              disabled={Boolean(unavailableReason)}
              onClick={() => onStart(selectedMode)}
            >
              {detail.start}
              <ArrowRight size={15} />
            </button>
          </div>
        </>
      )}
    </div>
  )
}

function ModeDetail({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[6.5rem_minmax(0,1fr)] gap-4 py-2">
      <dt className="font-medium text-gray-700">{label}</dt>
      <dd className="leading-5 text-gray-500">{value}</dd>
    </div>
  )
}

function detailLabel(copy: typeof chineseCopy, mode: OnboardingMode) {
  return copy.modes[mode].label
}

const chineseCopy = {
  guideSelectorLabel: '开始会话指南',
  guideViews: { composer: '输入框控件', modes: '会话模式' },
  startChat: '进入 Chat 体验',
  modeSelectorLabel: '选择会话模式',
  starterLabel: '可编辑的起步内容',
  labels: {
    bestFor: '适合',
    behavior: '运行方式',
    material: '可用资料',
    output: '产出',
  },
  unavailable: {
    mode: '“{name}”未启用此模式，可返回导师步骤调整或改用临时助手。',
    notebook: '“{name}”没有 Notebook 权限，无法使用整理模式。',
  },
  modes: {
    chat: {
      label: '聊天',
      title: 'Chat · 普通聊天',
      summary: '用自然的多轮交流解释、追问和解决问题，不会因为进入模式而强制启动固定 workflow。',
      bestFor: '概念讲解、问题分析、学习讨论和需要逐步澄清的任务。',
      behavior: '回答保持普通流式显示；Agent 可按需使用检索、搜索或代码工具。',
      material: '可挂载知识库、Notebook 和空间内容，并自然使用可靠的学习记忆。',
      output: '当前会话中的回答和引用，不强制生成独立报告或卡片。',
      start: '使用 Chat 开始',
    },
    research: {
      label: '调研',
      title: 'Research · 深入调研',
      summary: '先像普通聊天一样确认主题、范围和交付形式，再显式启动独立的详细调研 workflow。',
      bestFor: '需要多来源搜索、比较、综合、限制说明和可追溯引用的主题。',
      behavior: '主 Agent 负责需求确认；确认后由研究 workflow 搜索、阅读、整理并生成报告。',
      material: '联网搜索、网页来源，以及用户选择的知识库和 Notebook 材料。',
      output: '带来源的结构化研究报告，可保存到 Notebook，也可将网页来源导入知识库。',
      start: '使用 Research 开始',
    },
    quiz: {
      label: '测验',
      title: 'Quiz · 生成测验',
      summary: '先确认测验主题、范围和材料，再启动测验 workflow，生成可以持续作答的交互卡片。',
      bestFor: '检查理解、复习知识点、根据已有材料出题并获得答案解释。',
      behavior: 'Agent 先形成测验计划；确认后由 workflow 生成题目、答案、解释和引用。',
      material: '主题描述、知识库、Notebook、会话内容或空间中的已有学习材料。',
      output: '可持久恢复的测验卡片、作答记录、得分、解释和来源。',
      start: '使用 Quiz 开始',
    },
    organize: {
      label: '整理',
      title: 'Organize · 整理笔记',
      summary: '围绕 Notebook 进行搜索、阅读和结构维护，所有写入都先形成可审核的修改建议。',
      bestFor: '整理笔记结构、补充链接和标签、去重、合并或提出新笔记建议。',
      behavior: 'Agent 读取相关笔记后提出确定性编辑；用户审核接受后才会应用。',
      material: 'Notebook 及其中明确关联的内容；所选导师必须拥有 Notebook 权限。',
      output: '可逐项审核的编辑、链接、标签、移动、合并或新建笔记建议。',
      start: '使用 Organize 开始',
    },
  },
}

const englishCopy: typeof chineseCopy = {
  guideSelectorLabel: 'Start a conversation guide',
  guideViews: { composer: 'Composer controls', modes: 'Conversation modes' },
  startChat: 'Try it in Chat',
  modeSelectorLabel: 'Choose a conversation mode',
  starterLabel: 'Editable starter prompt',
  labels: {
    bestFor: 'Best for',
    behavior: 'Behavior',
    material: 'Material',
    output: 'Output',
  },
  unavailable: {
    mode: '“{name}” does not enable this mode. Return to Tutor or use Temporary Assistant.',
    notebook: '“{name}” has no Notebook permission, so Organize is unavailable.',
  },
  modes: {
    chat: {
      label: 'Chat',
      title: 'Chat · Normal conversation',
      summary: 'Explain, follow up, and solve problems through natural multi-turn conversation without forcing a fixed workflow.',
      bestFor: 'Concept explanations, problem analysis, learning discussions, and tasks that need clarification.',
      behavior: 'Answers stream normally. The Agent may use retrieval, search, or code tools when useful.',
      material: 'Selected Knowledge Bases, Notebook, Space content, and reliable learning memory.',
      output: 'Conversation answers and citations without requiring a separate report or card.',
      start: 'Start with Chat',
    },
    research: {
      label: 'Research',
      title: 'Research · Detailed investigation',
      summary: 'Clarify topic, scope, and deliverable through normal conversation before explicitly starting the detailed research workflow.',
      bestFor: 'Topics needing multi-source search, comparison, synthesis, limitations, and traceable citations.',
      behavior: 'The main Agent confirms requirements; the workflow then searches, reads, organizes, and writes the report.',
      material: 'Web search and pages plus selected Knowledge Base and Notebook material.',
      output: 'A cited structured report that can be saved to Notebook, with web sources optionally ingested into Knowledge Base.',
      start: 'Start with Research',
    },
    quiz: {
      label: 'Quiz',
      title: 'Quiz · Generate a quiz',
      summary: 'Confirm topic, scope, and material before starting the quiz workflow and creating a durable interactive card.',
      bestFor: 'Checking understanding, reviewing concepts, and generating questions from saved material.',
      behavior: 'The Agent proposes a quiz plan; after confirmation, the workflow generates questions, answers, explanations, and citations.',
      material: 'A topic, Knowledge Base, Notebook, conversation, or existing material in Space.',
      output: 'A restorable quiz card with answers, score, explanations, and sources.',
      start: 'Start with Quiz',
    },
    organize: {
      label: 'Organize',
      title: 'Organize · Maintain notes',
      summary: 'Search, read, and maintain Notebook structure while keeping every write behind a reviewable proposal.',
      bestFor: 'Organizing note structure, links, tags, duplicates, merges, or proposed new notes.',
      behavior: 'The Agent reads relevant notes and proposes deterministic edits that apply only after user review.',
      material: 'Notebook and explicitly related content. The selected Tutor must have Notebook permission.',
      output: 'Reviewable edit, link, tag, move, merge, or new-note proposals.',
      start: 'Start with Organize',
    },
  },
}
