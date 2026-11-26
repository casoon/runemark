---
title: Tables and details
description: Aligned tables for flat listings, key-value headers and horizontal rules for detail views.
order: 4
---

A report is for a verdict with findings. Two lighter shapes cover the rest of what a command
prints: a flat **list** and a **detail view**, a few facts about one thing followed by its text.
Both are part of the Rust crate; the Node package does not expose them.

## Tables

A `Table` aligns rows into columns. Each column is as wide as its widest cell, measured with
`unicode-width`, so CJK text and emoji line up.

```rust
use runemark::{Align, Badge, Cell, Column, ColorMode, Console, Table, Tone};

let table = Table::new()
    .add_column(Column::new().with_title("Mails").with_align(Align::Right))
    .add_column(Column::new().with_title("Folder"))
    .add_row([Cell::new("128"), Cell::new("Inbox")])
    .add_row([
        Cell::new("7"),
        Cell::new("Archive").with_badge(Badge::new("locked", Tone::Warning)),
    ]);

let console = Console::new(ColorMode::Never, false);
assert_eq!(
    table.render(console),
    "Mails  Folder\n  128  Inbox\n    7  Archive [locked]\n"
);
```

- **Columns are optional.** A table of plain text needs no `Column`: `Table::new().add_row(["a", "b"])`
  is left-aligned with no header. `Column` adds a title, shown muted in a header row once any
  column has one, and `Align::Right` for numbers. A row longer than the declared columns adds
  left-aligned columns.
- **Cells carry a tone and a badge.** `Cell::with_tone` paints the text, `Cell::with_badge` appends
  a `[label]` after it. Without colour the tone is gone, but the badge keeps its brackets and
  label, so `[locked]` still reads in a pipe.
- **No trailing spaces.** A line ends at its last visible cell, so plain output compares cleanly
  in golden tests; see [Testing](../testing/).
- **Cells are one line.** Control characters are neutralised as everywhere else, and a newline or
  tab becomes a space.
- **No truncation, no wrapping.** The table never decides to drop text. A tool that has to fit a
  width shortens its values before it adds them.

`Table::write_to` streams to any writer, like the other renderers.

## Key-value headers

A detail view usually starts with a few facts. `KeyValues` prints them one per line, the keys
muted and the values aligned:

```rust
use runemark::{ColorMode, Console, KeyValues, Tone};

let header = KeyValues::new()
    .add("Title", "Quarterly plan")
    .add("Account", "work")
    .add_toned("State", Tone::Success, "approved");

let console = Console::new(ColorMode::Never, false);
assert_eq!(
    header.render(console),
    "Title:   Quarterly plan\nAccount: work\nState:   approved\n"
);
```

Every line is `Key: value`, so the pairs stay readable without colour. Keys and values are one
line, as in a table cell.

## Rules

`Console::rule(width)` returns a horizontal rule of that many columns, muted, with no trailing
newline. It is `─` for the Unicode symbol theme and `-` for ASCII, so it follows
`Console::with_theme` and the colour decision like every other symbol. The width is the host's to
give: the terminal's columns, or a cap such as 72.

Together, the three make the header-rule-body shape:

```rust
use runemark::{ColorMode, Console, KeyValues};

let console = Console::new(ColorMode::Never, false);
let header = KeyValues::new().add("Title", "Quarterly plan");

let view = format!("{}{}\nBody text.\n", header.render(console), console.rule(21));
assert_eq!(view, "Title: Quarterly plan\n---------------------\nBody text.\n");
```

`Console::write_rule` writes to a writer instead.
