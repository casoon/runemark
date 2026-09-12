---
title: Colour policy
description: When runemark emits styles, how NO_COLOR is honoured, and how to get deterministic output for tests.
order: 1
---

## Colour modes

| Mode | Rust | Node | Behaviour |
| --- | --- | --- | --- |
| Auto | `ColorMode::Auto` | `color: 'auto'` (default) | Styles only when writing to an interactive terminal and `NO_COLOR` is not set |
| Always | `ColorMode::Always` | `color: 'always'` | Styles regardless of the destination |
| Never | `ColorMode::Never` | `color: 'never'` | Plain text – for tests, snapshots and logs |

`NO_COLOR` follows the [no-color.org](https://no-color.org/) convention: when it is set, `Auto`
emits no styles.

## Which stream

In Rust, create the console for the stream you write to, so `Auto` checks the right terminal:

```rust
use runemark::{ColorMode, Console};

let out = Console::stdout(ColorMode::Auto); // results
let err = Console::stderr(ColorMode::Auto); // progress and diagnostics
```

In Node, rendering returns strings and your code writes them. `isTerminal` overrides the detection
when you know better, for example in a test harness:

```js
const { RunemarkConsole } = require('@casoon/runemark')

const console = new RunemarkConsole({ color: 'auto', isTerminal: false })
```

## Symbols

Status symbols use Unicode by default. Choose ASCII for terminals or logs that cannot display it:
`SymbolTheme` in Rust, `symbols: 'ascii'` in Node.

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
