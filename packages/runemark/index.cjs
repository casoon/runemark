'use strict'

const native = require('./native.cjs')

const Verdict = Object.freeze({
  Passed: 'passed',
  Warning: 'warning',
  Failed: 'failed',
  ActionRequired: 'action-required',
  Skipped: 'skipped',
  Info: 'info',
})

const Tone = Object.freeze({
  Title: 'title',
  Muted: 'muted',
  Info: 'info',
  Success: 'success',
  Warning: 'warning',
  Error: 'error',
})

class RunemarkConsole {
  #native

  constructor(options = {}) {
    this.#native = new native.NativeConsole(options)
  }

  render(tone, message) {
    return this.#native.paint(tone, message)
  }

  renderVerdict(verdict, message) {
    return this.#native.renderVerdict(verdict, message)
  }

  title(message) { return this.render(Tone.Title, message) }
  muted(message) { return this.render(Tone.Muted, message) }
  info(message) { return this.render(Tone.Info, message) }
  success(message) { return this.render(Tone.Success, message) }
  warning(message) { return this.render(Tone.Warning, message) }
  error(message) { return this.render(Tone.Error, message) }

  write(tone, message, stream = process.stdout) {
    stream.write(`${this.render(tone, message)}\n`)
  }

  writeInfo(message, stream) { this.write(Tone.Info, message, stream) }
  writeSuccess(message, stream) { this.write(Tone.Success, message, stream) }
  writeWarning(message, stream) { this.write(Tone.Warning, message, stream) }
  writeError(message, stream = process.stderr) { this.write(Tone.Error, message, stream) }
}

class RunemarkReport {
  #native

  constructor(input) {
    this.#native = new native.NativeReport(input)
  }

  render(options = {}) {
    return this.#native.render(options)
  }
}

class RunemarkProgress {
  #native

  constructor(options = {}) {
    this.#native = new native.NativeProgress(options)
  }

  start(total, message) { this.#native.start(total, message) }
  advance(position, message) { this.#native.advance(position, message) }
  notice(tone, message) { this.#native.notice(tone, message) }
  finish(verdict, message) { this.#native.finish(verdict, message) }
}

function renderStatus(verdict, message, options = {}) {
  return new RunemarkConsole(options).renderVerdict(verdict, message)
}

function renderReport(input, options = {}) {
  return new RunemarkReport(input).render(options)
}

function renderError(input, options = {}) {
  return native.renderError(input, options)
}

function renderDiff(input, options = {}) {
  return native.renderDiff(input, options)
}

function createProgress(options = {}) {
  return new RunemarkProgress(options)
}

module.exports = {
  RunemarkConsole,
  RunemarkProgress,
  RunemarkReport,
  Tone,
  Verdict,
  createProgress,
  renderDiff,
  renderError,
  renderReport,
  renderStatus,
}
