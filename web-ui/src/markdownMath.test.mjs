import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const ts = require('typescript')
const source = readFileSync(new URL('./markdownMath.ts', import.meta.url), 'utf8')
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText
const module = { exports: {} }
Function('module', 'exports', compiled)(module, module.exports)
const { normalizeLatexMathDelimiters } = module.exports

test('normalizes common LLM inline and display math delimiters', () => {
  const markdown = [
    '函数 \\(f(x) = \\frac{1}{x} - a\\) 的导数。',
    '',
    '\\[',
    'Q = XW^Q, \\quad K = XW^K',
    '\\]',
  ].join('\n')

  assert.equal(normalizeLatexMathDelimiters(markdown), [
    '函数 $f(x) = \\frac{1}{x} - a$ 的导数。',
    '',
    '$$',
    'Q = XW^Q, \\quad K = XW^K',
    '$$',
  ].join('\n'))
})

test('preserves formulas shown as inline or fenced code', () => {
  const markdown = [
    '`\\(inline example\\)` and \\(real formula\\)',
    '',
    '```markdown',
    '\\[code example\\]',
    '```',
  ].join('\n')

  assert.equal(normalizeLatexMathDelimiters(markdown), [
    '`\\(inline example\\)` and $real formula$',
    '',
    '```markdown',
    '\\[code example\\]',
    '```',
  ].join('\n'))
})

test('preserves existing dollar math, escaped delimiters, and unmatched input', () => {
  const markdown = '$x + y$; \\\\(literal\\\\); \\(unclosed'
  assert.equal(normalizeLatexMathDelimiters(markdown), markdown)
})

test('supports inline code spans that continue across a line break', () => {
  const markdown = '``code \\(not math\ncontinued\\)`` then \\(math\\)'
  assert.equal(
    normalizeLatexMathDelimiters(markdown),
    '``code \\(not math\ncontinued\\)`` then $math$',
  )
})
