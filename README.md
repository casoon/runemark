# runemark

Opinionated, human-readable terminal presentation for Rust command-line tools.

Runemark gives related CLI tools one consistent way to communicate progress,
status, findings, and next steps. It owns presentation conventions—not command
parsing, logging, domain models, or machine-readable report formats.

> Status: `0.1` is the initial public release. The API remains intentionally
> small while it is validated in real CLI tools.

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
- No runtime dependencies.

## Install

Add the core presentation layer to a Rust project:

```bash
cargo add runemark
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

## Design boundaries

| Runemark owns | Applications own |
| --- | --- |
| Terminal color policy and semantic tones | CLI arguments and configuration |
| Compact human-readable report layout | Domain finding types and business rules |
| Detail level conventions and next-step blocks | JSON, SARIF, Markdown, HTML, and other artifacts |
| Line-oriented terminal presentation | Logging, tracing, prompts, and full-screen TUIs |

## Roadmap

The initial API is deliberately narrow. Future additions are driven by real
consumer needs rather than speculative abstractions:

1. Validate the current API in real CLI tools before expanding it.

## Development

Run the complete local validation suite:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo package --locked
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for API and contribution guidelines.

## License

Runemark is licensed under the MIT license ([LICENSE](LICENSE)).
