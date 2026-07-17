import { useEffect, useMemo, useRef, useState, type ReactNode } from 'react'
import {
  ArrowLeft,
  ArrowRight,
  Bot,
  Check,
  CircleCheck,
  FileQuestion,
  MessageSquareText,
  NotebookPen,
  Search,
  Settings2,
  Sparkles,
  X,
} from 'lucide-react'
import { activeLlmConfig, hasUsableLlmConfig, testLlmConnection, type LlmSettings } from '../settings'
import type { TutorProfile } from '../tutorTypes'
import { useI18n } from '../i18n'
import { TutorChooser } from './TutorChooser'

export type OnboardingTask = 'chat' | 'research' | 'notebook' | 'quiz'

interface Props {
  settings: LlmSettings
  tutors: TutorProfile[]
  selectedTutorId: string | null
  step: number
  onStepChange: (step: number) => void
  onTutorSelect: (tutorId: string | null) => void
  onOpenModelSettings: () => void
  onManageTutors: () => void
  onDismiss: () => void
  onComplete: () => void
  onStartTask: (task: OnboardingTask) => void
}

type TestState = { status: 'idle' | 'running' | 'ok' | 'error'; message: string }

export function OnboardingDialog({
  settings,
  tutors,
  selectedTutorId,
  step,
  onStepChange,
  onTutorSelect,
  onOpenModelSettings,
  onManageTutors,
  onDismiss,
  onComplete,
  onStartTask,
}: Props) {
  const { language } = useI18n()
  const copy = language === 'en-US' ? englishCopy : chineseCopy
  const [testState, setTestState] = useState<TestState>({ status: 'idle', message: '' })
  const dialogRef = useRef<HTMLElement>(null)
  const activeModel = activeLlmConfig(settings)
  const modelReady = hasUsableLlmConfig(settings)
  const selectedTutor = useMemo(
    () => selectedTutorId ? tutors.find((tutor) => tutor.id === selectedTutorId) ?? null : null,
    [selectedTutorId, tutors],
  )

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onDismiss()
      if (event.key !== 'Tab') return
      const controls = [...(dialogRef.current?.querySelectorAll<HTMLElement>(
        'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
      ) ?? [])]
      if (controls.length === 0) return
      const first = controls[0]
      const last = controls[controls.length - 1]
      if (!first || !last) return
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && (document.activeElement === last || !dialogRef.current?.contains(document.activeElement))) {
        event.preventDefault()
        first.focus()
      }
    }
    window.addEventListener('keydown', handleKeyDown)
    dialogRef.current?.querySelector<HTMLElement>('button:not([disabled])')?.focus()
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [onDismiss])

  const testModel = async () => {
    if (!activeModel || !modelReady) return
    setTestState({ status: 'running', message: copy.model.testing })
    try {
      const result = await testLlmConnection(activeModel)
      setTestState({
        status: 'ok',
        message: typeof result.message === 'string' ? result.message : copy.model.testOk,
      })
    } catch (error) {
      setTestState({
        status: 'error',
        message: error instanceof Error ? error.message : copy.model.testError,
      })
    }
  }

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center bg-black/30 px-5 py-6 backdrop-blur-[2px]" role="presentation">
      <section
        ref={dialogRef}
        className="flex max-h-[min(720px,calc(100vh-48px))] w-full max-w-3xl overflow-hidden rounded-lg border border-gray-200 bg-white shadow-2xl"
        role="dialog"
        aria-modal="true"
        aria-labelledby="onboarding-title"
      >
        <aside className="w-52 shrink-0 border-r border-gray-200 bg-gray-50 px-5 py-6">
          <div className="mb-8 flex items-center gap-2 text-gray-900">
            <Sparkles size={20} className="text-blue-600" />
            <span className="text-sm font-semibold">Tutor Agent</span>
          </div>
          <ol className="space-y-1">
            {copy.steps.map((label, index) => (
              <li
                key={label}
                aria-current={index === step ? 'step' : undefined}
                className={`flex min-h-10 items-center gap-3 rounded-md px-3 text-sm ${
                  index === step ? 'bg-white font-medium text-gray-900 shadow-sm' : 'text-gray-500'
                }`}
              >
                <span className={`flex h-5 w-5 items-center justify-center rounded-full text-[11px] ${
                  index < step
                    ? 'bg-blue-600 text-white'
                    : index === step
                      ? 'border border-blue-600 text-blue-700'
                      : 'border border-gray-300 text-gray-500'
                }`}>
                  {index < step ? <Check size={12} /> : index + 1}
                </span>
                {label}
              </li>
            ))}
          </ol>
        </aside>

        <div className="flex min-h-[520px] min-w-0 flex-1 flex-col">
          <header className="flex items-start justify-between border-b border-gray-100 px-8 py-6">
            <div>
              <h1 id="onboarding-title" className="text-xl font-semibold text-gray-950">{copy.title}</h1>
              <p className="mt-1 text-sm text-gray-500">{copy.subtitle}</p>
            </div>
            <button
              type="button"
              className="inline-flex h-9 w-9 items-center justify-center rounded-md text-gray-500 hover:bg-gray-100 hover:text-gray-900 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
              title={copy.dismiss}
              aria-label={copy.dismiss}
              onClick={onDismiss}
            >
              <X size={18} />
            </button>
          </header>

          <div className="min-h-0 flex-1 overflow-y-auto px-8 py-7">
            {step === 0 && (
              <div>
                <StepHeading icon={<Settings2 size={21} />} title={copy.model.title} description={copy.model.description} />
                <div className="mt-7 rounded-md border border-gray-200 bg-gray-50 px-5 py-4">
                  <div className="flex items-center gap-3">
                    <span className={`flex h-9 w-9 items-center justify-center rounded-md ${modelReady ? 'bg-emerald-50 text-emerald-700' : 'bg-amber-50 text-amber-700'}`}>
                      {modelReady ? <CircleCheck size={20} /> : <Settings2 size={20} />}
                    </span>
                    <div className="min-w-0 flex-1">
                      <div className="truncate text-sm font-semibold text-gray-900">
                        {modelReady ? activeModel?.name : copy.model.missing}
                      </div>
                      <div className="mt-0.5 truncate text-xs text-gray-500">
                        {modelReady ? activeModel?.model : copy.model.missingDescription}
                      </div>
                    </div>
                    <button type="button" className="inline-flex h-9 items-center gap-2 rounded-md border border-gray-300 bg-white px-3 text-sm font-medium text-gray-700 hover:bg-gray-100" onClick={onOpenModelSettings}>
                      <Settings2 size={15} />
                      {modelReady ? copy.model.manage : copy.model.configure}
                    </button>
                  </div>
                </div>
                {modelReady && (
                  <div className="mt-4 flex items-center gap-3">
                    <button
                      type="button"
                      className="inline-flex h-9 items-center rounded-md border border-gray-300 bg-white px-3 text-sm font-medium text-gray-700 hover:bg-gray-100 disabled:opacity-50"
                      disabled={testState.status === 'running'}
                      onClick={() => void testModel()}
                    >
                      {testState.status === 'running' ? copy.model.testing : copy.model.test}
                    </button>
                    {testState.message && (
                      <span className={`text-xs ${testState.status === 'error' ? 'text-red-600' : testState.status === 'ok' ? 'text-emerald-700' : 'text-gray-500'}`}>
                        {testState.message}
                      </span>
                    )}
                  </div>
                )}
              </div>
            )}

            {step === 1 && (
              <div>
                <StepHeading icon={<Bot size={21} />} title={copy.tutor.title} description={copy.tutor.description} />
                <div className="mt-7">
                  <TutorChooser
                    tutors={tutors}
                    selectedTutorId={selectedTutorId}
                    onSelect={onTutorSelect}
                    onManage={onManageTutors}
                  />
                </div>
                <div className="mt-5 rounded-md bg-gray-50 px-4 py-3 text-sm text-gray-600">
                  {selectedTutor ? copy.tutor.selected.replace('{name}', selectedTutor.name) : copy.tutor.temporary}
                </div>
              </div>
            )}

            {step === 2 && (
              <div>
                <StepHeading icon={<Sparkles size={21} />} title={copy.task.title} description={copy.task.description} />
                <div className="mt-6 grid grid-cols-2 gap-3">
                  <TaskButton icon={<MessageSquareText size={19} />} title={copy.task.chat} description={copy.task.chatDescription} onClick={() => onStartTask('chat')} />
                  <TaskButton icon={<Search size={19} />} title={copy.task.research} description={copy.task.researchDescription} onClick={() => onStartTask('research')} />
                  <TaskButton icon={<NotebookPen size={19} />} title={copy.task.notebook} description={copy.task.notebookDescription} onClick={() => onStartTask('notebook')} />
                  <TaskButton icon={<FileQuestion size={19} />} title={copy.task.quiz} description={copy.task.quizDescription} onClick={() => onStartTask('quiz')} />
                </div>
              </div>
            )}
          </div>

          <footer className="flex items-center border-t border-gray-100 px-8 py-5">
            <button type="button" className="rounded-md px-2 py-2 text-sm text-gray-500 hover:bg-gray-100 hover:text-gray-900" onClick={onDismiss}>{copy.later}</button>
            <div className="ml-auto flex gap-2">
              {step > 0 && (
                <button type="button" className="inline-flex h-9 items-center gap-2 rounded-md border border-gray-300 bg-white px-3 text-sm font-medium text-gray-700 hover:bg-gray-100" onClick={() => onStepChange(step - 1)}>
                  <ArrowLeft size={15} />
                  {copy.back}
                </button>
              )}
              {step < 2 && (
                <button type="button" className="inline-flex h-9 items-center gap-2 rounded-md bg-blue-600 px-4 text-sm font-medium text-white hover:bg-blue-700" onClick={() => onStepChange(step + 1)}>
                  {copy.continue}
                  <ArrowRight size={15} />
                </button>
              )}
              {step === 2 && (
                <button type="button" className="inline-flex h-9 items-center gap-2 rounded-md bg-blue-600 px-4 text-sm font-medium text-white hover:bg-blue-700" onClick={onComplete}>
                  <Check size={15} />
                  {copy.done}
                </button>
              )}
            </div>
          </footer>
        </div>
      </section>
    </div>
  )
}

function StepHeading({ icon, title, description }: { icon: ReactNode; title: string; description: string }) {
  return (
    <div className="flex gap-3">
      <span className="mt-0.5 flex h-9 w-9 shrink-0 items-center justify-center rounded-md bg-blue-50 text-blue-700">{icon}</span>
      <div>
        <h2 className="text-lg font-semibold text-gray-950">{title}</h2>
        <p className="mt-1 max-w-xl text-sm leading-6 text-gray-500">{description}</p>
      </div>
    </div>
  )
}

function TaskButton({ icon, title, description, onClick }: { icon: ReactNode; title: string; description: string; onClick: () => void }) {
  return (
    <button
      type="button"
      className="flex min-h-24 items-start gap-3 rounded-md border border-gray-200 px-4 py-4 text-left transition-colors hover:border-blue-300 hover:bg-blue-50/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-500"
      onClick={onClick}
    >
      <span className="mt-0.5 text-blue-700">{icon}</span>
      <span>
        <span className="block text-sm font-semibold text-gray-900">{title}</span>
        <span className="mt-1 block text-xs leading-5 text-gray-500">{description}</span>
      </span>
    </button>
  )
}

const chineseCopy = {
  title: '开始使用',
  subtitle: '完成第一次真实学习任务',
  steps: ['模型准备', '选择导师', '开始任务'],
  dismiss: '关闭使用引导',
  later: '稍后再说',
  back: '上一步',
  continue: '继续',
  done: '完成',
  model: {
    title: '确认对话模型',
    description: 'Agent 需要一个可用的模型服务。已有配置会直接复用，不会要求你重新填写凭据。',
    missing: '尚未配置模型',
    missingDescription: '你仍可浏览应用，但开始 Agent 任务前需要完成配置。',
    configure: '配置模型',
    manage: '管理',
    test: '测试连接',
    testing: '正在测试...',
    testOk: '模型连接正常。',
    testError: '模型连接失败。',
  },
  tutor: {
    title: '选择陪伴你的导师',
    description: '导师拥有稳定的 Soul 和独立记忆。也可以暂不选择，以临时助手开始一次性对话。',
    selected: '将使用“{name}”开始新会话。',
    temporary: '未选择导师，将使用临时助手。',
  },
  task: {
    title: '选择第一个任务',
    description: '这些入口会打开真实工作区，你可以先修改预填内容再发送。',
    chat: '解释一个概念',
    chatDescription: '从普通对话开始，逐步追问。',
    research: '深入调研主题',
    researchDescription: '先确认范围，再启动研究 workflow。',
    notebook: '创建一份笔记',
    notebookDescription: '进入 Notebook 整理本地学习材料。',
    quiz: '生成一组测验',
    quizDescription: '根据主题或已有材料检查理解。',
  },
}

const englishCopy: typeof chineseCopy = {
  title: 'Get started',
  subtitle: 'Complete your first real learning task',
  steps: ['Model', 'Tutor', 'First task'],
  dismiss: 'Close onboarding',
  later: 'Maybe later',
  back: 'Back',
  continue: 'Continue',
  done: 'Complete',
  model: {
    title: 'Confirm a chat model',
    description: 'Agent tasks need a working model service. Existing settings are reused without asking for credentials again.',
    missing: 'No model configured',
    missingDescription: 'You can browse the app, but a model is required before starting an Agent task.',
    configure: 'Configure model',
    manage: 'Manage',
    test: 'Test connection',
    testing: 'Testing...',
    testOk: 'Model connection works.',
    testError: 'Model connection failed.',
  },
  tutor: {
    title: 'Choose a tutor',
    description: 'Tutors have a stable Soul and private continuity memory. Skip this step to use Temporary Assistant.',
    selected: 'New conversations will use “{name}”.',
    temporary: 'No tutor selected. Temporary Assistant will be used.',
  },
  task: {
    title: 'Choose your first task',
    description: 'Each option opens the real workspace with an editable starting point.',
    chat: 'Explain a concept',
    chatDescription: 'Start with a normal conversation and follow up naturally.',
    research: 'Research a topic',
    researchDescription: 'Confirm scope before starting the research workflow.',
    notebook: 'Create a note',
    notebookDescription: 'Open Notebook and organize local learning material.',
    quiz: 'Generate a quiz',
    quizDescription: 'Check understanding from a topic or saved material.',
  },
}
