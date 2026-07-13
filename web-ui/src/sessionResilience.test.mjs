import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const ts = require('typescript')
const source = readFileSync(new URL('./sessionResilience.ts', import.meta.url), 'utf8')
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText
const module = { exports: {} }
Function('module', 'exports', compiled)(module, module.exports)
const { isCurrentSessionEvent, isLatestSessionLoad, reconcileSessionRunState } = module.exports

test('rejects events and session loads from a stale selection', () => {
  assert.equal(isCurrentSessionEvent('session-a', 'session-b'), false)
  assert.equal(isCurrentSessionEvent('session-b', 'session-b'), true)
  assert.equal(isLatestSessionLoad(1, 2, 'session-a', 'session-b'), false)
  assert.equal(isLatestSessionLoad(2, 2, 'session-b', 'session-b'), true)
})

test('reconciles run indicators without changing session order or titles', () => {
  const current = [
    { id: 'pinned', title: 'Pinned', activeRun: { status: 'running' } },
    { id: 'recent', title: 'Recent', activeRun: { status: 'running' } },
  ]
  const reconciled = reconcileSessionRunState(current, [
    { id: 'recent', activeRun: null },
    { id: 'pinned', activeRun: { status: 'running', current_stage: 'search' } },
  ])

  assert.deepEqual(reconciled.map((session) => session.id), ['pinned', 'recent'])
  assert.equal(reconciled[0].title, 'Pinned')
  assert.equal(reconciled[0].activeRun.current_stage, 'search')
  assert.equal(reconciled[1].activeRun, null)
})
