---
title: Testing
description: Golden-test your CLI's output with plain rendering, strip_ansi and assert_plain_snapshot!.
order: 6
---

Output that is identical for identical input is easy to test. Two things keep it that way: render
with colour off, and pin the layout width.

## Plain output

`ColorMode::Never` emits no escape sequences, uses ASCII status markers and, by default, no
hyperlinks, whatever the environment. That is the mode for golden tests; see [Colour policy](../colour-policy/).

```rust
use runemark::{ColorMode, Console, Verdict};

let console = Console::new(ColorMode::Never, false);
assert_eq!(Verdict::Failed.render(console, "2 errors"), "[FAIL] 2 errors");
```

Pass `RenderOptions::new().with_width(80)` to `render_with_options` so wrapping does not depend on
the machine running the test.

## Stripping styles

The `testing` module has two helpers for output that was rendered with colour on.

- `testing::strip_ansi(&str) -> String` removes colour and style sequences and OSC 8 hyperlinks
  (terminated by BEL or `ESC \`), and leaves the visible text.
- `assert_plain_snapshot!(actual, expected)` strips `actual`, trims both sides and compares them
  with `assert_eq!`.

Stripping is how you test the styled path, and the visible text keeps the Unicode symbols:

```rust
use runemark::testing::strip_ansi;
use runemark::{assert_plain_snapshot, ColorMode, Console, Verdict};

let styled = Console::new(ColorMode::Always, false);
let output = Verdict::Passed.render(styled, "Done");

assert_ne!(output, "✔ Done");
assert_eq!(strip_ansi(&output), "✔ Done");
assert_plain_snapshot!(output, "✔ Done");
```

Stripping also drops the `file://` link target of a file location, which holds an absolute path
that differs per machine (see [Security](../security/)). With `ColorMode::Never` there is no link
to begin with.

## Node

There is no stripping helper in Node. Ask for plain output instead, with `color: 'never'`:

```js
const assert = require('node:assert/strict')
const { renderStatus } = require('@casoon/runemark')

assert.equal(renderStatus('failed', '2 errors', { color: 'never' }), '[FAIL] 2 errors')
```

For reports, pass `width` as well so the layout is the same everywhere.
