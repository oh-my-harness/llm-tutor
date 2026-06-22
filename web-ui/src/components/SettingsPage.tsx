import { useState, type ReactNode } from 'react'
import {
  Activity,
  Brain,
  Database,
  Palette,
  Plus,
  SlidersHorizontal,
  Trash2,
  type LucideIcon,
} from 'lucide-react'
import { createEmbeddingConfig } from '../settings'
import type { EmbeddingModelConfig, LlmProvider, LlmSettings } from '../settings'

interface Props {
  settings: LlmSettings
  onChange: (settings: LlmSettings) => void
}

const providerOptions: { value: LlmProvider; label: string; model: string; baseUrl: string; chatPath: string }[] = [
  {
    value: 'deepseek',
    label: 'DeepSeek',
    model: 'deepseek-v4-flash',
    baseUrl: 'https://api.deepseek.com',
    chatPath: '/chat/completions',
  },
  {
    value: 'anthropic',
    label: 'Anthropic',
    model: 'claude-haiku-4-5-20251001',
    baseUrl: 'https://api.anthropic.com',
    chatPath: '',
  },
  {
    value: 'openai',
    label: 'OpenAI compatible',
    model: 'gpt-4o-mini',
    baseUrl: 'https://api.openai.com',
    chatPath: '/v1/chat/completions',
  },
]

type SettingsTab = 'appearance' | 'llm' | 'embedding' | 'governance'

const settingsTabs: Array<{ key: SettingsTab; label: string; icon: LucideIcon }> = [
  { key: 'appearance', label: '外观', icon: Palette },
  { key: 'llm', label: 'LLM', icon: Brain },
  { key: 'embedding', label: '嵌入模型', icon: Database },
  { key: 'governance', label: '能力', icon: SlidersHorizontal },
]

export function SettingsPage({ settings, onChange }: Props) {
  const [activeTab, setActiveTab] = useState<SettingsTab>('embedding')

  const update = <K extends keyof LlmSettings>(key: K, value: LlmSettings[K]) => {
    onChange({ ...settings, [key]: value })
  }

  const activeEmbeddingConfig =
    settings.embeddingConfigs.find((config) => config.id === settings.activeEmbeddingConfigId) ?? null

  const selectProvider = (provider: LlmProvider) => {
    const option = providerOptions.find((item) => item.value === provider)
    if (!option) return
    onChange({
      ...settings,
      provider,
      model: option.model,
      baseUrl: option.baseUrl,
      chatPath: option.chatPath,
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

  return (
    <main className="flex min-h-0 flex-1 bg-gray-50">
      <aside className="hidden w-64 shrink-0 border-r border-gray-200 bg-white px-4 py-6 md:block">
        <div className="mb-8 px-2">
          <h2 className="text-xl font-semibold text-gray-900">设置</h2>
          <p className="mt-2 text-sm leading-6 text-gray-600">调整外观、模型服务和运行能力。</p>
        </div>

        <nav className="space-y-1">
          {settingsTabs.map((tab) => {
            const Icon = tab.icon
            const active = activeTab === tab.key
            return (
              <button
                key={tab.key}
                type="button"
                className={`flex w-full items-center gap-3 px-3 py-2.5 text-left text-sm ${
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
                className="border border-gray-300 bg-white px-3 py-2 text-sm"
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
              <div className="flex items-center justify-between border border-gray-200 px-4 py-3">
                <div>
                  <div className="text-sm font-medium text-gray-900">界面语言</div>
                  <div className="mt-1 text-sm text-gray-500">当前使用中文界面。</div>
                </div>
                <div className="inline-flex border border-gray-300 bg-gray-50 p-1 text-sm">
                  <button type="button" className="px-3 py-1 text-gray-500">
                    English
                  </button>
                  <button type="button" className="bg-white px-3 py-1 text-gray-900 shadow-sm">
                    中文
                  </button>
                </div>
              </div>
            </SettingsPanel>
          )}

          {activeTab === 'llm' && (
            <SettingsPanel icon={Brain} title="LLM" description="新会话会使用这里配置的对话模型服务。">
              <div>
                <label className="mb-2 block text-sm font-medium text-gray-700">Provider</label>
                <div className="inline-flex overflow-hidden border border-gray-300 text-sm">
                  {providerOptions.map((option) => (
                    <button
                      key={option.value}
                      type="button"
                      className={`px-4 py-2 ${
                        settings.provider === option.value
                          ? 'bg-gray-900 text-white'
                          : 'bg-white text-gray-700 hover:bg-gray-50'
                      }`}
                      onClick={() => selectProvider(option.value)}
                    >
                      {option.label}
                    </button>
                  ))}
                </div>
              </div>

              <div className="grid gap-4 md:grid-cols-2">
                <Field label="Model">
                  <input
                    className="w-full border border-gray-300 px-3 py-2 text-sm outline-none focus:border-gray-900"
                    value={settings.model}
                    onChange={(event) => update('model', event.target.value)}
                  />
                </Field>

                <Field label="API key">
                  <input
                    className="w-full border border-gray-300 px-3 py-2 text-sm outline-none focus:border-gray-900"
                    type="password"
                    value={settings.apiKey}
                    onChange={(event) => update('apiKey', event.target.value)}
                    placeholder="sk-..."
                  />
                </Field>

                <Field label="Base URL">
                  <input
                    className="w-full border border-gray-300 px-3 py-2 text-sm outline-none focus:border-gray-900"
                    value={settings.baseUrl}
                    onChange={(event) => update('baseUrl', event.target.value)}
                  />
                </Field>

                <Field label="Chat path">
                  <input
                    className="w-full border border-gray-300 px-3 py-2 text-sm outline-none focus:border-gray-900"
                    value={settings.chatPath}
                    onChange={(event) => update('chatPath', event.target.value)}
                    placeholder="/v1/chat/completions"
                  />
                </Field>
              </div>
            </SettingsPanel>
          )}

          {activeTab === 'embedding' && (
            <SettingsPanel icon={Database} title="嵌入模型" description="知识库入库和检索会使用这里的向量模型配置。">
              {settings.embeddingConfigs.length === 0 ? (
                <div className="flex min-h-40 flex-col items-center justify-center border border-dashed border-gray-300 bg-gray-50 px-4 py-8 text-center">
                  <p className="text-sm text-gray-500">暂无配置文件。</p>
                  <button
                    type="button"
                    className="mt-4 inline-flex items-center gap-2 border border-gray-300 bg-white px-3 py-2 text-sm text-gray-800 hover:bg-gray-50"
                    onClick={addEmbeddingConfig}
                  >
                    <Plus size={16} />
                    添加配置
                  </button>
                </div>
              ) : (
                <div className="grid gap-5 lg:grid-cols-[230px_1fr]">
                  <div className="space-y-2">
                    {settings.embeddingConfigs.map((config) => (
                      <button
                        key={config.id}
                        type="button"
                        className={`w-full border px-4 py-3 text-left ${
                          config.id === settings.activeEmbeddingConfigId
                            ? 'border-gray-900 bg-gray-50'
                            : 'border-gray-200 bg-white hover:bg-gray-50'
                        }`}
                        onClick={() => update('activeEmbeddingConfigId', config.id)}
                      >
                        <div className="text-sm font-semibold text-gray-900">{config.name || 'OpenAI'}</div>
                        <div className="mt-1 truncate text-xs text-gray-500">{config.model || '未设置模型'}</div>
                      </button>
                    ))}
                    <button
                      type="button"
                      className="inline-flex w-full items-center justify-center gap-2 border border-gray-300 bg-white px-3 py-2 text-sm text-gray-800 hover:bg-gray-50"
                      onClick={addEmbeddingConfig}
                    >
                      <Plus size={16} />
                      添加配置
                    </button>
                  </div>

                  {activeEmbeddingConfig && (
                    <div className="space-y-5 border border-gray-200 p-4">
                      <div className="flex items-center gap-3">
                        <div>
                          <h4 className="text-sm font-semibold text-gray-900">提供商连接</h4>
                          <p className="mt-1 text-xs text-gray-500">OpenAI-compatible embedding endpoint.</p>
                        </div>
                        <button
                          type="button"
                          className="ml-auto inline-flex items-center gap-2 px-2 py-1 text-sm text-gray-500 hover:bg-gray-50 hover:text-gray-900"
                          onClick={() => deleteEmbeddingConfig(activeEmbeddingConfig.id)}
                        >
                          <Trash2 size={15} />
                          删除
                        </button>
                      </div>

                      <div className="grid gap-4 md:grid-cols-2">
                        <Field label="配置名称">
                          <input
                            className="w-full border border-gray-300 px-3 py-2 text-sm outline-none focus:border-gray-900"
                            value={activeEmbeddingConfig.name}
                            onChange={(event) =>
                              updateEmbeddingConfig(activeEmbeddingConfig.id, 'name', event.target.value)
                            }
                          />
                        </Field>

                        <Field label="提供商">
                          <select
                            className="w-full border border-gray-300 bg-white px-3 py-2 text-sm outline-none focus:border-gray-900"
                            value={activeEmbeddingConfig.provider}
                            onChange={() => updateEmbeddingConfig(activeEmbeddingConfig.id, 'provider', 'openai')}
                          >
                            <option value="openai">OpenAI compatible</option>
                          </select>
                        </Field>

                        <Field label="Base URL">
                          <input
                            className="w-full border border-gray-300 px-3 py-2 text-sm outline-none focus:border-gray-900"
                            value={activeEmbeddingConfig.baseUrl}
                            onChange={(event) =>
                              updateEmbeddingConfig(activeEmbeddingConfig.id, 'baseUrl', event.target.value)
                            }
                            placeholder="https://api.openai.com"
                          />
                        </Field>

                        <Field label="Embeddings path">
                          <input
                            className="w-full border border-gray-300 px-3 py-2 text-sm outline-none focus:border-gray-900"
                            value={activeEmbeddingConfig.embeddingsPath}
                            onChange={(event) =>
                              updateEmbeddingConfig(activeEmbeddingConfig.id, 'embeddingsPath', event.target.value)
                            }
                            placeholder="/v1/embeddings"
                          />
                        </Field>

                        <Field label="API Key">
                          <input
                            className="w-full border border-gray-300 px-3 py-2 text-sm outline-none focus:border-gray-900"
                            type="password"
                            value={activeEmbeddingConfig.apiKey}
                            onChange={(event) =>
                              updateEmbeddingConfig(activeEmbeddingConfig.id, 'apiKey', event.target.value)
                            }
                            placeholder="sk-..."
                          />
                        </Field>

                        <Field label="模型 ID">
                          <input
                            className="w-full border border-gray-300 px-3 py-2 text-sm outline-none focus:border-gray-900"
                            value={activeEmbeddingConfig.model}
                            onChange={(event) =>
                              updateEmbeddingConfig(activeEmbeddingConfig.id, 'model', event.target.value)
                            }
                            placeholder="text-embedding-3-small"
                          />
                        </Field>

                        <Field label="维度">
                          <input
                            className="w-full border border-gray-300 px-3 py-2 text-sm outline-none focus:border-gray-900"
                            min="1"
                            type="number"
                            value={activeEmbeddingConfig.dimensions}
                            onChange={(event) =>
                              updateEmbeddingConfig(
                                activeEmbeddingConfig.id,
                                'dimensions',
                                Number(event.target.value),
                              )
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

          {activeTab === 'governance' && (
            <SettingsPanel icon={Activity} title="能力" description="预算和工具执行审批会影响新建会话。">
              <Field label="Session budget">
                <div className="flex items-center gap-3">
                  <input
                    className="w-36 border border-gray-300 px-3 py-2 text-sm outline-none focus:border-gray-900"
                    min="0"
                    step="0.25"
                    type="number"
                    value={settings.budgetLimitUsd}
                    onChange={(event) => update('budgetLimitUsd', Number(event.target.value))}
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
  if (tab === 'llm') return '配置对话模型服务。'
  if (tab === 'embedding') return '配置知识库检索使用的嵌入模型。'
  return '配置预算和工具执行策略。'
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
    <section className="border border-gray-200 bg-white p-5">
      <div className="mb-5 flex items-start gap-3">
        <div className="flex h-9 w-9 items-center justify-center border border-gray-200 bg-gray-50 text-gray-700">
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
