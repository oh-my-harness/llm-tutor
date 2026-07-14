import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const ts = require('typescript')
const source = readFileSync(new URL('./settings.ts', import.meta.url), 'utf8')
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText
const module = { exports: {} }
Function('module', 'exports', compiled)(module, module.exports)
const {
  defaultLlmSettings,
  normalizeTheme,
  settingsForSession,
  settingsRequireSessionReset,
} = module.exports

test('keeps supported appearance themes', () => {
  assert.equal(normalizeTheme('cool-light'), 'cool-light')
  assert.equal(normalizeTheme('graphite-dark'), 'graphite-dark')
})

test('migrates missing and unknown theme values to cool light', () => {
  assert.equal(normalizeTheme(undefined), 'cool-light')
  assert.equal(normalizeTheme('legacy-dark'), 'cool-light')
})

test('theme changes do not reset the active runtime session', () => {
  const darkSettings = { ...defaultLlmSettings, theme: 'graphite-dark' }
  assert.equal(settingsRequireSessionReset(defaultLlmSettings, darkSettings), false)
  assert.equal(settingsRequireSessionReset(darkSettings, darkSettings), false)
  assert.equal(
    settingsRequireSessionReset(darkSettings, { ...darkSettings, model: 'another-model' }),
    true,
  )
})

test('builds runtime settings from an explicitly selected model config', () => {
  const settings = {
    ...defaultLlmSettings,
    llmConfigs: [
      {
        id: 'model-a',
        name: 'Model A',
        provider: 'openai',
        model: 'model-a-name',
        apiKey: 'key-a',
        baseUrl: 'https://a.example',
        chatPath: '/v1/chat/completions',
        contextWindowTokens: 64000,
      },
      {
        id: 'model-b',
        name: 'Model B',
        provider: 'anthropic',
        model: 'model-b-name',
        apiKey: 'key-b',
        baseUrl: 'https://b.example',
        chatPath: '',
        contextWindowTokens: 200000,
      },
    ],
    activeLlmConfigId: 'model-a',
  }

  const selected = settingsForSession(settings, 'model-b')
  assert.equal(selected.provider, 'anthropic')
  assert.equal(selected.model, 'model-b-name')
  assert.equal(selected.api_key, 'key-b')
  assert.equal(selected.context_window_tokens, 200000)
})
