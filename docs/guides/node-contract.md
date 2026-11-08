---
title: Node contract
description: What the npm package guarantees, how the report schema is versioned, and what stays internal.
order: 5
---

`@casoon/runemark` is a native Node-API addon around the Rust renderer, built with napi-rs.
Consumers work with plain objects and strings, never with Rust types or builders.

## Stable boundary

- Classes `RunemarkConsole` (semantic text), `RunemarkReport` (an immutable report) and
  `RunemarkProgress` (a progress lifecycle), plus the helpers `renderStatus`, `renderReport`,
  `renderError`, `renderDiff` and `createProgress`.
- Rendering returns strings; your code writes them. `write*` methods accept any object with a
  `write(text)` method.
- The string literals in `index.d.ts` are the contract. The native binding validates tones,
  verdicts, locations and file actions, so invalid input fails at the package boundary instead of
  rendering a partial report.
- `color: 'auto'` checks stdout unless `stream: 'stderr'` is passed on the console, render, error
  and diff options; an explicit `isTerminal` wins over both. Progress always uses stderr.
- `hyperlinks` switches OSC 8 links on or off independently of colour on the console, render, error
  and diff options. Unset, links follow the colour decision, as before.
- Numbers that map to Rust `u32` (`line`, `column`, `totalCount`, `maxCompactSamples`, `width`,
  progress `total` and `position`) must be integers from 0 to 4294967295. Anything else throws an
  error naming the field instead of wrapping. `location.column` without `location.line` throws.
- Defaults the Rust API makes you spell out: a finding's `tone` is `'warning'`, a badge's `'info'`.
- A metric `trend` is validated even without a `delta`: an unknown value throws, a valid one
  without a `delta` is accepted and renders nothing, as in the Rust core.
- Rust enums, public fields and builder internals are deliberately not exported.

## Parity with the Rust API

Every public item the `runemark` crate re-exports, and what Node does with it. Node gets what a
consumer needs to render a report, not parity for its own sake.

| Rust | Node | Status |
| --- | --- | --- |
| `ColorMode`, `Console`, `SymbolTheme`, `Tone` | `color`, `symbols`, `stream`, `isTerminal`, `RunemarkConsole`, `Tone` | Supported |
| `Console::with_hyperlinks`, `hyperlinks_enabled` | `hyperlinks` | Supported |
| `Verdict` | `Verdict`, `renderStatus`, `RunemarkConsole.renderVerdict` | Supported |
| `Report`, `RenderOptions` | `ReportInput`, `RunemarkReport`, `renderReport`, `width` | Supported |
| `Metric`, `Trend` | `MetricInput` (`verdict`, `tone`, `trend`, `delta`) | Supported |
| `FindingGroup`, `Finding`, `Confidence`, `Location` | `FindingGroupInput`, `FindingInput`, `confidence`, `LocationInput` | Supported |
| `Badge::new` | `BadgeInput` | Supported |
| `NextStep` | `NextStepInput` | Supported |
| `DetailLevel`, `Report::max_compact_samples` | `detailLevel`, `maxCompactSamples` | Supported |
| `ScopeNote`, `Report::add_scope_note` | `scopeNotes` | Supported |
| `Vocabulary`, `Report::add_vocabulary` | `vocabulary`, a tagged union on `kind` | Supported |
| `Report::top_issues_threshold` | `topIssuesThreshold` (`false` turns the summary off) | Supported |
| `ErrorBlock` | `renderError`, `ErrorBlockInput` | Supported |
| `DiffBlock`, `FileChange`, `FileAction` | `renderDiff`, `DiffInput` | Supported |
| `ProgressMode`, `TerminalProgress` | `RunemarkProgress`, `createProgress`, `mode` | Supported |
| `PlainProgress`, `SilentProgress` | `mode: 'always'` and `mode: 'never'` | Covered by `mode`, types not exported |
| `ProgressSink` | n/a | Not supported: a Rust trait for custom sinks, JavaScript has no counterpart |
| `TerminalProgress::is_interactive` | n/a | Not supported: Node passes `isTerminal` in and needs no answer back |
| `Badge::quick_win`, `gained`, `lost` | `{ label: 'QUICK WIN', tone: 'success' }` | Not supported: presets for fixed labels, spell the label and tone out |
| `format_osc8` | n/a | Not supported: low-level helper; reports emit links themselves, for allowed schemes only |
| `testing::strip_ansi` | `color: 'never'` | Not supported: ask for plain output instead of stripping styled output |
| `Menu`, `Item`, `Group`, `Hint`, `Layout`, `Outcome`, `Picked`, `SelectMode` | n/a | Not supported: Unix-only interactive terminal menu behind a Cargo feature, not rendering |
| `Console::stdout`, `Console::stderr` | `stream: 'stdout' \| 'stderr'` | Supported |

Nothing is marked planned. Open an issue if you need one of the unsupported items.

## Report schema versions

`ReportInput` carries `schemaVersion`. Version `1` is the only accepted shape. New optional fields
may be added without a new schema version — `MetricInput.verdict` arrived that way in 0.8.1, and
`scopeNotes`, `vocabulary` and `topIssuesThreshold` after it.
Renaming, changing the meaning of, or removing a field requires a new schema version and a new
major package version.

## Compatibility

- Node.js 20 or newer, Node-API 8.
- The native binding builds with Rust 1.88; the `runemark` crate keeps Rust 1.85 as its minimum.
- The package bundles one native binary per target listed in
  `packages/runemark/scripts/release-binaries.mjs`; the loader picks the one for the current
  platform. That script is the list, not this page.
- The native loader is private: import only `@casoon/runemark`. The `exports` map exposes
  nothing else, and the package works through `require` and through ESM named imports.
- Output matches the Rust crate for the same input and options.
