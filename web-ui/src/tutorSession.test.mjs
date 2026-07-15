import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const ts = require('typescript')
const source = readFileSync(new URL('./tutorSession.ts', import.meta.url), 'utf8')
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText
const module = { exports: {} }
Function('module', 'exports', compiled)(module, module.exports)
const { tutorBindingForCreate } = module.exports

test('keeps persistent and temporary tutor session bindings distinct', () => {
  assert.deepEqual(tutorBindingForCreate('general-tutor'), { tutor_id: 'general-tutor' })
  assert.deepEqual(tutorBindingForCreate(null), { tutor_id: null })
})

test('defaults a missing tutor choice to the temporary assistant', () => {
  assert.deepEqual(tutorBindingForCreate(undefined), { tutor_id: null })
})
