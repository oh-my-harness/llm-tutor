export type LlmProvider = 'anthropic' | 'deepseek' | 'openai'
export type EmbeddingProvider = 'openai'

export interface EmbeddingModelConfig {
  id: string
  name: string
  provider: EmbeddingProvider
  baseUrl: string
  embeddingsPath: string
  apiKey: string
  model: string
  dimensions: number
  sendDimensions: boolean
}

export interface LlmSettings {
  provider: LlmProvider
  model: string
  apiKey: string
  baseUrl: string
  chatPath: string
  budgetLimitUsd: number
  requireApproval: boolean
  embeddingConfigs: EmbeddingModelConfig[]
  activeEmbeddingConfigId: string | null
}

export const defaultLlmSettings: LlmSettings = {
  provider: 'deepseek',
  model: 'deepseek-v4-flash',
  apiKey: '',
  baseUrl: 'https://api.deepseek.com',
  chatPath: '/chat/completions',
  budgetLimitUsd: 2,
  requireApproval: false,
  embeddingConfigs: [],
  activeEmbeddingConfigId: null,
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
      embeddingConfigs: normalizeEmbeddingConfigs(parsed.embeddingConfigs),
      activeEmbeddingConfigId: normalizeActiveEmbeddingConfigId(
        parsed.activeEmbeddingConfigId,
        parsed.embeddingConfigs,
      ),
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

export function activeEmbeddingConfig(settings: LlmSettings): EmbeddingModelConfig | null {
  return settings.embeddingConfigs.find((config) => config.id === settings.activeEmbeddingConfigId) ?? null
}

export function createEmbeddingConfig(): EmbeddingModelConfig {
  return {
    id: crypto.randomUUID(),
    name: 'OpenAI',
    provider: 'openai',
    baseUrl: 'https://api.openai.com',
    embeddingsPath: '/v1/embeddings',
    apiKey: '',
    model: 'text-embedding-3-small',
    dimensions: 1536,
    sendDimensions: false,
  }
}

export function embeddingForSession(config: EmbeddingModelConfig) {
  return {
    provider: config.provider,
    model: config.model.trim(),
    api_key: config.apiKey.trim(),
    base_url: config.baseUrl.trim() || null,
    embeddings_path: config.embeddingsPath.trim() || null,
    dimensions: Number(config.dimensions || 0) || null,
    send_dimensions: config.sendDimensions,
  }
}

function normalizeEmbeddingConfigs(value: unknown): EmbeddingModelConfig[] {
  if (!Array.isArray(value)) return []

  return value.map((item) => {
    const config = item as Partial<EmbeddingModelConfig>
    return {
      id: typeof config.id === 'string' && config.id ? config.id : crypto.randomUUID(),
      name: typeof config.name === 'string' && config.name ? config.name : 'OpenAI',
      provider: 'openai',
      baseUrl: typeof config.baseUrl === 'string' ? config.baseUrl : 'https://api.openai.com',
      embeddingsPath: typeof config.embeddingsPath === 'string' ? config.embeddingsPath : '/v1/embeddings',
      apiKey: typeof config.apiKey === 'string' ? config.apiKey : '',
      model: typeof config.model === 'string' ? config.model : 'text-embedding-3-small',
      dimensions: Number(config.dimensions || 1536),
      sendDimensions: Boolean(config.sendDimensions),
    }
  })
}

function normalizeActiveEmbeddingConfigId(value: unknown, configs: unknown): string | null {
  if (typeof value !== 'string') return null
  const normalized = normalizeEmbeddingConfigs(configs)
  return normalized.some((config) => config.id === value) ? value : null
}
