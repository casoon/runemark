# Contributing to Runemark

## Development setup

### Prerequisites

- Rust 1.85 or newer
- Git

### Build and test

```bash
git clone <repository-url>
cd runemark
cargo test --all-features
```

Run the same checks as CI before opening a pull request:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo package --locked
```

## API principles

Runemark is a presentation library. Contributions should preserve these
boundaries:

- Accept application-owned data; do not add audit, SEO, accessibility, or
  generator-specific domain types.
- Keep machine-readable formats outside the crate.
- Prefer a writer-first API over global output state or direct `println!`.
- Preserve TTY detection, `NO_COLOR`, and plain deterministic rendering.
- Add a dependency only when the capability cannot be a small adapter over an
  existing focused crate.

## Adding presentation features

1. Start with a concrete use case from at least one consuming CLI.
2. Add a public semantic type or renderer only when it generalizes beyond that
   application.
3. Cover both colorful and plain output with unit or snapshot-style tests.
4. Document the behavior and its non-goals in the README or API docs.
5. Avoid changing plain-text output accidentally; it is part of the testing
   and piping contract.

## Pull requests

1. Create a focused branch.
2. Keep the public API addition and its tests in the same change.
3. Explain the real consumer and why the existing API is insufficient on its
   own.
4. Use a concise conventional commit subject where practical, such as
   `feat: add grouped finding summary` or `fix: preserve plain output`.

## Releasing

1. Bump the version in `Cargo.toml`, `bindings/node/Cargo.toml` (package and
   `runemark` dependency) and `packages/runemark/package.json`, and date the
   release in `CHANGELOG.md`. `node scripts/check-release-metadata.mjs` checks
   that they agree.
2. After merging to `main`, optionally run `gh workflow run release.yml --ref main`
   to build and smoke-test all native binaries before tagging (dry run, nothing
   is published).
3. Run `cargo publish` from your machine. The registry token stays there rather
   than in repository secrets, and releasing is a deliberate step instead of a
   side effect of pushing a tag.
4. Push the tag `v<version>` from `main`. The Release workflow builds the
   native binaries, confirms the version is on crates.io, and attaches the
   binaries plus `SHA256SUMS` to the GitHub release. A tag pushed before
   publishing fails with that reason.
5. Publish the npm package the same way:

   ```bash
   cd packages/runemark
   npm ci
   npm run build
   npm run fetch:binaries
   npm publish
   ```

   `prepublishOnly` refuses to publish unless all six binaries match the
   release checksums and the host binary passes the smoke test.

## Reporting issues

Include the Runemark version, Rust version, operating system, terminal, color
mode, whether output was redirected, and a minimal reproduction. For rendering
issues, include the plain output as well as the ANSI output when possible.
