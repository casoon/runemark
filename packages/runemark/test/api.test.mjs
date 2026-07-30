import assert from 'node:assert/strict'
import test from 'node:test'

import {
  RunemarkConsole,
  RunemarkProgress,
  RunemarkReport,
  Verdict,
  renderDiff,
  renderError,
} from '../index.cjs'

const plain = { color: 'never', symbols: 'ascii', isTerminal: false }

test('console exposes render and write as separate operations', () => {
  const console = new RunemarkConsole(plain)
  const writes = []
  const stream = { write: (value) => writes.push(value) }

  assert.equal(console.success('Build completed'), 'Build completed')
  console.writeWarning('Review configuration', stream)
  assert.deepEqual(writes, ['Review configuration\n'])
})

test('report accepts a versioned object contract and renders plain output', () => {
  const report = new RunemarkReport({
    schemaVersion: 1,
    title: 'Project analysis',
    verdict: Verdict.Warning,
    metrics: [{ key: 'Issues', value: '1', tone: 'warning' }],
    groups: [{
      title: 'Code quality',
      findings: [{ message: 'src/index.ts contains 1,420 lines', ruleId: 'maintainability/file-size' }],
    }],
  })

  const output = report.render({ ...plain, width: 80 })

  assert.match(output, /^\[WARN\] Project analysis/m)
  assert.match(output, /Issues: 1/)
  assert.match(output, /maintainability\/file-size/)
})

test('report rejects unsupported schema versions at the native boundary', () => {
  assert.throws(
    () => new RunemarkReport({ schemaVersion: 2, title: 'Future report', verdict: Verdict.Info }),
    /unsupported ReportInput schemaVersion/,
  )
})

test('progress can be used without terminal output', () => {
  const progress = new RunemarkProgress({ mode: 'never', ...plain, total: 2 })

  progress.start(2, 'Checking files')
  progress.advance(2, 'Files checked')
  progress.finish(Verdict.Passed, 'Complete')
})

test('error and diff renderers use the same native presentation core', () => {
  assert.match(
    renderError({ heading: 'Missing browser', commands: ['auditmysite browser install'] }, plain),
    /^\[FAIL\] Missing browser/m,
  )
  assert.equal(
    renderDiff({ changes: [{ action: 'added', path: 'src/report.ts' }] }, plain),
    '  + CREATE src/report.ts\n',
  )
})
