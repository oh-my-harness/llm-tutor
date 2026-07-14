import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const ts = require('typescript')
const source = readFileSync(new URL('./messageActions.ts', import.meta.url), 'utf8')
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText
const module = { exports: {} }
Function('module', 'exports', compiled)(module, module.exports)
const { appendMessageQuote, formatMessageQuote, previousUserMessageIndex } = module.exports

test('formats a clearly delimited multi-line message quote', () => {
  assert.equal(
    formatMessageQuote('assistant', 'First line\n\nSecond line'),
    '> **Quoted assistant message**\n> First line\n> \n> Second line',
  )
})

test('appends a quote after existing composer text', () => {
  assert.equal(
    appendMessageQuote('My question', 'user', 'Original prompt'),
    'My question\n\n> **Quoted user message**\n> Original prompt',
  )
})

test('finds the prior user turn across status and assistant messages', () => {
  const messages = [
    { role: 'user' },
    { role: 'status' },
    { role: 'assistant' },
    { role: 'assistant' },
  ]
  assert.equal(previousUserMessageIndex(messages, 3), 0)
  assert.equal(previousUserMessageIndex(messages, 0), -1)
})
