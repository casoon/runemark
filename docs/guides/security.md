---
title: Security
description: How runemark handles untrusted text – finding messages, file names and URLs – before it reaches a terminal.
order: 6
---

Findings often contain text the tool did not write itself: file names, page titles, URLs, messages
from other tools. runemark treats all of it as untrusted.

## Visible text

Control characters such as ESC (`\x1b`), BEL (`\x07`), DEL (`\x7f`) and other C0/C1 codes are
neutralised into caret notation (for example `^[`) or stripped before output. Newlines and tabs are
kept. Carriage returns are neutralised, because they can overwrite earlier terminal output.

## Hyperlinks

- URL locations become OSC 8 hyperlinks only for explicit `http://` and `https://` schemes.
  Other schemes (`javascript:`, `file:`, `data:` …) render as plain text.
- A hyperlink target that contains control characters, escape characters or embedded terminators
  (`\x1b\\`, `\x07`) is rejected and rendered without a link.

## Parity

The Rust crate and the Node package give the same guarantees.

## Reporting a vulnerability

See [SECURITY.md](https://github.com/casoon/runemark/blob/main/SECURITY.md) in the repository.
