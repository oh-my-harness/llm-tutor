import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const ts = require('typescript')
const source = readFileSync(new URL('./memoryReview.ts', import.meta.url), 'utf8')
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText
const module = { exports: {} }
Function('module', 'exports', compiled)(module, module.exports)
const { areAllMemoryChangesSelected, memoryChangeIds, toggleMemoryChange } = module.exports

test('collects stable change ids for default review selection', () => {
  assert.deepEqual(memoryChangeIds([{ id: 'a' }, { id: 'b' }]), ['a', 'b'])
})

test('toggles one reviewed change without disturbing the others', () => {
  assert.deepEqual(toggleMemoryChange(['a', 'b'], 'a'), ['b'])
  assert.deepEqual(toggleMemoryChange(['b'], 'a'), ['b', 'a'])
})

test('all-selected checks actual change ids instead of array length', () => {
  const changes = [{ id: 'a' }, { id: 'b' }]
  assert.equal(areAllMemoryChangesSelected(changes, ['a', 'b']), true)
  assert.equal(areAllMemoryChangesSelected(changes, ['a', 'stale']), false)
  assert.equal(areAllMemoryChangesSelected([], []), false)
})
