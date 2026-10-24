# Changelog

All notable changes to runemark are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and runemark follows
[Semantic Versioning](https://semver.org/); below `1.0.0` a minor release may change
existing behaviour, and such entries are marked `Breaking`.

## [Unreleased]

### Changed

- **Docs:** the Node contract is documented in one place instead of two.
  `NODE_API.md` is gone and its content lives in the project documentation;
  `SECURITY.md` keeps the policy and links there for the escape and trust model.
  Details:
  [Node contract](https://casoon.github.io/runemark/docs/guides/node-contract/).
- **Contributors:** `cargo doc` with default features and the Node binding's own
  test suite pass again. A doc link to `Menu::run_multi`, which only exists with
  the `select` feature, and a `"pass"` verdict literal where the contract says
  `"passed"` had both been failing CI.

## [0.9.0] - 2026-09-21

### Added

- **Select:** `Menu::run_multi` picks any number of entries — space ticks the one under
  the cursor, enter confirms, and `Picked::Chosen` carries the ticked ids in display
  order. Hints are neither shown nor answered there, since a key that ended the menu
  would drop the ticks.
- **Select:** `Menu::with_ticked` starts named entries ticked. Ticks are kept by id, so
  they survive the filter and switching tabs.

## [0.8.1] - 2026-09-20

### Changed

- **Crate:** no library changes from `0.8.0`. The version exists so the npm package,
  which shares the crate's version, can carry the Node fix below.

### Fixed

- **Node:** a metric's verdict is passed through as `MetricInput.verdict`.
  `Metric::with_verdict` landed in `0.6.0` and was never wired up, so Node callers had
  only a tone, which is nothing in a pipe or under `NO_COLOR`. It is applied before an
  explicit `tone`, which still wins. Details:
  [Metrics as outcomes](https://casoon.github.io/runemark/docs/guides/reports/#metrics-as-outcomes).

## [0.8.0] - 2026-09-20

### Breaking

- **Select:** `Group` carries two more public fields, `tab` and `divider`, so a literal
  construction of it no longer compiles. `Group::new` and the builders are unaffected.

### Added

- **Select:** `Group::in_tab` puts a run of groups in one tab rather than one each, and
  `Group::with_divider` marks where the row stops being one kind of thing. Inside a
  shared tab each group keeps its label as a heading, so only the row gets its length
  back. Details:
  [Groups that share a tab](https://casoon.github.io/runemark/docs/guides/selection/#groups-that-share-a-tab).

## [0.7.1] - 2026-09-20

### Fixed

- **Crate:** `0.7.0` did not compile on Rust `1.85`, the version this crate declares. The
  tab digits were matched with a `let` chain, which is stable only from `1.88`; the local
  toolchain accepted it and the declared minimum was never tried. The same key is now read
  without one, and a digit no tab carries is swallowed rather than falling through to a
  hint bound to it.
- **Node:** the binding required `runemark ^0.6.0` while the crate beside it was `0.7.0`,
  so every workflow that touches `bindings/node` failed to resolve. Its package version
  was bumped for the release, its dependency on the crate was not.

## [0.7.0] - 2026-09-19

### Added

- **Select:** `Layout::Tabs` arranges a menu's groups as a row of tabs, showing only the
  active group's entries, for a list that would otherwise be taller than the terminal.
  `←` `→`, `Tab` / `Shift-Tab` and the digits `1`–`9` switch groups. The application
  decides when to ask for the layout. Details:
  [Groups as tabs](https://casoon.github.io/runemark/docs/guides/selection/#groups-as-tabs).
- **Select:** `Menu::with_summary` adds a second heading line, for what the menu adds up
  to — how many entries, how many groups — which the list itself only says by being
  counted.

### Changed

- **Select:** `Menu::render` ignores `Layout::Tabs` and lists every group. Nothing on the
  other end of a pipe can press a key to reach the second tab, so hiding one there would
  lose entries rather than save lines.
- **Select:** the search spans every group in either layout, and leaves the tab row while
  it runs. Tabs answer "I know roughly where"; the filter answers "I know exactly what".
  Details:
  [Filtering](https://casoon.github.io/runemark/docs/guides/selection/#filtering).

## [0.6.0] - 2026-09-19

### Added

- **Reports:** `Metric::with_verdict` renders the verdict's symbol in front of a metric,
  and supplies its tone where none was set. A tone alone disappears with colour: piped or
  under `NO_COLOR`, a failing metric read exactly like a passing one. An explicit
  `with_tone` still wins. Details:
  [Metrics as outcomes](https://casoon.github.io/runemark/docs/guides/reports/#metrics-as-outcomes).

## [0.5.2] - 2026-09-19

### Fixed

- **Select:** a short menu no longer advertises `/ search` in its footer. Filtering still
  works — the key is never taken away — but a two-entry yes/no question offering to search
  itself reads as clutter, and below a handful of entries every one of them is already on
  screen.

## [0.5.1] - 2026-09-19

### Fixed

- **Select:** an interactive menu now shows `/ search` in its footer. `/` is reserved for
  the filter and cannot be bound as a hint, so nothing else could advertise it — a key the
  menu answers to but never mentions is a key nobody presses. `Menu::render` still offers
  no keys: printed to a pipe there is no keyboard.

## [0.5.0] - 2026-09-19

### Added

- **Select:** `/` filters a menu as you type. Groups with nothing left disappear, the
  cursor sits on the best match, and `Backspace` widens the query again. Matching is
  tiered: the name as a substring, then as a subsequence (`dpl` finds `deploy`), then the
  description. Details:
  [Filtering](https://casoon.github.io/runemark/docs/guides/selection/#filtering).
- **Select:** `Esc` leaves the filter before it leaves the menu, so a mistyped query costs
  one key rather than the whole selection. While filtering, every printable key is part of
  the query, so a menu binding `q` as a hint can still be searched for `quality`.

### Changed

- **Select:** `/` is reserved and can no longer be bound as a hint key.

## [0.4.2] - 2026-09-19

### Fixed

- **Select:** a menu entry longer than the terminal is wide wrapped to column zero, which
  destroyed the two-column layout. Entries are now shortened to fit, with `…` marking the
  cut; wrapping would change how many lines the frame occupies, which the redraw depends
  on. Details:
  [Terminals smaller than the menu](https://casoon.github.io/runemark/docs/guides/selection/#terminals-smaller-than-the-menu).
- **Select:** where the label column leaves too little room for a description to say
  anything, the description is dropped rather than cut to a stub.

## [0.4.1] - 2026-09-18

### Fixed

- **Select:** a menu taller than the terminal corrupted the display: the redraw moved up
  by the full frame height, which a short terminal clamps, so each frame ate the lines
  above. The body is now windowed, and `↑ N more` / `↓ N more` mark what is off screen.
  Details:
  [Terminals smaller than the menu](https://casoon.github.io/runemark/docs/guides/selection/#terminals-smaller-than-the-menu).
- **Select:** the redraw now moves up by the number of lines actually written rather than
  a separately computed height, which is what let the two drift apart.

## [0.4.0] - 2026-09-18

### Added

- **Select:** the `select` feature adds a grouped, keyboard-driven menu (`Menu`, `Group`,
  `Item`, `Hint`, `Outcome`, `SelectMode`) — the first API in the crate that reads from
  the terminal. It drives termios directly, so the only dependency is `libc` and the
  interactive path is Unix-only. Details:
  [Selection](https://casoon.github.io/runemark/docs/guides/selection/).
- **Select:** `Menu::render` produces the same layout as plain text and needs no feature,
  so a non-interactive caller has something to show.
- **Select:** `SelectMode` follows `ColorMode` and `ProgressMode`: `Auto` is interactive
  only for a terminal, and a menu that cannot be interactive returns `Outcome::Unavailable`
  without blocking on a read. Details:
  [Without a terminal](https://casoon.github.io/runemark/docs/guides/selection/#without-a-terminal).

## [0.3.3] - 2026-09-14

### Added

- **Progress:** `TerminalProgress::is_interactive` reports whether a live progress bar is
  rendered.

### Fixed

- **Progress:** `TerminalProgress::stderr` no longer selects the `indicatif` bar where
  `indicatif` hides its output (`TERM` unset or `dumb`); every notice was silently dropped
  there. Such terminals now get plain lifecycle lines.

## [0.3.2] - 2026-09-14

### Changed

- **Progress:** the optional `indicatif` dependency moves to 0.18, which drops the
  unmaintained `number_prefix` crate.

### Fixed

- **Reports:** narrow layouts now wrap a finding's rule id, confidence and location onto
  continuation lines with the hanging indent instead of overflowing the given width.
- **Progress:** the interactive bar no longer flashes one frame in the default `indicatif`
  style when an operation starts.

## [0.3.1] - 2026-09-12

### Added

- **Node:** native Node.js bindings and the `@casoon/runemark` TypeScript package, with a
  versioned, object-oriented API that accepts plain data objects while keeping Rust
  presentation models internal. Details:
  [Node contract](https://casoon.github.io/runemark/docs/guides/node-contract/).

### Changed

- **Contributors:** plain-text golden rendering tests and TypeScript contract test
  fixtures.

### Fixed

- **npm:** the package now ships native binaries for all six supported platforms.
  `@casoon/runemark@0.3.0` was an incomplete npm-only release that contained only the
  macOS arm64 binary; there is no 0.3.0 crate release. Details:
  [Node platforms](https://casoon.github.io/runemark/docs/getting-started/installation/#node-platforms).

### Security

- **Console:** terminal escape sequences in application text are sanitised before they are
  written. Details:
  [Security](https://casoon.github.io/runemark/docs/guides/security/).

## [0.2.0] - 2026-07-30

### Breaking

- **API:** public presentation types are now non-exhaustive, so runemark can extend its
  semantic vocabulary without a breaking release. Downstream exhaustive matches and struct
  literals no longer compile.

### Changed

- **Reports:** report internals are split into model and renderer modules while preserving
  the public `runemark::report` API.

### Fixed

- **Reports:** detailed reports now explicitly identify findings that were not provided in
  the report data instead of implying a complete list.

## [0.1.1] - 2026-07-29

### Fixed

- **Reports:** rendering now accepts trait-object writers without an intermediate buffer.

## [0.1.0] - 2026-07-29

### Added

- **Console:** initial `Console`, `ColorMode` and semantic `Tone` API, with a TTY-aware
  colour policy that honours `NO_COLOR`. Details:
  [Colour policy](https://casoon.github.io/runemark/docs/guides/colour-policy/).
