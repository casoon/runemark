# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Fixed

- Narrow report layouts now wrap a finding's rule ID, confidence and location
  onto continuation lines with the hanging indent instead of overflowing the
  given width.

## [0.3.1] - 2026-09-12

### Added

- Native Node.js bindings and the `@casoon/runemark` TypeScript package.
- A versioned, object-oriented Node API that accepts plain data objects while
  keeping Rust presentation models internal.
- Full plain-text golden rendering tests and hardened terminal escape sanitization.
- Multi-platform packaging and TypeScript contract test fixtures.

### Fixed

- The npm package now ships native binaries for all six supported platforms.
  `@casoon/runemark@0.3.0` was an incomplete npm-only release that contained
  only the macOS arm64 binary; there is no 0.3.0 crate release.

## [0.2.0] - 2026-07-30

### Changed

- Public presentation types are now non-exhaustive so Runemark can extend its
  semantic vocabulary without a breaking release. This is a breaking change
  for downstream exhaustive matches and struct literals.
- Report internals are split into model and renderer modules while preserving
  the public `runemark::report` API.

### Fixed

- Detailed reports now explicitly identify findings that were not provided in
  the report data instead of implying a complete list.

## [0.1.1] - 2026-07-29

### Fixed

- Report rendering now accepts trait-object writers without an intermediate buffer.

## [0.1.0] - 2026-07-29

### Added

- Initial `Console`, `ColorMode`, and semantic `Tone` API.
- TTY-aware color policy with `NO_COLOR` support.
