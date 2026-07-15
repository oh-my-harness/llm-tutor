import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const ts = require('typescript')
const source = readFileSync(new URL('./tutorTypes.ts', import.meta.url), 'utf8')
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText
const module = { exports: {} }
Function('module', 'exports', compiled)(module, module.exports)
const { normalizeTutorProfile } = module.exports

test('keeps native Soul Markdown unchanged', () => {
  const profile = normalizeTutorProfile({ soul_markdown: '# Identity\n\nTeach visually.' })
  assert.equal(profile.soul_markdown, '# Identity\n\nTeach visually.')
})

test('maps only stable legacy role fields into Soul Markdown', () => {
  const profile = normalizeTutorProfile({ role: 'Teach math', goal: 'Learn algebra' })
  assert.match(profile.soul_markdown, /Teach math/)
  assert.doesNotMatch(profile.soul_markdown, /Learn algebra/)
})
