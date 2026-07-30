'use strict'

const { RunemarkReport, Verdict } = require('../../packages/runemark')

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
