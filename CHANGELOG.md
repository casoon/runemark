# Changelog

All notable changes to this project are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/)
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.5.2] - 2026-09-19

### Fixed

- A short menu no longer advertises `/ search` in its footer. Filtering still
  works — the key is never taken away — but a two-entry yes/no question
  offering to search itself reads as clutter, and below a handful of entries
  every one of them is already on screen.

## [0.5.1] - 2026-09-19

### Fixed

- An interactive menu now shows `/ search` in its footer. `/` is reserved for
  the filter and cannot be bound as a hint, so nothing else could advertise it —
  a key the menu answers to but never mentions is a key nobody presses.
  `Menu::render` still offers no keys: printed to a pipe there is no keyboard.

## [0.5.0] - 2026-09-19

### Added

- `/` filters a menu as you type. Groups with nothing left disappear, the
  cursor sits on the best match, and `Backspace` widens the query again.
  Matching is tiered: the query as a substring of the name, then as a
  subsequence of it (`dpl` finds `deploy`), then as a substring of the
  description — a name the user is typing towards beats a description that
  happens to share letters.
- `Esc` leaves the filter before it leaves the menu, so a mistyped query costs
  one key rather than the whole selection. While filtering, every printable key
  is part of the query, so a menu binding `q` as a hint can still be searched
  for `quality`.
- `/` is reserved and can no longer be bound as a hint key.

An empty query restores the menu as it was rather than ranking everything equal
and sorting it alphabetically, which would undo the ordering the grouping
exists to provide.

## [0.4.2] - 2026-09-19

### Fixed

- A menu entry longer than the terminal is wide wrapped to column zero, which
  destroyed the two-column layout — a 78-character description behind a
  19-column label column produced 101-column lines. Entries are now shortened
  to fit, with `…` marking the cut. Wrapping is deliberately not used: it would
  change how many lines the frame occupies, which the redraw depends on.
- Where the label column leaves too little room for a description to say
  anything, the description is dropped rather than cut to a stub.

`Menu::render` is unchanged and neither shortened nor windowed.

## [0.4.1] - 2026-09-18

### Fixed

- A menu taller than the terminal corrupted the display. Redrawing moved the
  cursor up by the full frame height, which a short terminal clamps, so each
  frame ate the lines above it. The body is now windowed to what the terminal
  can show, with `↑ N more` and `↓ N more` marking what is out of view, and the
  window moves the least amount that keeps the cursor visible.
- The redraw now moves up by the number of lines actually written rather than a
  separately computed height, which is what let the two drift apart.

`Menu::render` is unchanged and never windowed: a pipe or a file has no height
to run out of.

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
