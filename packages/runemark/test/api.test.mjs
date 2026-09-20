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
  const progress = new RunemarkProgress({ mode: 'never', ...plain })

  progress.start(2, 'Checking files')
  progress.advance(2, 'Files checked')
  progress.finish(Verdict.Passed, 'Complete')
})

test('progress respects explicit isTerminal override for all combinations', () => {
  for (const isTerminal of [true, false]) {
    for (const mode of ['never', 'always', 'auto']) {
      const p = new RunemarkProgress({ mode, isTerminal, color: 'never' })
      assert.ok(p)
    }
  }
})

test('terminal escape sequences and disallowed URL schemes are safely neutralized', () => {
  const console = new RunemarkConsole({ color: 'never', isTerminal: false })
  const neutralized = console.info('Untrusted: \x1b[31mRed\x1b[0m\x07')
  assert.equal(neutralized, 'Untrusted: ^[[31mRed^[[0m^G')

  const report = new RunemarkReport({
    schemaVersion: 1,
    title: 'Security check',
    verdict: Verdict.Passed,
    groups: [{
      title: 'Links',
      findings: [{
        message: 'Malicious link',
        location: { kind: 'url', value: 'javascript:alert(1)' },
      }],
    }],
  })
  const rendered = report.render({ color: 'always', isTerminal: true })
  assert.ok(!rendered.includes('\x1b]8;;'), 'Must not create OSC 8 link for javascript: scheme')
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

test('a metric verdict is legible without colour', () => {
  // The gap this pins: a metric carried a tone and nothing else, and a tone is
  // nothing in a pipe or under NO_COLOR — "Errors: 3" read exactly like a
  // clean count. The Rust core gained the verdict in 0.6.0; Node did not get
  // it until 0.8.1.
  const report = new RunemarkReport({
    schemaVersion: 1,
    title: 'Audit',
    verdict: Verdict.Failed,
    metrics: [
      { key: 'Errors', value: '3', verdict: Verdict.Failed },
      { key: 'Warnings', value: '0', verdict: Verdict.Passed },
    ],
  })

  const output = report.render({ ...plain, width: 80 })

  assert.match(output, /\[FAIL\] Errors: 3/)
  assert.match(output, /\[OK\] Warnings: 0/)
})

test('a metric verdict rejects a value that is not one', () => {
  assert.throws(
    () =>
      new RunemarkReport({
        schemaVersion: 1,
        title: 'Audit',
        verdict: Verdict.Warning,
        metrics: [{ key: 'Errors', value: '3', verdict: 'catastrophic' }],
      }).render(plain),
    /verdict/,
  )
})

test('an explicit tone still wins over the verdict it was given', () => {
  const report = new RunemarkReport({
    schemaVersion: 1,
    title: 'Audit',
    verdict: Verdict.Warning,
    metrics: [{ key: 'Errors', value: '3', verdict: Verdict.Failed, tone: 'muted' }],
  })

  // The symbol comes from the verdict either way; only the colour is the
  // tone's, and plain output cannot show it.
  assert.match(report.render({ ...plain, width: 80 }), /\[FAIL\] Errors: 3/)
})

test('report rendering has exact byte-for-byte parity with Rust core', () => {
  const report = new RunemarkReport({
    schemaVersion: 1,
    title: 'Security & Quality Audit',
    verdict: Verdict.Warning,
    metrics: [
      { key: 'Issues', value: '2', tone: 'warning' },
      { key: 'Coverage', value: '88%', tone: 'success', trend: 'positive', delta: '+3%' },
    ],
    groups: [{
      title: 'Vulnerabilities',
      findings: [
        {
          message: 'SQL injection vulnerability',
          tone: 'error',
          ruleId: 'security/sql-injection',
          location: { kind: 'file', value: 'src/db.rs', line: 42 },
          confidence: 'high',
          badge: { label: 'critical', tone: 'error' },
        },
        {
          message: 'Hardcoded secret',
          tone: 'warning',
          ruleId: 'security/secret-leak',
          location: { kind: 'file', value: 'src/auth.rs', line: 10, column: 5 },
        },
      ],
    }],
    nextSteps: [
      { text: 'Fix critical vulnerabilities', command: 'cargo audit fix' },
    ],
  })

  const rendered = report.render(plain)
  const expected =
    '[WARN] Security & Quality Audit\n\n' +
    '  Issues: 2   Coverage: 88% (+3%)\n\n' +
    '* Vulnerabilities (2)\n' +
    '  - [critical] SQL injection vulnerability [security/sql-injection] (high confidence) at src/db.rs:42\n' +
    '  - Hardcoded secret [security/secret-leak] at src/auth.rs:10:5\n\n' +
    'Next steps:\n' +
    '  - Fix critical vulnerabilities\n' +
    '    $ cargo audit fix\n'

  assert.equal(rendered, expected)
})
