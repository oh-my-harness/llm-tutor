import type { ReactNode } from 'react'
import type { LlmProvider, LlmSettings } from '../settings'

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

export function SettingsPage({ settings, onChange }: Props) {
  const update = <K extends keyof LlmSettings>(key: K, value: LlmSettings[K]) => {
    onChange({ ...settings, [key]: value })
  }

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

  return (
    <main className="flex-1 overflow-y-auto bg-gray-50">
      <div className="mx-auto max-w-4xl px-6 py-6">
        <div className="mb-5">
          <h2 className="text-xl font-semibold text-gray-900">Settings</h2>
          <p className="mt-1 text-sm text-gray-600">Configure the model provider used for new tutor sessions.</p>
        </div>

        <section className="border border-gray-200 bg-white p-5">
          <div className="grid gap-5 md:grid-cols-[220px_1fr]">
            <div>
              <h3 className="text-sm font-semibold text-gray-900">LLM provider</h3>
              <p className="mt-1 text-sm text-gray-500">New sessions use these values.</p>
            </div>

            <div className="space-y-5">
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
            </div>
          </div>
        </section>

        <section className="mt-4 border border-gray-200 bg-white p-5">
          <div className="grid gap-5 md:grid-cols-[220px_1fr]">
            <div>
              <h3 className="text-sm font-semibold text-gray-900">Governance</h3>
              <p className="mt-1 text-sm text-gray-500">Budget and approval defaults.</p>
            </div>

            <div className="space-y-5">
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
            </div>
          </div>
        </section>
      </div>
    </main>
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
