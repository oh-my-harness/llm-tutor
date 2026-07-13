import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const ts = require('typescript')

const source = readFileSync(new URL('./quizRestore.ts', import.meta.url), 'utf8')
const compiled = ts.transpileModule(source, {
  compilerOptions: {
    module: ts.ModuleKind.CommonJS,
    target: ts.ScriptTarget.ES2022,
  },
}).outputText

const module = { exports: {} }
Function('module', 'exports', 'require', compiled)(module, module.exports, require)
const { attachRestoredQuizzesToMessages } = module.exports

function quiz(id, title = 'Restored quiz') {
  return {
    id,
    title,
    kb_id: '__conversation__',
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
  }
}

test('restores quiz card onto the assistant message that owns the artifact', async () => {
  const expected = quiz('quiz-1')
  const restored = await attachRestoredQuizzesToMessages(
    [
      { role: 'assistant', text: 'Planning complete.' },
      {
        role: 'assistant',
        text: 'Quiz ready.',
        artifacts: [{ type: 'quiz_session', quiz_id: 'quiz-1' }],
      },
    ],
    [],
    async (id) => (id === expected.id ? expected : null),
  )

  assert.equal(restored[0].quiz, undefined)
  assert.equal(restored[1].quiz?.id, 'quiz-1')
})

test('creates a synthetic quiz card message when only a tool trace survived', async () => {
  const expected = quiz('quiz-2', 'Synthetic quiz')
  const restored = await attachRestoredQuizzesToMessages(
    [{ role: 'user', text: 'Make a quiz.' }],
    [
      {
        kind: 'tool_result',
        payload: {
          kind: 'tool_result',
          tool: 'create_quiz',
          ok: true,
          details: { quiz: expected },
        },
      },
    ],
    async () => null,
  )

  assert.equal(restored.length, 2)
  assert.equal(restored[1].role, 'assistant')
  assert.equal(restored[1].quiz?.id, 'quiz-2')
  assert.deepEqual(restored[1].artifacts, [{ type: 'quiz_session', quiz_id: 'quiz-2' }])
})
