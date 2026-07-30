'use strict'

const { RunemarkConsole } = require('../../packages/runemark')

const console = new RunemarkConsole({ color: 'auto' })

console.writeInfo('Starting site audit')
console.writeSuccess('42 pages checked')
console.writeWarning('3 images need alt text')
console.writeError('Audit could not finish')

const text = console.success('Build complete')
process.stdout.write(`${text}\n`)
