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
const { defaultLlmSettings, normalizeTheme, settingsRequireSessionReset } = module.exports

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
