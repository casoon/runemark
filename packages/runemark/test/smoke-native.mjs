import assert from 'node:assert/strict'
import {
  RunemarkConsole,
  Verdict,
  renderReport,
  renderStatus,
} from '../index.cjs'

const console = new RunemarkConsole({ color: 'never', isTerminal: false })
assert.equal(console.success('Native smoke passed'), 'Native smoke passed')

const status = renderStatus(Verdict.Passed, 'Engine ready', { color: 'never', isTerminal: false })
assert.match(status, /\[OK\] Engine ready/)

const report = renderReport(
  {
    schemaVersion: 1,
    title: 'Runtime Validation',
    verdict: Verdict.Passed,
    metrics: [{ key: 'Targets', value: '6/6', tone: 'success' }],
  },
  { color: 'never', isTerminal: false },
)
assert.match(report, /\[OK\] Runtime Validation/)
assert.match(report, /Targets: 6\/6/)

process.stdout.write(
  `Native smoke validation passed on ${process.platform} (${process.arch})\n`,
)
