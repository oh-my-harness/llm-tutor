export type LlmProvider = 'anthropic' | 'openai'
export type EmbeddingProvider = 'openai'
export type SearchProvider = 'duckduckgo'

export interface LlmModelConfig {
  id: string
  name: string
  provider: LlmProvider
  model: string
  apiKey: string
  baseUrl: string
  chatPath: string
}

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

export interface SearchConfig {
  id: string
  name: string
  provider: SearchProvider
  baseUrl: string
  apiKey: string
  maxResults: number
}

export interface LlmSettings {
  provider: LlmProvider
  model: string
  apiKey: string
  baseUrl: string
  chatPath: string
  budgetLimitUsd: number
  requireApproval: boolean
  llmConfigs: LlmModelConfig[]
  activeLlmConfigId: string | null
  embeddingConfigs: EmbeddingModelConfig[]
  activeEmbeddingConfigId: string | null
  searchConfigs: SearchConfig[]
  activeSearchConfigId: string | null
}

export const defaultLlmSettings: LlmSettings = {
  provider: 'openai',
  model: 'deepseek-v4-flash',
  apiKey: '',
  baseUrl: 'https://api.deepseek.com',
  chatPath: '/chat/completions',
  budgetLimitUsd: 2,
  requireApproval: false,
  llmConfigs: [],
  activeLlmConfigId: null,
  embeddingConfigs: [],
  activeEmbeddingConfigId: null,
  searchConfigs: [],
  activeSearchConfigId: null,
}

export function loadLlmSettings(): LlmSettings {
  const raw = localStorage.getItem('tutor.llmSettings')
  if (!raw) return defaultLlmSettings

  try {
    const parsed = JSON.parse(raw) as Partial<LlmSettings>
    const llmConfigs = normalizeLlmConfigs(parsed.llmConfigs, parsed)
    const activeLlmConfigId = normalizeActiveConfigId(parsed.activeLlmConfigId, llmConfigs)
    const activeLlmConfig = llmConfigs.find((config) => config.id === activeLlmConfigId) ?? null
    const embeddingConfigs = normalizeEmbeddingConfigs(parsed.embeddingConfigs)
    const searchConfigs = normalizeSearchConfigs(parsed.searchConfigs)

    return {
      ...defaultLlmSettings,
      ...parsed,
      ...(activeLlmConfig ? llmConfigToLegacyFields(activeLlmConfig) : {}),
      provider: activeLlmConfig
        ? activeLlmConfig.provider
        : normalizeLlmProvider((parsed as { provider?: unknown }).provider),
      budgetLimitUsd: Number(parsed.budgetLimitUsd ?? defaultLlmSettings.budgetLimitUsd),
      requireApproval: Boolean(parsed.requireApproval),
      llmConfigs,
      activeLlmConfigId,
      embeddingConfigs,
      activeEmbeddingConfigId: normalizeActiveConfigId(parsed.activeEmbeddingConfigId, embeddingConfigs),
      searchConfigs,
      activeSearchConfigId: normalizeActiveConfigId(parsed.activeSearchConfigId, searchConfigs),
    }
  } catch {
    return defaultLlmSettings
  }
}

export function saveLlmSettings(settings: LlmSettings) {
  localStorage.setItem('tutor.llmSettings', JSON.stringify(settings))
}

export function settingsForSession(settings: LlmSettings) {
  const config = activeLlmConfig(settings)
  return {
    provider: config?.provider ?? settings.provider,
    model: (config?.model ?? settings.model).trim(),
    api_key: (config?.apiKey ?? settings.apiKey).trim(),
    base_url: (config?.baseUrl ?? settings.baseUrl).trim() || null,
    chat_path: (config?.chatPath ?? settings.chatPath).trim() || null,
    budget_limit_usd: settings.budgetLimitUsd,
    require_approval: settings.requireApproval,
  }
}

export function searchForSession(settings: LlmSettings) {
  const config = activeSearchConfig(settings)
  if (!config) return null
  return {
    provider: config.provider,
    base_url: config.baseUrl.trim(),
    api_key: config.apiKey.trim() || null,
    max_results: Number(config.maxResults || 5),
  }
}

export function activeLlmConfig(settings: LlmSettings): LlmModelConfig | null {
  return settings.llmConfigs.find((config) => config.id === settings.activeLlmConfigId) ?? null
}

export function createLlmConfig(provider: LlmProvider = 'openai'): LlmModelConfig {
  const preset = llmProviderPreset(provider)
  return {
    id: crypto.randomUUID(),
    name: preset.label,
    provider,
    model: preset.model,
    apiKey: '',
    baseUrl: preset.baseUrl,
    chatPath: preset.chatPath,
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

export function activeSearchConfig(settings: LlmSettings): SearchConfig | null {
  return settings.searchConfigs.find((config) => config.id === settings.activeSearchConfigId) ?? null
}

export function createSearchConfig(): SearchConfig {
  return {
    id: crypto.randomUUID(),
    name: 'DuckDuckGo',
    provider: 'duckduckgo',
    baseUrl: 'https://api.duckduckgo.com/',
    apiKey: '',
    maxResults: 5,
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

export function llmProviderPreset(provider: LlmProvider) {
  if (provider === 'anthropic') {
    return {
      label: 'Anthropic',
      model: 'claude-haiku-4-5-20251001',
      baseUrl: 'https://api.anthropic.com',
      chatPath: '',
    }
  }

  return {
    label: 'OpenAI-compatible',
    model: 'gpt-4o-mini',
    baseUrl: 'https://api.openai.com',
    chatPath: '/v1/chat/completions',
  }
}

function normalizeLlmConfigs(value: unknown, legacy: Partial<LlmSettings>): LlmModelConfig[] {
  if (Array.isArray(value)) {
    return value.map((item) => normalizeLlmConfig(item))
  }

  if (legacy.model || legacy.apiKey || legacy.provider || legacy.baseUrl || legacy.chatPath) {
    const legacyProvider = legacy.provider as unknown
    const provider = normalizeLlmProvider(legacyProvider)
    const legacyPresetLabel =
      legacyProvider === 'deepseek' ? 'DeepSeek' : llmProviderPreset(provider).label

    return [
      {
        id: crypto.randomUUID(),
        name: legacyPresetLabel,
        provider,
        model: typeof legacy.model === 'string' ? legacy.model : defaultLlmSettings.model,
        apiKey: typeof legacy.apiKey === 'string' ? legacy.apiKey : '',
        baseUrl: typeof legacy.baseUrl === 'string' ? legacy.baseUrl : defaultLlmSettings.baseUrl,
        chatPath: typeof legacy.chatPath === 'string' ? legacy.chatPath : defaultLlmSettings.chatPath,
      },
    ]
  }

  return []
}

function normalizeLlmConfig(value: unknown): LlmModelConfig {
  const config = value as Partial<LlmModelConfig>
  const provider = normalizeLlmProvider(config.provider)
  const preset = llmProviderPreset(provider)
  return {
    id: typeof config.id === 'string' && config.id ? config.id : crypto.randomUUID(),
    name: typeof config.name === 'string' && config.name ? config.name : preset.label,
    provider,
    model: typeof config.model === 'string' ? config.model : preset.model,
    apiKey: typeof config.apiKey === 'string' ? config.apiKey : '',
    baseUrl: typeof config.baseUrl === 'string' ? config.baseUrl : preset.baseUrl,
    chatPath: typeof config.chatPath === 'string' ? config.chatPath : preset.chatPath,
  }
}

function normalizeLlmProvider(value: unknown): LlmProvider {
  if (value === 'anthropic') return 'anthropic'
  return 'openai'
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

function normalizeSearchConfigs(value: unknown): SearchConfig[] {
  if (!Array.isArray(value)) return []

  return value.map((item) => {
    const config = item as Partial<SearchConfig>
    return {
      id: typeof config.id === 'string' && config.id ? config.id : crypto.randomUUID(),
      name: typeof config.name === 'string' && config.name ? config.name : 'DuckDuckGo',
      provider: 'duckduckgo',
      baseUrl: typeof config.baseUrl === 'string' ? config.baseUrl : 'https://api.duckduckgo.com/',
      apiKey: typeof config.apiKey === 'string' ? config.apiKey : '',
      maxResults: Number(config.maxResults || 5),
    }
  })
}

function normalizeActiveConfigId<T extends { id: string }>(value: unknown, configs: T[]): string | null {
  if (typeof value !== 'string') return configs[0]?.id ?? null
  return configs.some((config) => config.id === value) ? value : configs[0]?.id ?? null
}

function llmConfigToLegacyFields(config: LlmModelConfig) {
  return {
    provider: config.provider,
    model: config.model,
    apiKey: config.apiKey,
    baseUrl: config.baseUrl,
    chatPath: config.chatPath,
  }
}
