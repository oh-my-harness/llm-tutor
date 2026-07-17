import type { UiLanguage } from './i18n'
import type { TutorProfile } from './tutorTypes'

export type OnboardingMode = 'chat' | 'research' | 'quiz' | 'organize'
export type OnboardingModeBlock = 'capability' | 'notebook' | null

export const onboardingModes: OnboardingMode[] = ['chat', 'research', 'quiz', 'organize']

export function onboardingModeBlock(mode: OnboardingMode, tutor: TutorProfile | null): OnboardingModeBlock {
  if (!tutor) return null
  if (!tutor.allowed_capabilities.includes(mode)) return 'capability'
  if (mode === 'organize' && !tutor.resource_permissions.notebook) return 'notebook'
  return null
}

export function initialOnboardingMode(tutor: TutorProfile | null): OnboardingMode {
  const preferred = isOnboardingMode(tutor?.default_capability) ? tutor.default_capability : 'chat'
  if (!onboardingModeBlock(preferred, tutor)) return preferred
  return onboardingModes.find((mode) => !onboardingModeBlock(mode, tutor)) ?? 'chat'
}

export function onboardingStarterPrompt(mode: OnboardingMode, language: UiLanguage) {
  const prompts: Record<UiLanguage, Record<OnboardingMode, string>> = {
    'zh-CN': {
      chat: '请解释一个我正在学习的概念，先问问我已经了解多少。',
      research: '我想深入调研一个主题，请先帮我确认研究范围和期望产出。',
      quiz: '请为我生成一组简短测验，先询问我要使用的主题或已有材料。',
      organize: '请帮我整理 Notebook，先询问要处理的范围和目标，读取相关笔记后提出可审核的修改建议。',
    },
    'en-US': {
      chat: 'Explain a concept I am learning, starting by asking what I already know.',
      research: 'I want to research a topic in depth. First help me clarify the scope and desired output.',
      quiz: 'Create a short quiz for me. First ask what topic or saved material I want to use.',
      organize: 'Help me organize Notebook. First ask what scope and outcome I want, then read relevant notes and propose reviewable changes.',
    },
  }
  return prompts[language][mode]
}

function isOnboardingMode(value: string | null | undefined): value is OnboardingMode {
  return value === 'chat' || value === 'research' || value === 'quiz' || value === 'organize'
}
