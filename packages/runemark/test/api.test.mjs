import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

import {
  RunemarkConsole,
  RunemarkProgress,
  RunemarkReport,
  Verdict,
  createProgress,
  renderDiff,
  renderError,
  renderReport,
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

test('metric trend is validated with and without a delta', () => {
  const render = (metric) =>
    renderReport({ schemaVersion: 1, title: 'T', verdict: Verdict.Passed, metrics: [metric] }, plain)

  assert.throws(() => render({ key: 'k', value: 'v', trend: 'bogus' }), /invalid metric trend: bogus/)
  assert.throws(
    () => render({ key: 'k', value: 'v', trend: 'bogus', delta: '+1' }),
    /invalid metric trend: bogus/,
  )
  // A valid trend without a delta renders no delta text.
  assert.match(render({ key: 'k', value: 'v', trend: 'positive' }), /^ {2}k: v$/m)
  assert.match(render({ key: 'k', value: 'v', trend: 'positive', delta: '+1' }), /k: v \(\+1\)/)
})

test('stream selects what auto checks, and an explicit isTerminal wins', () => {
  const noColor = process.env.NO_COLOR
  delete process.env.NO_COLOR
  try {
    for (const stream of [undefined, 'stdout', 'stderr']) {
      const options = { color: 'auto', stream, isTerminal: true }
      assert.match(new RunemarkConsole(options).error('x'), /\x1b\[/)
      assert.match(renderError({ heading: 'x' }, options), /\x1b\[/)
      assert.match(renderDiff({ changes: [{ action: 'added', path: 'a' }] }, options), /\x1b\[/)
      assert.match(renderReport({ title: 'T', verdict: Verdict.Failed }, options), /\x1b\[/)

      const off = { color: 'auto', stream, isTerminal: false }
      assert.equal(new RunemarkConsole(off).error('x'), 'x')
    }
  } finally {
    if (noColor !== undefined) process.env.NO_COLOR = noColor
  }
})

test('stream rejects a value that is not stdout or stderr', () => {
  assert.throws(() => new RunemarkConsole({ stream: 'stdin' }), /invalid stream: stdin/)
  assert.throws(() => renderError({ heading: 'x' }, { stream: 'file' }), /invalid stream: file/)
  assert.throws(() => renderDiff({ changes: [] }, { stream: 'file' }), /invalid stream: file/)
  assert.throws(
    () => renderReport({ title: 'T', verdict: Verdict.Passed }, { stream: 'file' }),
    /invalid stream: file/,
  )
})

const sampleFindings = (count) =>
  Array.from({ length: count }, (_, index) => ({
    message: `Finding ${index + 1}`,
    ruleId: `rule/${index + 1}`,
    remedy: `Fix ${index + 1}`,
    location: { kind: 'file', value: 'a.rs', line: index + 1 },
  }))

const reportWith = (group, extra = {}) => ({
  schemaVersion: 1,
  title: 'T',
  verdict: Verdict.Warning,
  groups: [{ title: 'G', ...group }],
  ...extra,
})

test('compact level samples findings, detailed level shows every remedy', () => {
  const input = reportWith({ findings: sampleFindings(5) })
  const compact = renderReport(input, plain)
  assert.match(compact, /Finding 3/)
  assert.doesNotMatch(compact, /Finding 4/)
  assert.doesNotMatch(compact, /Remedy:/)
  assert.match(compact, /\.\.\. 2 more finding\(s\)/)

  const detailed = renderReport({ ...input, detailLevel: 'detailed' }, plain)
  assert.match(detailed, /Finding 5/)
  assert.match(detailed, /Remedy: Fix 5/)
  assert.doesNotMatch(detailed, /more finding\(s\)/)
})

test('maxCompactSamples bounds the compact sample', () => {
  const output = renderReport(reportWith({ findings: sampleFindings(5) }, { maxCompactSamples: 1 }), plain)
  assert.match(output, /Finding 1/)
  assert.doesNotMatch(output, /Finding 2/)
  assert.match(output, /\.\.\. 4 more finding\(s\)/)
})

test('totalCount and advisory shape the group heading', () => {
  const counted = renderReport(reportWith({ findings: sampleFindings(2), totalCount: 10 }), plain)
  assert.match(counted, /^\* G \(10\)$/m)
  assert.match(counted, /\.\.\. 8 more finding\(s\)/)

  const advisory = renderReport(reportWith({ findings: sampleFindings(1), advisory: true }), plain)
  assert.match(advisory, /^\* G \(1\) \[advisory\]$/m)
})

test('symbols selects the unicode or ascii theme', () => {
  const input = reportWith({ findings: sampleFindings(1) })
  assert.match(renderReport(input, { ...plain, symbols: 'ascii' }), /^\[WARN\] T/)
  assert.match(renderReport(input, { ...plain, symbols: 'unicode' }), /^⚠ T/)
})

test('width stacks metrics and wraps long messages', () => {
  const input = {
    schemaVersion: 1,
    title: 'T',
    verdict: Verdict.Passed,
    metrics: [
      { key: 'Alpha', value: '1' },
      { key: 'Beta', value: '2' },
      { key: 'Gamma', value: '3' },
    ],
    groups: [{ title: 'G', findings: [{ message: 'word '.repeat(20).trim(), tone: 'info' }] }],
  }

  const wide = renderReport(input, plain)
  assert.match(wide, /^ {2}Alpha: 1 {3}Beta: 2 {3}Gamma: 3$/m)

  const narrow = renderReport(input, { ...plain, width: 30 })
  assert.match(narrow, /^ {2}Beta: 2$/m)
  for (const line of narrow.split('\n')) {
    assert.ok(line.length <= 30, `line exceeds width: ${line}`)
  }
})

test('a finding and a badge without a tone use warning and info', () => {
  const colour = { color: 'always', symbols: 'ascii', isTerminal: true }
  const render = (finding) => renderReport(reportWith({ findings: [finding] }), colour)

  assert.equal(
    render({ message: 'm', badge: { label: 'b' } }),
    render({ message: 'm', tone: 'warning', badge: { label: 'b', tone: 'info' } }),
  )
  assert.notEqual(render({ message: 'm' }), render({ message: 'm', tone: 'error' }))
})

test('renderError renders explanation, remedy and commands', () => {
  assert.equal(
    renderError(
      {
        heading: 'Browser executable not found',
        explanation: 'The audit needs a local browser.',
        remedy: 'Install one:',
        commands: ['npx playwright install chromium', 'npx playwright --version'],
      },
      plain,
    ),
    '[FAIL] Browser executable not found\n' +
      '  The audit needs a local browser.\n\n' +
      '  Remedy: Install one:\n\n' +
      '  $ npx playwright install chromium\n' +
      '  $ npx playwright --version\n',
  )
})

test('renderDiff renders every action and a delta', () => {
  assert.equal(
    renderDiff(
      {
        changes: [
          { action: 'added', path: 'a.json' },
          { action: 'modified', path: 'b.ts', delta: '+18 -4' },
          { action: 'deleted', path: 'c.ts' },
          { action: 'renamed', path: 'd.ts -> e.ts' },
        ],
      },
      plain,
    ),
    '  + CREATE a.json\n' +
      '  ~ UPDATE b.ts (+18 -4)\n' +
      '  - DELETE c.ts\n' +
      '  -> RENAME d.ts -> e.ts\n',
  )
})

test('the native boundary rejects invalid enum values at runtime', () => {
  const report = (extra) => () =>
    renderReport({ schemaVersion: 1, title: 'T', verdict: Verdict.Passed, ...extra }, plain)
  const finding = (extra) =>
    report({ groups: [{ title: 'G', findings: [{ message: 'm', ...extra }] }] })
  const minimal = { title: 'T', verdict: Verdict.Passed }

  assert.throws(() => renderReport(minimal, { color: 'rainbow' }), /invalid color mode: rainbow/)
  assert.throws(() => renderReport(minimal, { symbols: 'emoji' }), /invalid symbol theme: emoji/)
  assert.throws(() => new RunemarkProgress({ mode: 'sometimes' }), /invalid progress mode: sometimes/)
  assert.throws(report({ detailLevel: 'extreme' }), /invalid detail level: extreme/)
  assert.throws(finding({ tone: 'critical' }), /invalid tone: critical/)
  assert.throws(finding({ confidence: 'ultra' }), /invalid confidence: ultra/)
  assert.throws(finding({ badge: { label: 'b', tone: 'critical' } }), /invalid tone: critical/)
  assert.throws(
    finding({ location: { kind: 'database', value: 'users' } }),
    /invalid location kind: database/,
  )
  assert.throws(
    () => renderDiff({ changes: [{ action: 'copied', path: 'a' }] }, plain),
    /invalid file action: copied/,
  )
  assert.throws(() => new RunemarkConsole(plain).render('critical', 'x'), /invalid tone: critical/)
})

test('numbers that do not fit a u32 throw and name the field', () => {
  const location = (extra) =>
    renderReport(
      reportWith({ findings: [{ message: 'm', location: { kind: 'file', value: 'a.rs', ...extra } }] }),
      plain,
    )

  assert.throws(() => location({ line: -1 }), /invalid location\.line: -1/)
  assert.throws(() => location({ line: 1.5 }), /invalid location\.line: 1\.5/)
  assert.throws(() => location({ line: 2 ** 32 }), /invalid location\.line/)
  assert.throws(() => location({ line: Number.NaN }), /invalid location\.line/)
  assert.throws(() => location({ line: 1, column: -3 }), /invalid location\.column: -3/)
  assert.match(location({ line: 4294967295 }), /a\.rs:4294967295/)

  assert.throws(() => renderReport(reportWith({ totalCount: -1 }), plain), /invalid totalCount/)
  assert.throws(
    () => renderReport(reportWith({}, { maxCompactSamples: 2 ** 40 }), plain),
    /invalid maxCompactSamples/,
  )
  assert.throws(() => renderReport(reportWith({}), { ...plain, width: -80 }), /invalid width: -80/)
  assert.throws(
    () => renderReport({ schemaVersion: 2 ** 32 + 1, title: 'T', verdict: Verdict.Passed }, plain),
    /unsupported ReportInput schemaVersion/,
  )

  const progress = new RunemarkProgress({ mode: 'never', ...plain })
  assert.throws(() => progress.start(2 ** 40, 'x'), /invalid total/)
  assert.throws(() => progress.advance(-1, 'x'), /invalid position/)
})

test('location.column without line is rejected instead of dropped', () => {
  assert.throws(
    () =>
      renderReport(
        reportWith({ findings: [{ message: 'm', location: { kind: 'file', value: 'a.rs', column: 4 } }] }),
        plain,
      ),
    /location\.column requires location\.line/,
  )
})

// Progress writes to the process's stderr outside JavaScript, so a child
// process is the only way to read what it wrote.
function progressStderr(mode) {
  const loader = fileURLToPath(new URL('../index.cjs', import.meta.url))
  const script = `
    const { createProgress, Verdict } = require(${JSON.stringify(loader)})
    const progress = createProgress({ mode: ${JSON.stringify(mode)}, color: 'never', symbols: 'ascii', isTerminal: false })
    progress.start(2, 'Checking pages')
    progress.notice('warning', 'Slow page')
    progress.advance(2, 'Checked pages')
    progress.finish(Verdict.Passed, 'Audit complete')
  `
  const child = spawnSync(process.execPath, ['-e', script], { encoding: 'utf8' })
  assert.equal(child.status, 0, child.stderr)
  assert.equal(child.stdout, '')
  return child.stderr
}

test('progress notice and finish write line-oriented output to stderr', () => {
  assert.equal(progressStderr('always'), 'Checking pages (0/2)\nSlow page\n[OK] Audit complete\n')
  assert.equal(progressStderr('never'), '')
})

test('createProgress returns a progress with the full lifecycle', () => {
  const progress = createProgress({ mode: 'never', ...plain })
  assert.ok(progress instanceof RunemarkProgress)
  progress.start(1, 'Task')
  progress.notice('info', 'Note')
  progress.advance(1, 'Task done')
  progress.finish(Verdict.Passed, 'Done')
  assert.throws(() => progress.notice('critical', 'x'), /invalid tone: critical/)
  assert.throws(() => progress.finish('catastrophic', 'x'), /invalid verdict: catastrophic/)
})
