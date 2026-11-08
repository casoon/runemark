---
title: Security
description: How runemark handles untrusted text – finding messages, file names and URLs – before it reaches a terminal.
order: 8
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
- File locations become `file://` hyperlinks that the tool builds itself: the path is
  canonicalised (symlinks resolved; a relative path that does not exist is joined to the working
  directory), percent-encoded, and followed by `#line:col` when known. The link target is
  therefore the **absolute path**. The visible text is only the path as given, but anything that
  captures the raw output (a log with forced colour, a terminal recording) also captures the
  escape sequence and with it the local directory layout, user name included.
- Links follow the colour decision, so plain and redirected output carries none. Turn them off
  where raw output leaves the machine: `Console::with_hyperlinks(false)` in Rust,
  `hyperlinks: false` in Node.
- A hyperlink target that contains control characters, escape characters or embedded terminators
  (`\x1b\\`, `\x07`) is rejected and rendered without a link.

## Parity

The Rust crate and the Node package give the same guarantees.

## Reporting a vulnerability

See [SECURITY.md](https://github.com/casoon/runemark/blob/main/SECURITY.md) in the repository.
