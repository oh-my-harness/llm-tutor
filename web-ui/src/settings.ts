export type LlmProvider = 'anthropic' | 'deepseek' | 'openai'

export interface LlmSettings {
  provider: LlmProvider
  model: string
  apiKey: string
  baseUrl: string
  chatPath: string
  budgetLimitUsd: number
  requireApproval: boolean
}

export const defaultLlmSettings: LlmSettings = {
  provider: 'deepseek',
  model: 'deepseek-v4-flash',
  apiKey: '',
  baseUrl: 'https://api.deepseek.com',
  chatPath: '/chat/completions',
  budgetLimitUsd: 2,
  requireApproval: false,
}

export function loadLlmSettings(): LlmSettings {
  const raw = localStorage.getItem('tutor.llmSettings')
  if (!raw) return defaultLlmSettings

  try {
    const parsed = JSON.parse(raw) as Partial<LlmSettings>
    return {
      ...defaultLlmSettings,
      ...parsed,
      budgetLimitUsd: Number(parsed.budgetLimitUsd ?? defaultLlmSettings.budgetLimitUsd),
      requireApproval: Boolean(parsed.requireApproval),
    }
  } catch {
    return defaultLlmSettings
  }
}

export function saveLlmSettings(settings: LlmSettings) {
  localStorage.setItem('tutor.llmSettings', JSON.stringify(settings))
}

export function settingsForSession(settings: LlmSettings) {
  return {
    provider: settings.provider,
    model: settings.model.trim(),
    api_key: settings.apiKey.trim(),
    base_url: settings.baseUrl.trim() || null,
    chat_path: settings.chatPath.trim() || null,
    budget_limit_usd: settings.budgetLimitUsd,
    require_approval: settings.requireApproval,
  }
}
