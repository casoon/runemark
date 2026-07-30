//! Opinionated, human-readable terminal presentation for Rust CLIs.
//!
//! Runemark owns semantic output conventions—status, emphasis, color policy,
//! and structured decision-oriented reports—while delegating low-level terminal
//! consistent terminal behavior.
//!
//! It deliberately does not parse command-line arguments, own application logging,
//! or define domain-specific report schemas.

pub mod color;
pub mod diff;
pub mod error_block;
mod internal;
pub mod location;
pub mod progress;
pub mod report;
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
pub use verdict::Verdict;
