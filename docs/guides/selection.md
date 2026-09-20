---
title: Selection
description: A grouped, keyboard-driven menu for tools that need a choice rather than a prompt.
order: 5
---

The optional `select` feature adds a grouped menu. It is the one place where runemark reads
from the terminal instead of only writing to it — a cursor has to react to keys.

It drives the terminal directly: termios for raw mode, four escape sequences for drawing, and
a small decoder for the keys a menu needs. The only dependency is `libc`, and the
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
| `←` `→`, `Tab`, `Shift-Tab` | Switch groups, with `Layout::Tabs` |
| `1`–`9` | Jump to that group, with `Layout::Tabs` |
| `Enter` | Select, returning `Outcome::Selected` |
| `/` | Start filtering |
| A hint key | Returns `Outcome::Hotkey`, matched case-insensitively |
| `Esc`, `q` | `Outcome::Cancelled` |
| `Ctrl-C` | `Outcome::Cancelled` |

A hint key wins over the built-in `q`, so a menu is free to bind `q` itself. `/` is reserved
for the filter and cannot be bound. A tabbed menu has given its digits away: they reach its
groups, and a hint bound to one is not seen there — the tab is the one of the two the user
can see on screen.

## Groups as tabs

`Layout::Tabs` puts the groups in a row above the list and shows only the active one's
entries:

```rust
# use runemark::{Group, Item, Layout, Menu};
let menu = Menu::new()
    .with_heading("web-casoon")
    .with_note("pnpm")
    .with_summary("27 scripts · 7 groups")
    .with_layout(Layout::Tabs)
    .add_group(Group::new("Development").add_item(Item::new("dev", "dev")))
    .add_group(Group::new("Build").add_item(Item::new("build", "build")));
```

```
web-casoon                                            pnpm
  27 scripts · 7 groups

  1 Development   2 Build   3 Preview   4 Deploy   5 Quality   …
    ─────────────
› dev              Start the site
  dev:landings
```

**Which groups become which tabs is the application's call**, as is when to ask for the
layout at all. A menu knows how many entries it has, not
how much of the screen its caller is willing to spend, and a layout that flipped on its own
whenever a window was resized would rearrange the list under a cursor already moving through
it.

The rule under the active tab is not decoration. With colour off it is the only thing on the
screen saying which group the entries below belong to.

Tabs past the ninth carry no digit — there is none left to offer — and the arrows still reach
them. When the row is wider than the terminal it is windowed like the body, growing outwards
from the active tab so that switching along the row scrolls it rather than jumping it. Tabs
are dropped from the ends rather than shortened: half a group name is no longer the word its
digit belongs to.

A single group gets no tab row. There is nothing to switch to, and the row would spend two
lines repeating the heading.

### Groups that share a tab

`Group::in_tab` puts a run of groups in one tab instead of one each, and `Group::with_divider`
marks where the row stops being one kind of thing and starts being another:

```rust
# use runemark::{Group, Item, Layout, Menu};
# let menu = Menu::new().with_layout(Layout::Tabs);
# let menu = menu.add_group(Group::new("Build").add_item(Item::new("b", "build")));
let menu = menu
    .add_group(
        Group::new("@scope/app")
            .in_tab("Packages")
            .with_divider()
            .add_item(Item::new("app/test", "test")),
    )
    .add_group(
        Group::new("@scope/site")
            .in_tab("Packages")
            .add_item(Item::new("site/test", "test")),
    );
```

```
  1 Development   2 Build   3 Quality   4 Deploy   │   5 Packages
                                                       ──────────
@scope/app
› test
@scope/site
  test
```

Inside the tab each group keeps its label as a heading, so the grouping survives; only the
row gets its length back. A group that gave the tab its name carries no heading, since the
row above says it already.

This is for a set of groups the row cannot carry. One tab each stops working sooner than it
looks: a real workspace here has 52 packages against 6 actions, which is a row of 58 that is
almost entirely package names, permanently scrolling, with the digits worthless past the
ninth. Collapsed, it is seven tabs.

A divider is drawn only between two tabs that are both on screen. Leading the row with one
would separate the tabs from nothing, and where the window starts mid-row the ellipsis
already marks the break.

`Menu::render` ignores the layout and lists every group. Nothing on the other end of a pipe
can press a key to reach the second tab, so hiding one there would lose entries rather than
save lines.

## Filtering

An interactive menu with more than a handful of entries shows `/ search` in its footer, before
any hints the application adds. A short one does not — filtering still works, but a yes/no
question offering to search itself reads as clutter, and below that every entry is already on
screen. `Menu::render` shows no keys at all, since a pipe has no keyboard.

`/` starts a filter; typing narrows the menu, `Backspace` widens it again. Groups with nothing
left disappear, and the cursor sits on the best match.

The filter spans **every** group in either layout, and a tabbed menu leaves its tab row while
it runs. Tabs answer "I know roughly where"; the filter answers "I know exactly what", which
is the case tabs are worst at — a query matching four groups shows all four together. Leaving
the filter returns to the tab it was started from.

`Esc` leaves the filter before it leaves the menu, so a mistyped query costs one key rather
than the whole selection. A second `Esc` cancels. While filtering, every printable key is part
of the query — a menu that binds `q` as a hint still lets you type `quality`.

Matching is case-insensitive and ranked in tiers:

1. the query as a substring of the **name**, earlier position first
2. the query as a **subsequence** of the name, tightest span first — `dpl` finds `deploy`
3. the query as a substring of the **description**

A name the user is typing towards beats a description that happens to contain the same
letters. An empty query is not a search result: it restores the menu as it was, rather than
ranking everything equal and sorting it alphabetically.

## Terminals smaller than the menu

Entries are **shortened to fit the width**, with `…` marking the cut, rather than wrapped. A
wrapped line would change how many lines the frame occupies, which the redraw counts on, and a
description spilling to column zero is what makes a long list unreadable in the first place.
Where the label column leaves too little room for a description to say anything, the
description is left out instead of cut to a stub.

A menu taller than the terminal is windowed: the body scrolls, and `↑ N more` / `↓ N more`
mark what is out of view. `Layout::Tabs` is the other answer to the same problem, for a list
whose groups mean something: it pages by group instead of scrolling past every one of them. The window moves the least amount that keeps the cursor visible, so
short cursor moves do not slide the whole screen.

The size comes from the terminal itself, re-read on every frame, so resizing the window
while the menu is open is picked up without a `SIGWINCH` handler. A terminal that reports no
height gets the whole menu.

`Menu::render` is neither windowed nor shortened. A pipe or a file has no height or width to
run out of, and cutting there would lose information for no reason.

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
