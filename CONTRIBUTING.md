# Contributing to runemark

## Development setup

### Prerequisites

- Rust 1.85 or newer for the crate (the MSRV CI tests); the Node binding in
  `bindings/node` needs Rust 1.88
- Git
- Node 20 or newer with npm, only for the Node binding and `packages/runemark`
- pnpm 10, only to build the project site

### Build and test

```bash
git clone https://github.com/casoon/runemark
cd runemark
cargo test --all-features
```

Run the same checks as CI before opening a pull request:

```bash
scripts/verify.sh          # everything the Ubuntu CI jobs run
scripts/verify.sh --rust   # without the Node binding section
```

It runs, in this order and stopping at the first failure:

- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test --all-features`, plus the same on the MSRV 1.85 when that
  toolchain is installed (`rustup toolchain install 1.85`)
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`, with and without
  `--all-features`
- `cargo package --locked`
- `node scripts/check-release-metadata.mjs`
- the Node job: binding clippy and test, then in `packages/runemark`
  `npm ci`, `npm test`, `npm run test:types`, `npm run test:contract` and
  `npm run pack:check`

CI additionally runs the tests on macOS and Windows and the Node package on
Node 20 and 22; those are not covered locally.

### Documentation and project site

`docs/` holds the documentation pages and `site/` is the Astro project that
publishes them (with the showcase) to GitHub Pages. The showcase renders with
the native Node binding, so build that first:

```bash
(cd packages/runemark && npm ci && npm run build)
cd site
pnpm install --frozen-lockfile
pnpm dev        # live preview
pnpm build      # production build into site/dist
```

The page theme is the npm package `@casoon/pages-theme`, a dependency of
`site/`. Fix theme problems in its own repository, not here.

## API principles

runemark is a presentation library. Contributions should preserve these
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
5. Add an entry under `## [Unreleased]` in `CHANGELOG.md` for every user-facing
   change, in the same pull request.

## Releasing

1. Bump the version in `Cargo.toml`, `bindings/node/Cargo.toml` (package and
   `runemark` dependency) and `packages/runemark/package.json`, refresh
   `packages/runemark/package-lock.json` (`npm install --package-lock-only`
   there), and date the release in `CHANGELOG.md`.
   `node scripts/check-release-metadata.mjs` checks that they agree.
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
5. The last job of the Release workflow publishes the npm package through npm
   trusted publishing. It waits in the `npm` environment until you approve it in
   the GitHub UI; no npm token exists anywhere, and the package gets a
   provenance statement. It fetches the binaries from the release
   (`npm run fetch:binaries`) and runs `npm publish`; `prepublishOnly` refuses
   to publish unless every binary the package ships matches the release
   checksums and the host binary passes the smoke test.
   Which binaries those are is `napi.targets` in
   `packages/runemark/package.json`, not this list — it was six until x86_64
   macOS was dropped. `scripts/release-binaries.mjs` derives its list from it,
   and `scripts/check-release-metadata.mjs` checks the release build matrix
   against it.

## Reporting issues

Do not report vulnerabilities as issues; follow [SECURITY.md](SECURITY.md).

For bugs, include the runemark version, Rust version, operating system, terminal, color
mode, whether output was redirected, and a minimal reproduction. For rendering
issues, include the plain output as well as the ANSI output when possible.
