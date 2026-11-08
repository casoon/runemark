//! Opinionated, human-readable terminal presentation for Rust CLIs.
//!
//! runemark owns semantic output conventions—status, emphasis, color policy,
//! and structured decision-oriented reports—so related tools behave consistently
//! in the terminal. Layout inputs such as the terminal width come from the host
//! application.
//!
//! It deliberately does not parse command-line arguments, own application logging,
//! or define domain-specific report schemas.

#![warn(missing_docs)]

pub mod color;
pub mod diff;
pub mod error_block;
mod internal;
pub mod location;
pub mod progress;
pub mod report;
// Temporary: the selection API still lacks item docs (#40); remove once documented.
pub mod select;
pub mod testing;
pub mod verdict;

pub use color::{ColorMode, Console, SymbolTheme, Tone};
pub use diff::{DiffBlock, FileAction, FileChange};
pub use error_block::ErrorBlock;
pub use location::{Location, format_osc8};
pub use progress::{PlainProgress, ProgressMode, ProgressSink, SilentProgress, TerminalProgress};
pub use report::{
    Badge, Confidence, DetailLevel, Finding, FindingGroup, Metric, NextStep, RenderOptions, Report,
    ScopeNote, Trend, Vocabulary,
};
pub use select::{Group, Hint, Item, Layout, Menu, Outcome, Picked, SelectMode};
pub use verdict::Verdict;

// Compile the README and the guide samples as doctests. The guides these include are
// listed outside `exclude` in Cargo.toml, so `cargo test` also passes on the packaged
// crate. The README carries a `select` sample, hence the feature gate.
#[cfg(all(doctest, feature = "select"))]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;

#[cfg(doctest)]
mod guide_doctests {
    #[doc = include_str!("../docs/getting-started/quickstart.mdx")]
    struct Quickstart;
    #[doc = include_str!("../docs/guides/colour-policy.md")]
    struct ColourPolicy;
    #[doc = include_str!("../docs/guides/errors-and-diffs.mdx")]
    struct ErrorsAndDiffs;
    #[doc = include_str!("../docs/guides/progress.md")]
    struct Progress;
    #[doc = include_str!("../docs/guides/reports.mdx")]
    struct Reports;
    #[doc = include_str!("../docs/guides/testing.md")]
    struct Testing;
}
