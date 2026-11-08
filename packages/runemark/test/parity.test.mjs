import assert from 'node:assert/strict'
import { readFileSync, readdirSync } from 'node:fs'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

import { renderDiff, renderError, renderReport } from '../index.cjs'

// The fixtures are written by the Rust core (tests/node_parity_fixtures.rs):
// each holds a Node input, the render options and the string the Rust
// renderer produced for them. Regenerate with RUNEMARK_BLESS=1.
const dir = new URL('../../../tests/fixtures/node-parity/', import.meta.url)
const renderers = { renderReport, renderError, renderDiff }

const files = readdirSync(dir).filter((name) => name.endsWith('.json')).sort()

test('the Rust parity fixtures are present', () => {
  assert.ok(files.length > 0, `no fixtures in ${fileURLToPath(dir)}`)
})

for (const name of files) {
  test(`byte-for-byte parity with the Rust core: ${name}`, () => {
    const fixture = JSON.parse(readFileSync(new URL(name, dir), 'utf8'))
    const render = renderers[fixture.function]
    assert.ok(render, `unknown renderer ${fixture.function}`)
    assert.equal(render(fixture.input, fixture.options), fixture.expected)
  })
}
