import assert from 'node:assert/strict'
import { execSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const rootDir = path.resolve(__dirname, '..')

// 1. Read Cargo.toml
const rootCargoToml = fs.readFileSync(path.join(rootDir, 'Cargo.toml'), 'utf8')
const rootVersionMatch = rootCargoToml.match(/name\s*=\s*"runemark"[\s\S]*?version\s*=\s*"([^"]+)"/)
assert.ok(rootVersionMatch, 'Root Cargo.toml must contain a package version')
const rootVersion = rootVersionMatch[1]

// 2. Read bindings/node/Cargo.toml
const nodeCargoToml = fs.readFileSync(path.join(rootDir, 'bindings/node/Cargo.toml'), 'utf8')
const nodeVersionMatch = nodeCargoToml.match(/name\s*=\s*"runemark-node"[\s\S]*?version\s*=\s*"([^"]+)"/)
assert.ok(nodeVersionMatch, 'bindings/node/Cargo.toml must contain a package version')
const nodeVersion = nodeVersionMatch[1]

// The binding's own dependency on the crate beside it. Bumping the package
// version and leaving this behind resolves against crates.io instead of the
// checkout, and every workflow touching bindings/node fails to select a
// version. That shipped once, in 0.7.0.
const nodeDepMatch = nodeCargoToml.match(/^runemark\s*=\s*\{[^}]*?version\s*=\s*"([^"]+)"/m)
assert.ok(nodeDepMatch, 'bindings/node/Cargo.toml must depend on runemark with a version')
const nodeDepVersion = nodeDepMatch[1]

// 3. Read packages/runemark/package.json
const pkgJson = JSON.parse(fs.readFileSync(path.join(rootDir, 'packages/runemark/package.json'), 'utf8'))
const pkgVersion = pkgJson.version

console.log(`Checking version parity:`)
console.log(`  Root Cargo.toml:              ${rootVersion}`)
console.log(`  bindings/node/Cargo.toml:     ${nodeVersion}`)
console.log(`  bindings/node runemark dep:   ${nodeDepVersion}`)
console.log(`  packages/runemark/package.json: ${pkgVersion}`)

assert.equal(nodeVersion, rootVersion, 'bindings/node version must match root crate version')
assert.equal(
  nodeDepVersion,
  rootVersion,
  "bindings/node's runemark dependency must match root crate version",
)
assert.equal(pkgVersion, rootVersion, 'packages/runemark version must match root crate version')

// 4. Validate CHANGELOG.md vs git tags
const changelog = fs.readFileSync(path.join(rootDir, 'CHANGELOG.md'), 'utf8')
const releaseHeaderRegex = /^## \[(\d+\.\d+\.\d+)\] - (\d{4}-\d{2}-\d{2})/gm

let match
const existingTags = execSync('git tag -l', { encoding: 'utf8' })
  .split('\n')
  .map((t) => t.trim())
  .filter(Boolean)

while ((match = releaseHeaderRegex.exec(changelog)) !== null) {
  const [, ver, date] = match
  // The current version's dated entry is the release being prepared; its tag
  // is created afterwards and verified in tag mode below.
  if (ver === rootVersion) continue
  const expectedTag = `v${ver}`
  if (!existingTags.includes(expectedTag)) {
    throw new Error(
      `CHANGELOG.md specifies release date ${date} for version [${ver}], but git tag '${expectedTag}' does not exist in the repository.`
    )
  }
}

if (process.env.GITHUB_REF_TYPE === 'tag') {
  const currentTag = process.env.GITHUB_REF_NAME
  const expectedTag = `v${rootVersion}`
  assert.equal(currentTag, expectedTag, `release tag must match package version ${rootVersion}`)
  assert.match(
    changelog,
    new RegExp(`^## \\[${rootVersion.replaceAll('.', '\\.')}\\] - \\d{4}-\\d{2}-\\d{2}$`, 'm'),
    `CHANGELOG.md must contain a dated [${rootVersion}] release before publishing`,
  )
}

console.log('Release metadata check passed successfully.')
