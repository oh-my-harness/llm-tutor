import { useState, type ReactNode } from 'react'
import {
  Activity,
  Brain,
  Database,
  Globe2,
  Palette,
  Plus,
  SlidersHorizontal,
  Trash2,
  type LucideIcon,
} from 'lucide-react'
import {
  createEmbeddingConfig,
  createLlmConfig,
  createSearchConfig,
  llmProviderPreset,
  searchProviderPreset,
} from '../settings'
import type {
  EmbeddingModelConfig,
  LlmModelConfig,
  LlmProvider,
  LlmSettings,
  SearchConfig,
  SearchProvider,
} from '../settings'

interface Props {
  settings: LlmSettings
  onChange: (settings: LlmSettings) => void
}

const providerOptions: { value: LlmProvider; label: string; description: string }[] = [
  {
    value: 'openai',
    label: 'OpenAI-compatible',
    description: '适用于 OpenAI、DeepSeek、通义、硅基流动等 /chat/completions 接口。',
  },
  {
    value: 'anthropic',
    label: 'Anthropic Messages',
    description: '适用于 Anthropic Messages API。',
  },
]

type SettingsTab = 'appearance' | 'llm' | 'embedding' | 'search' | 'governance'
type ConfigTestState = {
  status: 'running' | 'ok' | 'error'
  message: string
}

const settingsTabs: Array<{ key: SettingsTab; label: string; icon: LucideIcon }> = [
  { key: 'appearance', label: '外观', icon: Palette },
  { key: 'llm', label: 'LLM', icon: Brain },
  { key: 'embedding', label: '嵌入模型', icon: Database },
  { key: 'search', label: 'Search', icon: Globe2 },
  { key: 'governance', label: '能力', icon: SlidersHorizontal },
]

export function SettingsPage({ settings, onChange }: Props) {
  const [activeTab, setActiveTab] = useState<SettingsTab>('llm')
  const [testState, setTestState] = useState<Record<string, ConfigTestState>>({})

  const update = <K extends keyof LlmSettings>(key: K, value: LlmSettings[K]) => {
    onChange({ ...settings, [key]: value })
  }

  const activeLlmConfig =
    settings.llmConfigs.find((config) => config.id === settings.activeLlmConfigId) ?? null
  const activeEmbeddingConfig =
    settings.embeddingConfigs.find((config) => config.id === settings.activeEmbeddingConfigId) ?? null
  const activeSearchConfig =
    settings.searchConfigs.find((config) => config.id === settings.activeSearchConfigId) ?? null

  const addLlmConfig = () => {
    const config = createLlmConfig()
    onChange({
      ...settings,
      ...legacyFieldsFromLlmConfig(config),
      llmConfigs: [...settings.llmConfigs, config],
      activeLlmConfigId: config.id,
    })
  }

  const updateLlmConfig = <K extends keyof LlmModelConfig>(
    id: string,
    key: K,
    value: LlmModelConfig[K],
  ) => {
    const nextConfigs = settings.llmConfigs.map((config) => {
      if (config.id !== id) return config
      if (key !== 'provider') return { ...config, [key]: value }
      const provider = value as LlmProvider
      const preset = llmProviderPreset(provider)
      return {
        ...config,
        provider,
        name: config.name || preset.label,
        model: preset.model,
        baseUrl: preset.baseUrl,
        chatPath: preset.chatPath,
        contextWindowTokens: preset.contextWindowTokens,
      }
    })
    const active = nextConfigs.find((config) => config.id === settings.activeLlmConfigId)
    onChange({
      ...settings,
      ...(active ? legacyFieldsFromLlmConfig(active) : {}),
      llmConfigs: nextConfigs,
    })
  }

  const selectLlmConfig = (id: string) => {
    const config = settings.llmConfigs.find((item) => item.id === id)
    onChange({
      ...settings,
      ...(config ? legacyFieldsFromLlmConfig(config) : {}),
      activeLlmConfigId: id,
    })
  }

  const deleteLlmConfig = (id: string) => {
    const nextConfigs = settings.llmConfigs.filter((config) => config.id !== id)
    const nextActiveId =
      settings.activeLlmConfigId === id ? nextConfigs[0]?.id ?? null : settings.activeLlmConfigId
    const active = nextConfigs.find((config) => config.id === nextActiveId)
    onChange({
      ...settings,
      ...(active ? legacyFieldsFromLlmConfig(active) : {}),
      llmConfigs: nextConfigs,
      activeLlmConfigId: nextActiveId,
    })
  }

  const addEmbeddingConfig = () => {
    const config = createEmbeddingConfig()
    onChange({
      ...settings,
      embeddingConfigs: [...settings.embeddingConfigs, config],
      activeEmbeddingConfigId: config.id,
    })
  }

  const updateEmbeddingConfig = <K extends keyof EmbeddingModelConfig>(
    id: string,
    key: K,
    value: EmbeddingModelConfig[K],
  ) => {
    onChange({
      ...settings,
      embeddingConfigs: settings.embeddingConfigs.map((config) =>
        config.id === id ? { ...config, [key]: value } : config,
      ),
    })
  }

  const deleteEmbeddingConfig = (id: string) => {
    const nextConfigs = settings.embeddingConfigs.filter((config) => config.id !== id)
    onChange({
      ...settings,
      embeddingConfigs: nextConfigs,
      activeEmbeddingConfigId:
        settings.activeEmbeddingConfigId === id
          ? nextConfigs[0]?.id ?? null
          : settings.activeEmbeddingConfigId,
    })
  }

  const addSearchConfig = () => {
    const config = createSearchConfig()
    onChange({
      ...settings,
      searchConfigs: [...settings.searchConfigs, config],
      activeSearchConfigId: config.id,
    })
  }

  const updateSearchConfig = <K extends keyof SearchConfig>(
    id: string,
    key: K,
    value: SearchConfig[K],
  ) => {
    onChange({
      ...settings,
      searchConfigs: settings.searchConfigs.map((config) =>
        config.id === id ? { ...config, [key]: value } : config,
      ),
    })
  }

  const updateSearchProvider = (id: string, provider: SearchProvider) => {
    const preset = searchProviderPreset(provider)
    onChange({
      ...settings,
      searchConfigs: settings.searchConfigs.map((config) =>
        config.id === id
          ? {
              ...config,
              provider,
              name: config.name === 'DuckDuckGo' || config.name === 'Bing' ? preset.name : config.name,
              baseUrl: preset.baseUrl,
            }
          : config,
      ),
    })
  }

  const deleteSearchConfig = (id: string) => {
    const nextConfigs = settings.searchConfigs.filter((config) => config.id !== id)
    onChange({
      ...settings,
      searchConfigs: nextConfigs,
      activeSearchConfigId:
        settings.activeSearchConfigId === id
          ? nextConfigs[0]?.id ?? null
          : settings.activeSearchConfigId,
    })
  }

  const setConfigTestState = (id: string, state: ConfigTestState) => {
    setTestState((current) => ({ ...current, [id]: state }))
  }

  const testLlmConfig = async (config: LlmModelConfig) => {
    setConfigTestState(config.id, { status: 'running', message: 'Testing model connection...' })
    try {
      const response = await fetch('/api/settings/test/llm', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          provider: config.provider,
          model: config.model,
          api_key: config.apiKey,
          base_url: config.baseUrl,
          chat_path: config.chatPath,
        }),
      })
      const payload = await response.json().catch(() => ({}))
      if (!response.ok) {
        throw new Error(payload.error || 'Model test failed')
      }
      const confirmedWindow = Number(payload.confirmed_context_window_tokens || 0)
      if (confirmedWindow > 0 && confirmedWindow !== config.contextWindowTokens) {
        updateLlmConfig(config.id, 'contextWindowTokens', confirmedWindow)
      }
      const usage =
        payload.input_tokens || payload.output_tokens
          ? ` Input ${payload.input_tokens ?? 0}, output ${payload.output_tokens ?? 0} tokens.`
          : ''
      setConfigTestState(config.id, {
        status: 'ok',
        message: `${payload.message || 'Model connection works.'}${usage}`,
      })
    } catch (error) {
      setConfigTestState(config.id, {
        status: 'error',
        message: error instanceof Error ? error.message : 'Model test failed',
      })
    }
  }

  const testEmbeddingConfig = async (config: EmbeddingModelConfig) => {
    setConfigTestState(config.id, { status: 'running', message: 'Testing embedding connection...' })
    try {
      const response = await fetch('/api/settings/test/embedding', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          provider: config.provider,
          model: config.model,
          api_key: config.apiKey,
          base_url: config.baseUrl,
          embeddings_path: config.embeddingsPath,
          dimensions: config.dimensions,
          send_dimensions: config.sendDimensions,
        }),
      })
      const payload = await response.json().catch(() => ({}))
      if (!response.ok) {
        throw new Error(payload.error || 'Embedding test failed')
      }
      const dimensions = Number(payload.dimensions || 0)
      if (dimensions > 0 && dimensions !== config.dimensions) {
        updateEmbeddingConfig(config.id, 'dimensions', dimensions)
      }
      const usage = payload.total_tokens ? ` Total ${payload.total_tokens} tokens.` : ''
      setConfigTestState(config.id, {
        status: 'ok',
        message: `${payload.message || 'Embedding connection works.'}${usage}`,
      })
    } catch (error) {
      setConfigTestState(config.id, {
        status: 'error',
        message: error instanceof Error ? error.message : 'Embedding test failed',
      })
    }
  }

  return (
    <main className="flex min-h-0 flex-1 bg-gray-50">
      <aside className="hidden w-64 shrink-0 border-r border-gray-200 bg-white px-4 py-6 md:block">
        <div className="mb-8 px-2">
          <h2 className="text-xl font-semibold text-gray-900">设置</h2>
          <p className="mt-2 text-sm leading-6 text-gray-600">配置模型服务、知识库索引和运行能力。</p>
        </div>

        <nav className="space-y-1">
          {settingsTabs.map((tab) => {
            const Icon = tab.icon
            const active = activeTab === tab.key
            return (
              <button
                key={tab.key}
                type="button"
                className={`flex w-full items-center gap-3 rounded-md px-3 py-2.5 text-left text-sm ${
                  active ? 'bg-gray-900 text-white' : 'text-gray-700 hover:bg-gray-100 hover:text-gray-900'
                }`}
                onClick={() => setActiveTab(tab.key)}
              >
                <Icon size={18} />
                <span>{tab.label}</span>
              </button>
            )
          })}
        </nav>
      </aside>

      <div className="min-w-0 flex-1 overflow-y-auto">
        <div className="mx-auto max-w-5xl px-5 py-6 md:px-8">
          <div className="mb-6 flex flex-wrap items-center gap-3">
            <div className="md:hidden">
              <label className="sr-only" htmlFor="settings-tab">
                设置分类
              </label>
              <select
                id="settings-tab"
                className="rounded-md border border-gray-300 bg-white px-3 py-2 text-sm"
                value={activeTab}
                onChange={(event) => setActiveTab(event.target.value as SettingsTab)}
              >
                {settingsTabs.map((tab) => (
                  <option key={tab.key} value={tab.key}>
                    {tab.label}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <h2 className="text-xl font-semibold text-gray-900">{settingsTabs.find((tab) => tab.key === activeTab)?.label}</h2>
              <p className="mt-1 text-sm text-gray-600">{tabDescription(activeTab)}</p>
            </div>
            <span className="ml-auto text-sm text-gray-500">所有更改已保存</span>
          </div>

          {activeTab === 'appearance' && (
            <SettingsPanel
              icon={Palette}
              title="界面外观"
              description="这些设置会作为后续主题和语言配置的入口。"
            >
              <div className="flex items-center justify-between rounded-lg border border-gray-200 px-4 py-3">
                <div>
                  <div className="text-sm font-medium text-gray-900">界面语言</div>
                  <div className="mt-1 text-sm text-gray-500">当前使用中文界面。</div>
                </div>
                <div className="inline-flex rounded-md border border-gray-300 bg-gray-50 p-1 text-sm">
                  <button type="button" className="px-3 py-1 text-gray-500">
                    English
                  </button>
                  <button type="button" className="rounded bg-white px-3 py-1 text-gray-900 shadow-sm">
                    中文
                  </button>
                </div>
              </div>
            </SettingsPanel>
          )}

          {activeTab === 'llm' && (
            <SettingsPanel icon={Brain} title="LLM" description="新会话会使用当前选中的对话模型服务。">
              {settings.llmConfigs.length === 0 ? (
                <EmptyConfig onAdd={addLlmConfig} label="暂无 LLM 配置" />
              ) : (
                <div className="grid gap-5 lg:grid-cols-[230px_1fr]">
                  <ConfigList
                    items={settings.llmConfigs.map((config) => ({
                      id: config.id,
                      title: config.name || llmProviderPreset(config.provider).label,
                      subtitle: config.model || '未设置模型',
                    }))}
                    activeId={settings.activeLlmConfigId}
                    addLabel="添加配置"
                    onAdd={addLlmConfig}
                    onSelect={selectLlmConfig}
                  />

                  {activeLlmConfig && (
                    <div className="space-y-5 rounded-lg border border-gray-200 p-4">
                      <ConfigHeader
                        title="模型接口"
                        description="选择接口模式，并填写端点、Key 和模型 ID。"
                        onDelete={() => deleteLlmConfig(activeLlmConfig.id)}
                      />
                      <ConfigTestBar
                        state={testState[activeLlmConfig.id]}
                        label="测试配置"
                        onTest={() => testLlmConfig(activeLlmConfig)}
                      />
                      <div className="grid gap-4 md:grid-cols-2">
                        <Field label="配置名称">
                          <TextInput
                            value={activeLlmConfig.name}
                            onChange={(value) => updateLlmConfig(activeLlmConfig.id, 'name', value)}
                          />
                        </Field>

                        <Field label="接口模式">
                          <select
                            className={inputClassName}
                            value={activeLlmConfig.provider}
                            onChange={(event) =>
                              updateLlmConfig(activeLlmConfig.id, 'provider', event.target.value as LlmProvider)
                            }
                          >
                            {providerOptions.map((option) => (
                              <option key={option.value} value={option.value}>
                                {option.label}
                              </option>
                            ))}
                          </select>
                          <p className="mt-1 text-xs text-gray-500">
                            {providerOptions.find((option) => option.value === activeLlmConfig.provider)?.description}
                          </p>
                        </Field>

                        <Field label="模型 ID">
                          <TextInput
                            value={activeLlmConfig.model}
                            onChange={(value) => updateLlmConfig(activeLlmConfig.id, 'model', value)}
                          />
                        </Field>

                        <Field label="API Key">
                          <TextInput
                            type="password"
                            value={activeLlmConfig.apiKey}
                            placeholder="sk-..."
                            onChange={(value) => updateLlmConfig(activeLlmConfig.id, 'apiKey', value)}
                          />
                        </Field>

                        <Field label="Base URL">
                          <TextInput
                            value={activeLlmConfig.baseUrl}
                            onChange={(value) => updateLlmConfig(activeLlmConfig.id, 'baseUrl', value)}
                          />
                        </Field>

                        <Field label="Chat path">
                          <TextInput
                            value={activeLlmConfig.chatPath}
                            placeholder="/v1/chat/completions"
                            onChange={(value) => updateLlmConfig(activeLlmConfig.id, 'chatPath', value)}
                          />
                        </Field>

                        <Field label="上下文窗口 tokens">
                          <TextInput
                            type="number"
                            min="1024"
                            step="1024"
                            value={String(activeLlmConfig.contextWindowTokens)}
                            onChange={(value) =>
                              updateLlmConfig(activeLlmConfig.id, 'contextWindowTokens', Number(value))
                            }
                          />
                        </Field>
                      </div>
                    </div>
                  )}
                </div>
              )}
            </SettingsPanel>
          )}

          {activeTab === 'embedding' && (
            <SettingsPanel icon={Database} title="嵌入模型" description="知识库入库和检索会使用这里的向量模型配置。">
              {settings.embeddingConfigs.length === 0 ? (
                <EmptyConfig onAdd={addEmbeddingConfig} label="暂无嵌入模型配置" />
              ) : (
                <div className="grid gap-5 lg:grid-cols-[230px_1fr]">
                  <ConfigList
                    items={settings.embeddingConfigs.map((config) => ({
                      id: config.id,
                      title: config.name || 'OpenAI',
                      subtitle: config.model || '未设置模型',
                    }))}
                    activeId={settings.activeEmbeddingConfigId}
                    addLabel="添加配置"
                    onAdd={addEmbeddingConfig}
                    onSelect={(id) => update('activeEmbeddingConfigId', id)}
                  />

                  {activeEmbeddingConfig && (
                    <div className="space-y-5 rounded-lg border border-gray-200 p-4">
                      <ConfigHeader
                        title="嵌入接口"
                        description="配置 OpenAI-compatible embeddings 接口。"
                        onDelete={() => deleteEmbeddingConfig(activeEmbeddingConfig.id)}
                      />
                      <ConfigTestBar
                        state={testState[activeEmbeddingConfig.id]}
                        label="测试配置"
                        onTest={() => testEmbeddingConfig(activeEmbeddingConfig)}
                      />
                      <div className="grid gap-4 md:grid-cols-2">
                        <Field label="配置名称">
                          <TextInput
                            value={activeEmbeddingConfig.name}
                            onChange={(value) =>
                              updateEmbeddingConfig(activeEmbeddingConfig.id, 'name', value)
                            }
                          />
                        </Field>

                        <Field label="接口模式">
                          <select
                            className={inputClassName}
                            value={activeEmbeddingConfig.provider}
                            onChange={() => updateEmbeddingConfig(activeEmbeddingConfig.id, 'provider', 'openai')}
                          >
                            <option value="openai">OpenAI-compatible</option>
                          </select>
                        </Field>

                        <Field label="Base URL">
                          <TextInput
                            value={activeEmbeddingConfig.baseUrl}
                            placeholder="https://api.openai.com"
                            onChange={(value) =>
                              updateEmbeddingConfig(activeEmbeddingConfig.id, 'baseUrl', value)
                            }
                          />
                        </Field>

                        <Field label="Embeddings path">
                          <TextInput
                            value={activeEmbeddingConfig.embeddingsPath}
                            placeholder="/v1/embeddings"
                            onChange={(value) =>
                              updateEmbeddingConfig(activeEmbeddingConfig.id, 'embeddingsPath', value)
                            }
                          />
                        </Field>

                        <Field label="API Key">
                          <TextInput
                            type="password"
                            value={activeEmbeddingConfig.apiKey}
                            placeholder="sk-..."
                            onChange={(value) =>
                              updateEmbeddingConfig(activeEmbeddingConfig.id, 'apiKey', value)
                            }
                          />
                        </Field>

                        <Field label="模型 ID">
                          <TextInput
                            value={activeEmbeddingConfig.model}
                            placeholder="text-embedding-3-small"
                            onChange={(value) =>
                              updateEmbeddingConfig(activeEmbeddingConfig.id, 'model', value)
                            }
                          />
                        </Field>

                        <Field label="维度">
                          <TextInput
                            type="number"
                            min="1"
                            value={String(activeEmbeddingConfig.dimensions)}
                            onChange={(value) =>
                              updateEmbeddingConfig(activeEmbeddingConfig.id, 'dimensions', Number(value))
                            }
                          />
                        </Field>

                        <label className="flex items-center gap-3 self-end py-2 text-sm text-gray-800">
                          <input
                            className="h-4 w-4"
                            type="checkbox"
                            checked={activeEmbeddingConfig.sendDimensions}
                            onChange={(event) =>
                              updateEmbeddingConfig(
                                activeEmbeddingConfig.id,
                                'sendDimensions',
                                event.target.checked,
                              )
                            }
                          />
                          发送 dimensions 参数
                        </label>
                      </div>
                    </div>
                  )}
                </div>
              )}
            </SettingsPanel>
          )}

          {activeTab === 'search' && (
            <SettingsPanel icon={Globe2} title="Search" description="Configure web search used by agent tools.">
              {settings.searchConfigs.length === 0 ? (
                <EmptyConfig onAdd={addSearchConfig} label="No search config" />
              ) : (
                <div className="grid gap-5 lg:grid-cols-[230px_1fr]">
                  <ConfigList
                    items={settings.searchConfigs.map((config) => ({
                      id: config.id,
                      title: config.name || 'DuckDuckGo',
                      subtitle: `${config.provider} · ${config.maxResults} results`,
                    }))}
                    activeId={settings.activeSearchConfigId}
                    addLabel="Add config"
                    onAdd={addSearchConfig}
                    onSelect={(id) => update('activeSearchConfigId', id)}
                  />

                  {activeSearchConfig && (
                    <div className="space-y-5 rounded-lg border border-gray-200 p-4">
                      <ConfigHeader
                        title="Web search provider"
                        description="Use free HTML providers for fallback, or configure paid search APIs for more reliable agent research."
                        onDelete={() => deleteSearchConfig(activeSearchConfig.id)}
                      />
                      <div className="grid gap-4 md:grid-cols-2">
                        <Field label="Config name">
                          <TextInput
                            value={activeSearchConfig.name}
                            onChange={(value) =>
                              updateSearchConfig(activeSearchConfig.id, 'name', value)
                            }
                          />
                        </Field>

                        <Field label="Provider">
                          <select
                            className={inputClassName}
                            value={activeSearchConfig.provider}
                            onChange={(event) =>
                              updateSearchProvider(
                                activeSearchConfig.id,
                                event.target.value as SearchProvider,
                              )
                            }
                          >
                            <option value="duckduckgo">DuckDuckGo</option>
                            <option value="bing">Bing</option>
                            <option value="brave">Brave Search API</option>
                            <option value="tavily">Tavily</option>
                            <option value="serper">Serper</option>
                            <option value="serpapi">SerpAPI</option>
                            <option value="exa">Exa</option>
                          </select>
                        </Field>

                        <Field label="Base URL">
                          <TextInput
                            value={activeSearchConfig.baseUrl}
                            placeholder={searchProviderPreset(activeSearchConfig.provider).baseUrl}
                            onChange={(value) =>
                              updateSearchConfig(activeSearchConfig.id, 'baseUrl', value)
                            }
                          />
                        </Field>

                        <Field label="API Key">
                          <TextInput
                            type="password"
                            value={activeSearchConfig.apiKey}
                            placeholder={
                              activeSearchConfig.provider === 'duckduckgo' ||
                              activeSearchConfig.provider === 'bing'
                                ? 'optional'
                                : 'required'
                            }
                            onChange={(value) =>
                              updateSearchConfig(activeSearchConfig.id, 'apiKey', value)
                            }
                          />
                        </Field>

                        <Field label="Max results">
                          <TextInput
                            type="number"
                            min="1"
                            max="10"
                            value={String(activeSearchConfig.maxResults)}
                            onChange={(value) =>
                              updateSearchConfig(activeSearchConfig.id, 'maxResults', Number(value))
                            }
                          />
                        </Field>

                        <Field label="Fetch timeout seconds">
                          <TextInput
                            type="number"
                            min="3"
                            max="60"
                            value={String(activeSearchConfig.fetchTimeoutSecs)}
                            onChange={(value) =>
                              updateSearchConfig(activeSearchConfig.id, 'fetchTimeoutSecs', Number(value))
                            }
                          />
                        </Field>

                        <Field label="Max fetched characters">
                          <TextInput
                            type="number"
                            min="1000"
                            max="60000"
                            step="1000"
                            value={String(activeSearchConfig.maxFetchChars)}
                            onChange={(value) =>
                              updateSearchConfig(activeSearchConfig.id, 'maxFetchChars', Number(value))
                            }
                          />
                        </Field>
                      </div>
                    </div>
                  )}
                </div>
              )}
            </SettingsPanel>
          )}

          {activeTab === 'governance' && (
            <SettingsPanel icon={Activity} title="能力" description="预算和工具执行审批会影响新建会话。">
              <Field label="Session budget">
                <div className="flex items-center gap-3">
                  <TextInput
                    className="w-36"
                    min="0"
                    step="0.25"
                    type="number"
                    value={String(settings.budgetLimitUsd)}
                    onChange={(value) => update('budgetLimitUsd', Number(value))}
                  />
                  <span className="text-sm text-gray-600">USD</span>
                </div>
              </Field>

              <label className="flex items-center gap-3 text-sm text-gray-800">
                <input
                  className="h-4 w-4"
                  type="checkbox"
                  checked={settings.requireApproval}
                  onChange={(event) => update('requireApproval', event.target.checked)}
                />
                Require approval before tool execution
              </label>
            </SettingsPanel>
          )}
        </div>
      </div>
    </main>
  )
}

function tabDescription(tab: SettingsTab) {
  if (tab === 'appearance') return '调整界面语言和视觉偏好。'
  if (tab === 'llm') return '配置对话模型服务，可新增多个服务配置。'
  if (tab === 'embedding') return '配置知识库检索使用的嵌入模型。'
  if (tab === 'search') return '配置 agent web_search 工具使用的搜索服务。'
  return '配置预算和工具执行策略。'
}

function EmptyConfig({ label, onAdd }: { label: string; onAdd: () => void }) {
  return (
    <div className="flex min-h-40 flex-col items-center justify-center rounded-lg border border-dashed border-gray-300 bg-gray-50 px-4 py-8 text-center">
      <p className="text-sm text-gray-500">{label}</p>
      <button
        type="button"
        className="mt-4 inline-flex items-center gap-2 rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-800 hover:bg-gray-50"
        onClick={onAdd}
      >
        <Plus size={16} />
        添加配置
      </button>
    </div>
  )
}

function ConfigList({
  items,
  activeId,
  addLabel,
  onAdd,
  onSelect,
}: {
  items: Array<{ id: string; title: string; subtitle: string }>
  activeId: string | null
  addLabel: string
  onAdd: () => void
  onSelect: (id: string) => void
}) {
  return (
    <div className="space-y-2">
      {items.map((item) => (
        <button
          key={item.id}
          type="button"
          className={`w-full rounded-lg border px-4 py-3 text-left ${
            item.id === activeId
              ? 'border-gray-900 bg-gray-50'
              : 'border-gray-200 bg-white hover:bg-gray-50'
          }`}
          onClick={() => onSelect(item.id)}
        >
          <div className="text-sm font-semibold text-gray-900">{item.title}</div>
          <div className="mt-1 truncate text-xs text-gray-500">{item.subtitle}</div>
        </button>
      ))}
      <button
        type="button"
        className="inline-flex w-full items-center justify-center gap-2 rounded-md border border-gray-300 bg-white px-3 py-2 text-sm text-gray-800 hover:bg-gray-50"
        onClick={onAdd}
      >
        <Plus size={16} />
        {addLabel}
      </button>
    </div>
  )
}

function ConfigHeader({
  title,
  description = '配置会保存到本地浏览器，并用于新建会话。',
  onDelete,
}: {
  title: string
  description?: string
  onDelete: () => void
}) {
  return (
    <div className="flex items-center gap-3">
      <div>
        <h4 className="text-sm font-semibold text-gray-900">{title}</h4>
        <p className="mt-1 text-xs text-gray-500">{description}</p>
      </div>
      <button
        type="button"
        className="ml-auto inline-flex items-center gap-2 rounded-md px-2 py-1 text-sm text-gray-500 hover:bg-gray-50 hover:text-gray-900"
        onClick={onDelete}
      >
        <Trash2 size={15} />
        删除
      </button>
    </div>
  )
}

function ConfigTestBar({
  state,
  label,
  onTest,
}: {
  state?: ConfigTestState
  label: string
  onTest: () => void
}) {
  const running = state?.status === 'running'
  const tone =
    state?.status === 'ok'
      ? 'border-emerald-200 bg-emerald-50 text-emerald-700'
      : state?.status === 'error'
        ? 'border-red-200 bg-red-50 text-red-700'
        : 'border-gray-200 bg-gray-50 text-gray-600'
  return (
    <div className="flex flex-wrap items-center gap-3 rounded-md border border-gray-200 bg-gray-50 p-3">
      <button
        type="button"
        className="inline-flex items-center justify-center rounded-md border border-gray-300 bg-white px-3 py-2 text-sm font-medium text-gray-800 hover:bg-gray-50 disabled:cursor-not-allowed disabled:opacity-60"
        disabled={running}
        onClick={onTest}
      >
        {running ? '测试中...' : label}
      </button>
      {state && (
        <div className={`min-w-0 flex-1 rounded-md border px-3 py-2 text-sm ${tone}`}>
          {state.message}
        </div>
      )}
    </div>
  )
}

function SettingsPanel({
  icon: Icon,
  title,
  description,
  children,
}: {
  icon: LucideIcon
  title: string
  description: string
  children: ReactNode
}) {
  return (
    <section className="rounded-lg border border-gray-200 bg-white p-5">
      <div className="mb-5 flex items-start gap-3">
        <div className="flex h-9 w-9 items-center justify-center rounded-md border border-gray-200 bg-gray-50 text-gray-700">
          <Icon size={18} />
        </div>
        <div>
          <h3 className="text-sm font-semibold text-gray-900">{title}</h3>
          <p className="mt-1 text-sm text-gray-500">{description}</p>
        </div>
      </div>
      <div className="space-y-5">{children}</div>
    </section>
  )
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="block">
      <span className="mb-1 block text-sm font-medium text-gray-700">{label}</span>
      {children}
    </label>
  )
}

function TextInput({
  className = '',
  onChange,
  ...props
}: Omit<React.InputHTMLAttributes<HTMLInputElement>, 'onChange'> & {
  onChange: (value: string) => void
}) {
  return (
    <input
      {...props}
      className={`${inputClassName} ${className}`}
      onChange={(event) => onChange(event.target.value)}
    />
  )
}

function legacyFieldsFromLlmConfig(config: LlmModelConfig) {
  return {
    provider: config.provider,
    model: config.model,
    apiKey: config.apiKey,
    baseUrl: config.baseUrl,
    chatPath: config.chatPath,
  }
}

const inputClassName =
  'w-full rounded-md border border-gray-300 bg-white px-3 py-2 text-sm outline-none focus:border-gray-900'
