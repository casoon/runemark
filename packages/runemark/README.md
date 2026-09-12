# @casoon/runemark

Native terminal presentation for Node.js command-line tools, powered by the
Runemark Rust renderer. It provides consistent status messages, reports,
errors, file-change previews, and progress output without maintaining a second
JavaScript renderer.

Requires Node.js 20 or newer.

## Install

```bash
npm install @casoon/runemark
```

## Status messages

`RunemarkConsole` renders text without writing it. Use the `write*` methods
when output should go directly to a stream.

```js
const { RunemarkConsole } = require('@casoon/runemark')

const console = new RunemarkConsole({ color: 'auto' })

console.writeInfo('Starting site audit')
console.writeSuccess('42 pages checked')
console.writeWarning('3 images need alt text')
console.writeError('Audit could not finish')

const text = console.success('Build complete')
process.stdout.write(`${text}\n`)
```

Use `color: 'never'` for snapshots and plain logs. `color: 'auto'` is
TTY-aware and is the default.

## Reports

Reports use a versioned plain-object contract. Keep `schemaVersion: 1` when
constructing reports; it prevents the Node interface from depending on Rust
implementation types.

```js
const { RunemarkReport, Verdict } = require('@casoon/runemark')

const report = new RunemarkReport({
  schemaVersion: 1,
  title: 'Accessibility audit',
  verdict: Verdict.Warning,
  metrics: [
    { key: 'Pages checked', value: '42', tone: 'success' },
    { key: 'Findings', value: '3', tone: 'warning' },
  ],
  groups: [
    {
      title: 'Images',
      findings: [
        {
          message: 'The logo has no text alternative',
          tone: 'warning',
          ruleId: 'wcag2a/1.1.1',
          location: { kind: 'url', value: 'https://example.com/' },
          remedy: 'Add a concise alt attribute to the image.',
        },
      ],
    },
  ],
  nextSteps: [
    { text: 'Fix the image alternative and run the audit again.' },
  ],
})

process.stdout.write(report.render({
  color: process.stdout.isTTY ? 'always' : 'never',
  width: process.stdout.columns,
}))
```

For one-off reports, use `renderReport(input, options)` instead.

## Errors and file changes

```js
const { renderDiff, renderError } = require('@casoon/runemark')

process.stderr.write(renderError({
  heading: 'Browser executable not found',
  explanation: 'The audit needs a local browser.',
  commands: ['npx playwright install chromium'],
}, { color: 'auto' }))

process.stdout.write(renderDiff({
  changes: [
    { action: 'added', path: 'reports/a11y.json' },
    { action: 'modified', path: 'src/audit.ts', delta: '+18 -4' },
  ],
}, { color: 'auto' }))
```

## Progress

Progress output uses stderr, leaving stdout free for JSON, SARIF, or other
machine-readable artifacts. With `mode: 'auto'`, progress is interactive only
for a terminal and stays line-oriented when redirected.

```js
const { createProgress, Verdict } = require('@casoon/runemark')

const progress = createProgress({ mode: 'auto' })
progress.start(42, 'Checking pages')

for (let page = 1; page <= 42; page += 1) {
  progress.advance(page, `Checked page ${page}`)
}

progress.finish(Verdict.Passed, 'Audit complete')
```

## API and compatibility

The stable public API consists of `RunemarkConsole`, `RunemarkReport`,
`RunemarkProgress`, and the `renderStatus`, `renderReport`, `renderError`,
`renderDiff`, and `createProgress` helpers. TypeScript declarations are bundled
with the package. See the repository's
[Node API contract](https://github.com/casoon/runemark/blob/main/NODE_API.md)
for the compatibility policy.
