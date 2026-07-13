import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import test from 'node:test'
import { createRequire } from 'node:module'

const require = createRequire(import.meta.url)
const ts = require('typescript')
const source = readFileSync(new URL('./researchRestore.ts', import.meta.url), 'utf8')
const compiled = ts.transpileModule(source, {
  compilerOptions: { module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 },
}).outputText
const module = { exports: {} }
Function('module', 'exports', compiled)(module, module.exports)
const { attachRestoredResearchReports, researchReportFromTracePayload } = module.exports

test('uses the structured workflow title instead of the report first sentence', () => {
  const report = researchReportFromTracePayload({
    kind: 'tool_result',
    tool: 'create_research_report',
    details: {
      title: 'Transformer Architecture',
      markdown: 'This is the first sentence.\n\n## Summary\nDetails.',
    },
  })
  assert.equal(report.title, 'Transformer Architecture')
})

test('restores a Research report card from its durable run artifact', () => {
  const restored = attachRestoredResearchReports(
    [{
      role: 'assistant',
      text: 'Research report ready.',
      artifacts: [{
        type: 'research_report',
        artifact_store: 'runtime_trace',
        artifact_id: 'run-1',
        title: 'Transformer Architecture',
      }],
    }],
    [{
      payload: {
        kind: 'tool_result',
        tool: 'create_research_report',
        run_id: 'run-1',
        details: {
          title: 'Transformer Architecture',
          markdown: '# Report\n\n## Summary\nRestored.',
        },
      },
    }],
  )

  assert.equal(restored[0].text, '# Report\n\n## Summary\nRestored.')
  assert.equal(restored[0].researchTitle, 'Transformer Architecture')
  assert.equal(restored[0].researchUnavailable, false)
})

test('marks a missing Research trace instead of silently dropping the attachment', () => {
  const restored = attachRestoredResearchReports(
    [{
      role: 'assistant',
      text: 'Research report ready.',
      artifacts: [{ type: 'research_report', artifact_id: 'missing', title: 'Missing report' }],
    }],
    [],
  )
  assert.equal(restored[0].researchUnavailable, true)
  assert.equal(restored[0].researchTitle, 'Missing report')
})
