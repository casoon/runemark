# runemark

Opinionated, human-readable terminal presentation for Rust and Node.js
command-line tools.

Runemark gives related CLI tools one consistent way to communicate progress,
status, findings, and next steps. It owns presentation conventions—not command
parsing, logging, domain models, or machine-readable report formats.

**Website and documentation:** [casoon.github.io/runemark](https://casoon.github.io/runemark/)

> Status: `0.3` is released on crates.io and npm. The API remains
> intentionally small while it is validated in real CLI tools.

## What it provides

Most command-line tools start with a mix of `println!`, ANSI snippets, and a
progress bar. That works until the tool needs consistent CI output, color
policy, grouped findings, compact summaries, detailed mode, or actionable next
steps.

Runemark is the shared presentation layer for those concerns:

- Semantic output: success, warning, error, info, muted text, and headings.
- Predictable color policy: TTY-aware by default, `NO_COLOR` compliant, with
  explicit `always` and `never` modes.
- Plain, deterministic output when redirected or snapshot-tested.
- Report models for verdicts, summary metrics, finding groups, and next steps.
- Actionable error blocks, file-change previews, and clickable locations for
  compatible terminals.
- Grouped, keyboard-driven selection with filtering, for tools that need a menu
  rather than a prompt (Unix).
- Small core dependency footprint; `indicatif` and `libc` are optional, behind
  the `progress` and `select` features.

## Install

Add the core presentation layer to a Rust project:

```bash
cargo add runemark
```

For Node.js 20 or newer, install the native package:

```bash
npm install @casoon/runemark
```

## Quick start

```rust
use runemark::{ColorMode, Console, Tone};

let console = Console::stdout(ColorMode::Auto);

println!("{}", console.paint(Tone::Title, "site audit"));
println!("{}", console.paint(Tone::Warning, "3 findings need review"));
println!("{}", console.paint(Tone::Muted, "Run with --details for every finding."));
```

`ColorMode::Auto` emits styles only to an interactive terminal and honours the
[`NO_COLOR`](https://no-color.org/) convention. Use `Always` for a forced
colorful local experience or `Never` for tests and plain logs.

## Reports

Build reports from application-owned data and choose compact or detailed
rendering in the host CLI. Runemark never defines domain-specific finding or
machine-report types.

```rust
use runemark::{Finding, FindingGroup, Report, Tone, Verdict};

let report = Report::new("Site audit", Verdict::Warning).add_group(
    FindingGroup::new("Accessibility")
        .add_finding(Finding::new(Tone::Warning, "Image has no text alternative")),
);

print!("{}", report.render(runemark::Console::stdout(runemark::ColorMode::Auto)));
```

### Metrics as outcomes

A metric carries a value; `with_verdict` also makes it a status:

```rust
use runemark::{Metric, Verdict};

let metric = Metric::new("Tests", "1.2s").with_verdict(Verdict::Failed);
```

A tone alone disappears with colour — piped, or under `NO_COLOR`, a failing
metric reads exactly like a passing one. The verdict's symbol survives both, and
supplies the tone where none was set.

### Adaptive layout

Applications can provide a known terminal width without giving Runemark access
to their terminal environment. Narrow layouts stack summary metrics and wrap
finding messages with a hanging indent.

```rust
use runemark::{ColorMode, Console, RenderOptions, Report, Verdict};

# let report = Report::new("Site audit", Verdict::Info);

let output = report.render_with_options(
    Console::stdout(ColorMode::Auto),
    RenderOptions::new().with_width(80),
);
```

### Progress

The optional `progress` feature adds an `indicatif`-backed terminal progress
sink. `Auto` uses a compact, line-oriented lifecycle when output is redirected,
so machine-readable stdout remains clean.

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

### Selection

The optional `select` feature adds a grouped menu. Runemark owns the layout, the
cursor and the key handling; the application owns what the entries are.

It talks to the terminal directly — termios for raw mode, four escape sequences
for drawing — so the only dependency is `libc` and the interactive path is
Unix-only. Elsewhere `run` reports `Outcome::Unavailable`, which is the same
fallback a pipeline takes, so callers need no `cfg`.

```rust
use runemark::{ColorMode, Console, Group, Hint, Item, Menu, Outcome, SelectMode};
use std::io::IsTerminal;

let menu = Menu::new()
    .with_heading("casoon.dev")
    .with_note("pnpm")
    .add_group(
        Group::new("Development")
            .add_item(Item::new("dev", "dev").with_description("Start the site")),
    )
    .add_hint(Hint::new('U', "Updates"));

let outcome = menu.run(
    Console::stderr(ColorMode::Auto),
    SelectMode::Auto,
    std::io::stderr().is_terminal(),
)?;

// Without a terminal the menu does not run and never blocks; render it instead.
if outcome == Outcome::Unavailable {
    print!("{}", menu.render(Console::stdout(ColorMode::Auto)));
}
# Ok::<(), std::io::Error>(())
```

`/` filters the menu as you type, ranking names above descriptions and tight
matches above scattered ones.

`Layout::Tabs` puts the groups in a row above the list and shows only the
active one's entries, for a list that would otherwise be taller than the
terminal — `←` `→` and the digits switch groups. `Group::in_tab` collapses a
run of groups into one tab where they would otherwise flood the row, and
`Group::with_divider` marks where one kind of tab ends and another begins.
`Menu::with_summary` adds a second heading line for what the menu adds up to.

`Menu::render` needs no feature — it is plain formatting, and it is what a
non-interactive caller shows. A menu larger than the terminal is fitted to it:
taller lists scroll, with `↑ N more` / `↓ N more` marking what is out of view,
and entries are shortened rather than wrapped. `render` does neither, since a
pipe has no height or width to run out of.

## Design boundaries

| Runemark owns | Applications own |
| --- | --- |
| Terminal color policy and semantic tones | CLI arguments and configuration |
| Compact human-readable report layout | Domain finding types and business rules |
| Detail level conventions and next-step blocks | JSON, SARIF, Markdown, HTML, and other artifacts |
| Line-oriented terminal presentation | Logging, tracing, free-text prompts, and full-screen TUIs |
| Grouped interactive selection (`select`, Unix) | What the entries mean, how they are grouped and ordered |

## Roadmap

The initial API is deliberately narrow. Future additions are driven by real
consumer needs rather than speculative abstractions:

1. Validate the current API in real CLI tools before expanding it.

## Examples

Run the Rust examples directly from this repository:

```bash
cargo run --example report_demo
cargo run --example progress_demo --features progress
cargo run --example select_demo --features select
```

Node.js examples are in [examples/node](examples/node). Build the local native
package once, then run an example:

```bash
npm --prefix packages/runemark run build
node examples/node/report.cjs
```

## Development

Run the complete local validation suite:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo package --locked
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for API and contribution guidelines.
See [NODE_API.md](NODE_API.md) for the stable Node.js package contract.
The npm package includes its own README with [Node.js examples](packages/runemark/README.md).

## License

Runemark is licensed under the MIT license ([LICENSE](LICENSE)).
