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
shape in 0.3. New optional fields may be added without changing the schema
version. Renaming, changing the meaning of, or removing a field requires a new
schema version and a new major npm package version. Rust enums, public fields,
and builder implementation details are intentionally not exported.

String literals in `index.d.ts` are the Node contract. They are validated by
the native binding, so invalid tones, verdicts, locations, and file actions
fail at the package boundary rather than rendering a partial report.

## Compatibility policy

- The package requires Node.js 20 or newer and targets Node-API 8.
- The binding crate has its own Rust 1.88 build requirement; the `runemark`
  core crate retains its Rust 1.85 minimum supported version.
- Native binaries are distributed as exact-version optional platform packages.
  The root package is not published until every configured target is built and
  runtime-tested in CI.
- The generated native loader is private. Consumers import only
  `@casoon/runemark`, never `native.cjs` or a platform package.

## Release rule

NPM release is a separate production workflow. It must build every configured
target, run the package's Node tests against the generated artifact, publish
all platform packages, then publish the root package. npm publication is never
performed from a local development machine.
