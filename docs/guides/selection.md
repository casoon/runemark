---
title: Selection
description: A grouped, keyboard-driven menu for tools that need a choice rather than a prompt.
order: 5
---

The optional `select` feature adds a grouped menu. It is the one place where runemark reads
from the terminal instead of only writing to it — a cursor has to react to keys.

It drives the terminal directly: termios for raw mode, four escape sequences for drawing, and
a small decoder for the six keys a menu needs. The only dependency is `libc`, and the
interactive path is **Unix-only**. On other platforms `run` returns `Outcome::Unavailable`,
the same result a pipeline gets, so a caller writes one code path with no `cfg`.

The boundary still holds in the direction that matters: a menu carries labels, descriptions
and hints, and nothing about what the entries mean. Grouping, ordering and wording stay with
the application.

```bash
cargo add runemark --features select
```

## Building a menu

```rust
use runemark::{Group, Hint, Item, Menu};

let menu = Menu::new()
    .with_heading("casoon.dev")
    .with_note("pnpm")
    .add_group(
        Group::new("Development")
            .add_item(Item::new("dev", "dev").with_description("Start the site"))
            .add_item(Item::new("dev:landings", "dev:landings")),
    )
    .add_group(Group::new("Build").add_item(Item::new("build", "build")))
    .add_hint(Hint::new('U', "Updates"));
```

`Item::id` is the application's own identifier and is what selection returns. The label is
what the user sees; descriptions form a second column that lines up across every group.

## Running it

```rust
# use runemark::{ColorMode, Console, Menu, Outcome, SelectMode};
# let menu = Menu::new();
use std::io::IsTerminal;

let outcome = menu.run(
    Console::stderr(ColorMode::Auto),
    SelectMode::Auto,
    std::io::stderr().is_terminal(),
)?;

match outcome {
    Outcome::Selected(id) => println!("chose {id}"),
    Outcome::Hotkey(key) => println!("pressed {key}"),
    Outcome::Cancelled => {}
    Outcome::Unavailable => print!("{}", menu.render(Console::stdout(ColorMode::Auto))),
}
# Ok::<(), std::io::Error>(())
```

The menu writes to stderr, so the caller's stdout stays clean for piping.

`is_terminal` is supplied by the caller rather than detected inside runemark, matching the
rest of the crate: the application owns the decision about its own streams.

## Keys

| Key | Effect |
| --- | --- |
| `↑` `↓` | Move the cursor, wrapping at both ends |
| `Enter` | Select, returning `Outcome::Selected` |
| A hint key | Returns `Outcome::Hotkey`, matched case-insensitively |
| `Esc`, `q` | `Outcome::Cancelled` |
| `Ctrl-C` | `Outcome::Cancelled` |

A hint key wins over the built-in `q`, so a menu is free to bind `q` itself.

## Terminals shorter than the menu

A menu taller than the terminal is windowed: the body scrolls, and `↑ N more` / `↓ N more`
mark what is out of view. The window moves the least amount that keeps the cursor visible, so
short cursor moves do not slide the whole screen.

The height comes from the terminal itself, re-read on every frame, so resizing the window
while the menu is open is picked up without a `SIGWINCH` handler. A terminal that reports no
height gets the whole menu.

`Menu::render` is never windowed. A pipe or a file has no height to run out of, and
truncating there would drop entries for no reason.

## Without a terminal

`SelectMode` follows the same shape as `ColorMode` and `ProgressMode`:

| Mode | Behaviour |
| --- | --- |
| `Auto` | Interactive only for a terminal |
| `Always` | Interactive regardless |
| `Never` | Never interactive |

Where the mode and the stream rule out interaction — for an empty menu, on a non-Unix
platform, or when the process has no controlling terminal — `run` returns
`Outcome::Unavailable` immediately. **It never blocks on a read that cannot be answered**,
which is what keeps a menu safe in a pipeline and in CI.

`Menu::render` needs no feature at all. It is plain formatting with no cursor and no terminal
control, and it is what a non-interactive caller shows instead.

## Terminal restoration

Raw mode is held by a guard that restores the terminal when it goes out of scope, on every
path out: an early return, an error, or an unwinding panic.

A signal that kills the process outright — `SIGTERM`, `SIGHUP` — leaves the terminal in raw
mode, because no destructor runs. runemark installs no signal handlers. An application that
must survive that has to install its own. `Ctrl-C` is not affected: in raw mode it arrives as
a key event and is reported as `Outcome::Cancelled`.

## Why not a terminal crate

The surface a menu needs is small: raw mode, `\x1b[?25l` / `\x1b[?25h` for the cursor,
`\x1b[{n}F` and `\x1b[J` for redrawing in place, and six keys. Pulling in a cross-platform
terminal stack for that costs far more than it returns — 28 packages against the two runemark
now has with the feature on.

Two details make the hand-written decoder correct rather than merely short:

- **A bare `Escape` and an arrow key start with the same byte.** Only a bounded read separates
  them. The bound comes from termios `VTIME`, not `poll` or `select`: on macOS a pty answers
  `poll` with `POLLNVAL` instead of readability, so a poll-based timeout blocks forever on a
  read that never completes. `VTIME` puts the timing in the kernel, where it is portable.
- **Arrow keys have two encodings.** `ESC [ A` normally, `ESC O A` in application cursor mode,
  which tmux and some terminals enable. Handling only the first breaks navigation there.
