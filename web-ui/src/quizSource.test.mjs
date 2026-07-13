import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const ts = require('typescript')

const source = readFileSync(new URL('./quizSource.ts', import.meta.url), 'utf8')
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText

const module = { exports: {} }
Function('module', 'exports', compiled)(module, module.exports)
const { quizSourceType } = module.exports

function quiz(overrides = {}) {
  return {
    id: 'quiz-1',
    title: 'Sample quiz',
    kb_id: '',
    status: 'active',
    config: {
      topic: null,
      difficulty: 'medium',
      question_count: 1,
      question_type: 'single_choice',
    },
    questions: [
      {
        id: 'q1',
        question_type: 'single_choice',
        stem: 'Question?',
        options: [],
        correct_option_id: 'A',
        explanation: '',
        citations: [],
        tags: [],
        difficulty: 'medium',
      },
    ],
    answers: [],
    score: null,
    verification: null,
    created_at: '',
    updated_at: '',
    ...overrides,
  }
}

test('quiz source type honors stored sentinel ids before generic kb checks', () => {
  assert.equal(quizSourceType(quiz({ kb_id: '__notebook__' })), 'notebook')
  assert.equal(quizSourceType(quiz({ kb_id: '__conversation__' })), 'conversation')
  assert.equal(quizSourceType(quiz({ kb_id: 'kb-123' })), 'knowledge_base')
})

test('quiz source type falls back to source text when no kb id is stored', () => {
  assert.equal(
    quizSourceType(quiz({
      questions: [{
        ...quiz().questions[0],
        citations: [{ source: 'notebook: OPC report', text: '...', score: null }],
      }],
    })),
    'notebook',
  )
  assert.equal(quizSourceType(quiz({ title: 'Quiz from mentioned_space_items' })), 'space')
})
