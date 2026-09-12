---
title: Progress
description: A progress lifecycle for long-running work that keeps redirected output clean.
order: 4
---

A progress run has four steps: `start` with a total, `advance` with the current position, optional
notices, and `finish` with a verdict.

## Rust

Enable the `progress` feature for the `indicatif`-backed terminal sink:

```sh
cargo add runemark --features progress
```

```rust
use runemark::{ColorMode, Console, ProgressMode, ProgressSink, TerminalProgress, Verdict};
use std::io::IsTerminal;

let progress = TerminalProgress::stderr(
    ProgressMode::Auto,
    Console::stderr(ColorMode::Auto),
    std::io::stderr().is_terminal(),
);
progress.start(10, "Auditing URLs");
progress.advance(1, "https://example.com");
progress.finish(Verdict::Passed, "Audit complete");
```

`ProgressMode::Auto` shows a live bar in an interactive terminal and switches to a compact,
line-oriented lifecycle when output is redirected, so stdout stays clean for machine-readable
results. Without the feature, `PlainProgress` and `SilentProgress` implement the same
`ProgressSink` trait.

## Node

```js
const { createProgress } = require('@casoon/runemark')

const progress = createProgress({ mode: 'auto' })
progress.start(pages.length, 'Checking pages')
pages.forEach((page, index) => progress.advance(index + 1, page))
progress.finish('passed', 'Audit complete')
```

Progress writes to stderr. In `auto` mode the terminal check uses stderr unless `isTerminal` is set.
