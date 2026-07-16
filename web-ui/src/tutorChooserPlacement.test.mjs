import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const ts = require('typescript')
const source = readFileSync(new URL('./tutorChooserPlacement.ts', import.meta.url), 'utf8')
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText
const module = { exports: {} }
Function('module', 'exports', compiled)(module, module.exports)
const { placeTutorChooser } = module.exports

test('opens above a chooser anchored near the bottom of the chat window', () => {
  const placement = placeTutorChooser(
    { left: 240, top: 700, bottom: 748, width: 720 },
    { width: 1200, height: 800 },
  )

  assert.equal(placement.bottom, 108)
  assert.equal(placement.top, undefined)
  assert.equal(placement.maxHeight, 288)
  assert.equal(placement.width, 480)
})

test('opens below when there is not enough room above', () => {
  const placement = placeTutorChooser(
    { left: 24, top: 32, bottom: 80, width: 360 },
    { width: 800, height: 700 },
  )

  assert.equal(placement.top, 88)
  assert.equal(placement.bottom, undefined)
})

test('keeps the floating list inside a narrow viewport', () => {
  const placement = placeTutorChooser(
    { left: 250, top: 500, bottom: 548, width: 400 },
    { width: 320, height: 640 },
  )

  assert.equal(placement.left, 12)
  assert.equal(placement.width, 296)
})
