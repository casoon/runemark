// Native binaries for the npm package are built by the Release workflow and
// attached to the GitHub release of the matching version tag.
//
//   node scripts/release-binaries.mjs fetch   download them into this package
//   node scripts/release-binaries.mjs verify  fail unless all of them are present
//                                             and match the release checksums and
//                                             their build provenance attestation
//
// The platform list is `napi.targets` in package.json; nothing here repeats it.
// verify also runs `gh attestation verify` for every binary. Set
// RUNEMARK_SKIP_ATTESTATION=1 to skip that for local runs (the release
// checksums are still compared).
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const packageDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')
const { version, napi } = JSON.parse(fs.readFileSync(path.join(packageDir, 'package.json'), 'utf8'))
const tag = `v${version}`
const checksumFile = 'SHA256SUMS'

const platforms = { darwin: 'darwin', windows: 'win32', linux: 'linux' }
const architectures = { x86_64: 'x64', aarch64: 'arm64' }

// File name napi gives a target's binary:
// aarch64-apple-darwin -> runemark.darwin-arm64.node
function binaryName(target) {
  const [arch, , system, abi] = target.split('-')
  if (!(arch in architectures) || !(system in platforms)) {
    throw new Error(`release-binaries: unsupported napi target ${target}`)
  }
  const suffix = abi ? `-${abi}` : ''
  return `${napi.binaryName}.${platforms[system]}-${architectures[arch]}${suffix}.node`
}

export const binaries = napi.targets.map(binaryName)

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

// Needs `gh` with a token (GH_TOKEN in CI). The attestation proves the file was
// built by this repository's workflow; the checksum alone only proves it matches
// SHA256SUMS, which is attached to the same release.
function verifyAttestation(name) {
  try {
    execFileSync(
      'gh',
      ['attestation', 'verify', path.join(packageDir, name), '--repo', 'casoon/runemark'],
      { stdio: 'pipe' },
    )
  } catch (error) {
    const detail = error.stderr ? `\n${error.stderr.toString().trim()}` : ''
    fail(`${name} has no valid build provenance attestation for casoon/runemark${detail}`)
  }
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
    if (!expected.has(name)) fail(`${name} is not listed in ${checksumFile}`)
    if (expected.get(name) !== actual) {
      fail(`${name} does not match the ${tag} release checksum; run \`npm run fetch:binaries\``)
    }
  }

  if (process.env.RUNEMARK_SKIP_ATTESTATION === '1') {
    console.warn('release-binaries: RUNEMARK_SKIP_ATTESTATION=1, build provenance attestations not checked')
  } else {
    for (const name of binaries) verifyAttestation(name)
  }

  if (!fs.existsSync(path.join(packageDir, 'native.cjs'))) {
    fail('native.cjs is missing; run `npm run build` first')
  }

  console.log(`All ${binaries.length} native binaries match the ${tag} release.`)
}

// Imported by scripts/check-release-metadata.mjs for `binaries`; only run the
// commands when invoked directly.
if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  const command = process.argv[2]
  if (command === 'fetch') fetchBinaries()
  else if (command === 'verify') verifyBinaries()
  else fail('usage: release-binaries.mjs fetch|verify')
}
