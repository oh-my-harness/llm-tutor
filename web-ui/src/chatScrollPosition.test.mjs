import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const ts = require('typescript')
const source = readFileSync(new URL('./chatScrollPosition.ts', import.meta.url), 'utf8')
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText
const module = { exports: {} }
Function('module', 'exports', compiled)(module, module.exports)
const { parseChatScrollPosition, restoredScrollTop } = module.exports

test('parses valid positions and rejects damaged local state', () => {
  assert.deepEqual(
    parseChatScrollPosition('{"scrollTop":320,"atBottom":false}'),
    { scrollTop: 320, atBottom: false },
  )
  assert.equal(parseChatScrollPosition('{"scrollTop":"320","atBottom":false}'), null)
  assert.equal(parseChatScrollPosition('not json'), null)
})

test('restores readers to their prior position and bottom followers to latest content', () => {
  assert.equal(restoredScrollTop({ scrollTop: 320, atBottom: false }, 2000, 600), 320)
  assert.equal(restoredScrollTop({ scrollTop: 320, atBottom: true }, 2000, 600), 1400)
})

test('clamps a saved position when the available content became shorter', () => {
  assert.equal(restoredScrollTop({ scrollTop: 1200, atBottom: false }, 900, 600), 300)
  assert.equal(restoredScrollTop({ scrollTop: -20, atBottom: false }, 900, 600), 0)
})
