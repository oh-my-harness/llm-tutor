import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const ts = require('typescript')
const source = readFileSync(new URL('./onboardingModes.ts', import.meta.url), 'utf8')
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText
const module = { exports: {} }
Function('module', 'exports', compiled)(module, module.exports)

const {
  initialOnboardingMode,
  onboardingModeBlock,
  onboardingStarterPrompt,
} = module.exports

function tutor(overrides = {}) {
  return {
    name: 'Tutor A',
    default_capability: 'chat',
    allowed_capabilities: ['chat', 'research', 'quiz', 'organize'],
    resource_permissions: { notebook: true },
    ...overrides,
  }
}

test('temporary assistant exposes every onboarding mode', () => {
  for (const mode of ['chat', 'research', 'quiz', 'organize']) {
    assert.equal(onboardingModeBlock(mode, null), null)
  }
})

test('tutor capability and Notebook permission block mode launch independently', () => {
  assert.equal(onboardingModeBlock('research', tutor({ allowed_capabilities: ['chat'] })), 'capability')
  assert.equal(onboardingModeBlock('organize', tutor({ resource_permissions: { notebook: false } })), 'notebook')
  assert.equal(onboardingModeBlock('quiz', tutor()), null)
})

test('initial mode honors an available Tutor default and falls back when blocked', () => {
  assert.equal(initialOnboardingMode(tutor({ default_capability: 'research' })), 'research')
  assert.equal(initialOnboardingMode(tutor({ default_capability: 'organize', resource_permissions: { notebook: false } })), 'chat')
})

test('starter prompts are shared across preview and launch languages', () => {
  assert.match(onboardingStarterPrompt('organize', 'zh-CN'), /整理 Notebook/)
  assert.match(onboardingStarterPrompt('research', 'en-US'), /clarify the scope/)
})
