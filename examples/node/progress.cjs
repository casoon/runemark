'use strict'

const { createProgress, Verdict } = require('../../packages/runemark')

const pages = [
  'https://example.com/',
  'https://example.com/about',
  'https://example.com/contact',
]
const progress = createProgress({ mode: 'auto' })

progress.start(pages.length, 'Checking pages')
for (const [index, page] of pages.entries()) {
  progress.advance(index + 1, page)
}
progress.finish(Verdict.Passed, 'Audit complete')
