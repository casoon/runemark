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
#[allow(missing_docs)]
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
