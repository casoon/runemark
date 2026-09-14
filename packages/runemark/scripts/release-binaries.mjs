// Native binaries for the npm package are built by the Release workflow and
// attached to the GitHub release of the matching version tag.
//
//   node scripts/release-binaries.mjs fetch   download them into this package
//   node scripts/release-binaries.mjs verify  fail unless all of them are present
//                                             and match the release checksums
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const packageDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const { version } = JSON.parse(fs.readFileSync(path.join(packageDir, 'package.json'), 'utf8'))
const tag = `v${version}`
const checksumFile = 'SHA256SUMS'
const binaries = [
  'runemark.darwin-arm64.node',
  'runemark.win32-x64-msvc.node',
  'runemark.linux-x64-gnu.node',
  'runemark.linux-x64-musl.node',
  'runemark.linux-arm64-gnu.node',
]

function fail(message) {
  console.error(`release-binaries: ${message}`)
  process.exit(1)
}

function fetchBinaries() {
  execFileSync(
    'gh',
    [
      'release', 'download', tag,
      '--repo', 'casoon/runemark',
      '--dir', packageDir,
      '--clobber',
      '--pattern', 'runemark.*.node',
      '--pattern', checksumFile,
    ],
    { stdio: 'inherit' },
  )
}

function verifyBinaries() {
  const checksumPath = path.join(packageDir, checksumFile)
  if (!fs.existsSync(checksumPath)) {
    fail(`${checksumFile} is missing; run \`npm run fetch:binaries\` first`)
  }
  const expected = new Map(
    fs.readFileSync(checksumPath, 'utf8')
      .trim()
      .split('\n')
      .map((line) => {
        const [hash, name] = line.trim().split(/\s+/)
        return [name, hash]
      }),
  )

  const present = fs.readdirSync(packageDir).filter((name) => /^runemark\..+\.node$/.test(name))
  const unexpected = present.filter((name) => !binaries.includes(name))
  if (unexpected.length > 0) fail(`unexpected binaries would be published: ${unexpected.join(', ')}`)

  for (const name of binaries) {
    const file = path.join(packageDir, name)
    if (!fs.existsSync(file)) fail(`${name} is missing; run \`npm run fetch:binaries\``)
    const actual = createHash('sha256').update(fs.readFileSync(file)).digest('hex')
    if (expected.get(name) !== actual) {
      fail(`${name} does not match the ${tag} release checksum; run \`npm run fetch:binaries\``)
    }
  }

  if (!fs.existsSync(path.join(packageDir, 'native.cjs'))) {
    fail('native.cjs is missing; run `npm run build` first')
  }

  console.log(`All ${binaries.length} native binaries match the ${tag} release.`)
}

const command = process.argv[2]
if (command === 'fetch') fetchBinaries()
else if (command === 'verify') verifyBinaries()
else fail('usage: release-binaries.mjs fetch|verify')
