# Node.js API contract

`@casoon/runemark` is the supported Node.js facade for the Rust renderer. It
is a native Node-API addon built with napi-rs; consumers do not interact with
Rust types or builders directly.

## Stable boundary

The public API consists of the classes `RunemarkConsole`, `RunemarkReport`,
and `RunemarkProgress`, plus the stateless render helpers. Each class owns one
focused responsibility: semantic text, an immutable report, or a progress
lifecycle. Rendering returns strings; JavaScript owns writing those strings to
its streams, which keeps tests, build tools, and log capture independent of
global output state.

`ReportInput` is a versioned plain-object DTO. Version `1` is the only accepted
shape so far. New optional fields may be added without changing the schema
version — `MetricInput.verdict` arrived that way in 0.8.1. Renaming, changing the meaning of, or removing a field requires a new
schema version and a new major npm package version. Rust enums, public fields,
and builder implementation details are intentionally not exported.

String literals in `index.d.ts` are the Node contract. They are validated by
the native binding, so invalid tones, verdicts, locations, and file actions
fail at the package boundary rather than rendering a partial report.

## Compatibility policy

- The package requires Node.js 20 or newer and targets Node-API 8.
- The binding crate has its own Rust 1.88 build requirement; the `runemark`
  core crate retains its Rust 1.85 minimum supported version.
- The package bundles a native binary for every target in
  `packages/runemark/scripts/release-binaries.mjs`; the loader picks the one
  for the current platform. That file is the list, not this sentence.
- The generated native loader is private. Consumers import only
  `@casoon/runemark`, never `native.cjs` or a binary file.

## Release rule

The Release workflow builds every configured target, runtime-tests each binary,
verifies the crate version is on crates.io, and attaches the binaries with
`SHA256SUMS` to the GitHub release. It publishes nothing itself: `cargo
publish` is run from a maintainer machine so the registry token stays there.
The npm package is then published the same way
(`npm run fetch:binaries && npm publish`); `prepublishOnly` refuses to publish
unless all six binaries match the release checksums and the host binary passes
the smoke test. See [CONTRIBUTING.md](CONTRIBUTING.md#releasing).
