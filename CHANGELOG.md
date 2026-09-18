# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.4.0] - 2026-09-18

### Added

- `select` feature: a grouped, keyboard-driven menu (`Menu`, `Group`, `Item`,
  `Hint`, `Outcome`, `SelectMode`). This is the first API in the crate that
  reads from the terminal; the documented design boundary is updated
  accordingly. It drives termios directly, so the only dependency is `libc`
  and the interactive path is Unix-only — elsewhere `Menu::run` reports
  `Outcome::Unavailable`, which is the same fallback a pipeline takes.
- `Menu::render` produces the same layout as plain text and needs no feature,
  so a non-interactive caller has something to show.
- `SelectMode` follows `ColorMode` and `ProgressMode`: `Auto` is interactive
  only for a terminal, and a menu that cannot be interactive returns
  `Outcome::Unavailable` without blocking on a read.

## [0.3.3] - 2026-09-14

### Fixed

- `TerminalProgress::stderr` no longer selects the `indicatif` bar where
  `indicatif` hides its output (`TERM` unset or `dumb`); every notice was
  silently dropped there. Such terminals now get plain lifecycle lines.

### Added

- `TerminalProgress::is_interactive` reports whether a live progress bar is rendered.

## [0.3.2] - 2026-09-14

### Fixed

- Narrow report layouts now wrap a finding's rule ID, confidence and location
  onto continuation lines with the hanging indent instead of overflowing the
  given width.
- The interactive progress bar no longer flashes one frame in the default
  `indicatif` style when an operation starts.

### Changed

- Update the optional `indicatif` dependency to 0.18 (drops the unmaintained
  `number_prefix` crate).

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
