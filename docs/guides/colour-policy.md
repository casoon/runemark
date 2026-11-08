---
title: Colour policy
description: When runemark emits styles, how NO_COLOR is honoured, and how to get deterministic output for tests.
order: 1
---

## Colour modes

| Mode | Rust | Node | Behaviour |
| --- | --- | --- | --- |
| Auto | `ColorMode::Auto` | `color: 'auto'` (default) | Styles only when writing to an interactive terminal, `NO_COLOR` is unset or empty, and `TERM` is not `dumb` |
| Always | `ColorMode::Always` | `color: 'always'` | Styles regardless of the destination |
| Never | `ColorMode::Never` | `color: 'never'` | Plain text – for tests, snapshots and logs |

`NO_COLOR` follows the [no-color.org](https://no-color.org/) convention: when it is set to a
non-empty value, `Auto` emits no styles. An empty `NO_COLOR` counts as unset. `TERM=dumb` also turns
`Auto` off. `Always` and `Never` ignore both variables.

runemark does not read `FORCE_COLOR` or `CLICOLOR_FORCE`, on purpose: the application decides, for
example by mapping its own `--color` flag or environment variable to a mode.

## Which stream

In Rust, create the console for the stream you write to, so `Auto` checks the right terminal:

```rust
use runemark::{ColorMode, Console};

let out = Console::stdout(ColorMode::Auto); // results
let err = Console::stderr(ColorMode::Auto); // progress and diagnostics
```

In Node, rendering returns strings and your code writes them. `auto` checks stdout unless you pass
`stream: 'stderr'` for text you write with `process.stderr.write`. `isTerminal` overrides the
detection altogether when you know better, for example in a test harness:

```js
const { RunemarkConsole, renderError } = require('@casoon/runemark')

// Treat the destination as not a terminal, for example in a test harness
const plain = new RunemarkConsole({ color: 'auto', isTerminal: false })

// Text written to stderr is checked against stderr
process.stderr.write(renderError({ heading: 'Build failed' }, { stream: 'stderr' }))
```

## Symbols

Symbols follow the colour decision. Where colour is on, status symbols are Unicode (`✔`, `⚠`, `✖`);
where it is off (`Never`, redirected output, `NO_COLOR`), they are ASCII markers (`[OK]`, `[WARN]`,
`[FAIL]`, `[INFO]`, `[ACT]`, `[SKIP]`), so the outcome survives without colour.

Override the choice with `Console::with_theme` and a `SymbolTheme` in Rust, or `symbols: 'unicode'`
or `symbols: 'ascii'` in Node:

```rust
use runemark::{ColorMode, Console, SymbolTheme, Verdict};

let plain = Console::new(ColorMode::Never, false);
assert_eq!(Verdict::Passed.render(plain, "Done"), "[OK] Done");

let unicode = plain.with_theme(SymbolTheme::Unicode);
assert_eq!(Verdict::Passed.render(unicode, "Done"), "✔ Done");
```

## Hyperlinks

OSC 8 links for locations follow the colour decision too: they appear exactly where styling does.
`Console::with_hyperlinks(bool)` in Rust and `hyperlinks: true | false` in Node decide
independently of colour, for example links without colour, or colour without links. See
[Security](../security/) for what a link target contains.

## Width

runemark never queries the terminal itself. Pass a known width and reports adapt: narrow layouts
stack summary metrics and wrap finding messages with a hanging indent.

```rust
use runemark::{ColorMode, Console, RenderOptions, Report, Verdict};

let report = Report::new("Site audit", Verdict::Info);
let output = report.render_with_options(
    Console::stdout(ColorMode::Auto),
    RenderOptions::new().with_width(80),
);
```

```js
report.render({ width: process.stdout.columns })
```

## Deterministic output

With `Never`, identical input produces identical text: no escape sequences, no terminal-dependent
behaviour. That is the mode for golden tests and snapshots.
