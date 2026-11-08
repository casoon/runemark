import assert from 'node:assert/strict'
import { execSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const pkgDir = path.resolve(__dirname, '..')

const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'runemark-consumer-'))

try {
  // 1. Pack package into temporary directory
  // Pack straight into tmpDir: renaming across drives fails on Windows (EXDEV).
  const packOutput = execSync(`npm pack --ignore-scripts --pack-destination "${tmpDir}"`, {
    cwd: pkgDir,
    encoding: 'utf8',
  }).trim()

  const tarballName = packOutput.split('\n').pop().trim()
  const destTarball = path.join(tmpDir, tarballName)

  // 2. Initialize consumer project
  execSync('npm init -y', { cwd: tmpDir, stdio: 'pipe' })

  // 3. Install packed tarball
  execSync(`npm install "${destTarball}"`, { cwd: tmpDir, stdio: 'pipe' })

  // 4. Test consumer script importing only via public package name '@casoon/runemark'
  const consumerScript = `
    const assert = require('node:assert/strict');
    const { RunemarkConsole, RunemarkReport, Verdict, renderStatus } = require('@casoon/runemark');

    const rmConsole = new RunemarkConsole({ color: 'never', isTerminal: false });
    assert.equal(rmConsole.success('Installed ok'), 'Installed ok');

    const status = renderStatus(Verdict.Passed, 'Ready', { color: 'never', isTerminal: false });
    assert.match(status, /\\[OK\\] Ready/);

    const report = new RunemarkReport({
      schemaVersion: 1,
      title: 'Consumer test',
      verdict: Verdict.Passed,
    });
    const output = report.render({ color: 'never', isTerminal: false });
    assert.match(output, /\\[OK\\] Consumer test/);
    process.stdout.write('Consumer contract validation passed.\\n');
  `
  fs.writeFileSync(path.join(tmpDir, 'consumer.cjs'), consumerScript)

  const testOutput = execSync('node consumer.cjs', {
    cwd: tmpDir,
    encoding: 'utf8',
  })
  assert.match(testOutput, /Consumer contract validation passed/)

  // 5. The same through ESM named imports, and the private loader stays closed.
  const esmScript = `
    import assert from 'node:assert/strict';
    import { createRequire } from 'node:module';
    import { RunemarkConsole, RunemarkReport, Verdict, renderStatus } from '@casoon/runemark';

    const rmConsole = new RunemarkConsole({ color: 'never', isTerminal: false });
    assert.equal(rmConsole.success('Installed ok'), 'Installed ok');
    assert.match(renderStatus(Verdict.Passed, 'Ready', { color: 'never', isTerminal: false }), /\\[OK\\] Ready/);
    const report = new RunemarkReport({ schemaVersion: 1, title: 'ESM consumer test', verdict: Verdict.Passed });
    assert.match(report.render({ color: 'never', isTerminal: false }), /\\[OK\\] ESM consumer test/);

    const require = createRequire(import.meta.url);
    assert.throws(() => require('@casoon/runemark/native.cjs'), { code: 'ERR_PACKAGE_PATH_NOT_EXPORTED' });
    process.stdout.write('ESM consumer validation passed.\\n');
  `
  fs.writeFileSync(path.join(tmpDir, 'consumer.mjs'), esmScript)

  const esmOutput = execSync('node consumer.mjs', {
    cwd: tmpDir,
    encoding: 'utf8',
  })
  assert.match(esmOutput, /ESM consumer validation passed/)
} finally {
  fs.rmSync(tmpDir, { recursive: true, force: true })
}
