//! TTY-aware progress sinks with an optional `indicatif` backend.

use crate::{Console, Tone, Verdict};
use std::io::Write;
use std::sync::Mutex;

/// Controls whether a progress sink renders an interactive progress bar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProgressMode {
    /// Use an interactive bar only for a terminal; use plain lifecycle lines otherwise.
    #[default]
    Auto,
    /// Prefer an interactive bar even if the output stream is not a terminal.
    Always,
    /// Suppress all progress output.
    Never,
}

impl ProgressMode {
    /// Returns whether this mode should use an interactive progress renderer.
    pub const fn is_interactive(self, is_terminal: bool) -> bool {
        match self {
            Self::Auto => is_terminal,
            Self::Always => true,
            Self::Never => false,
        }
    }
}

/// A thread-safe lifecycle sink for a long-running operation.
///
/// Progress updates are best-effort display events. A sink intentionally does
/// not propagate I/O failures into the application's primary operation.
pub trait ProgressSink: Send + Sync {
    /// Announces the start of an operation with its expected total work units.
    fn start(&self, total: u64, message: &str);

    /// Records the current completed work-unit count and an optional status message.
    fn advance(&self, position: u64, message: &str);

    /// Writes a notice without corrupting an active interactive progress bar.
    fn notice(&self, tone: Tone, message: &str);

    /// Closes the operation with its semantic outcome.
    fn finish(&self, verdict: Verdict, message: &str);
}

/// A progress sink that deliberately emits no output.
#[derive(Debug, Default)]
pub struct SilentProgress;

impl ProgressSink for SilentProgress {
    fn start(&self, _: u64, _: &str) {}

    fn advance(&self, _: u64, _: &str) {}

    fn notice(&self, _: Tone, _: &str) {}

    fn finish(&self, _: Verdict, _: &str) {}
}

/// A line-oriented progress sink for logs, pipes, and deterministic tests.
pub struct PlainProgress<W> {
    console: Console,
    writer: Mutex<W>,
}

impl<W> PlainProgress<W> {
    /// Creates a line-oriented progress sink using the supplied writer.
    pub fn new(console: Console, writer: W) -> Self {
        Self {
            console,
            writer: Mutex::new(writer),
        }
    }

    /// Returns the underlying writer after all progress output has completed.
    ///
    /// This is primarily useful for deterministic tests with an in-memory buffer.
    pub fn into_inner(self) -> W {
        self.writer
            .into_inner()
            .expect("progress writer mutex must not be poisoned")
    }
}

impl<W: Write + Send> ProgressSink for PlainProgress<W> {
    fn start(&self, total: u64, message: &str) {
        let Ok(mut writer) = self.writer.lock() else {
            return;
        };
        let _ = writeln!(writer, "{message} (0/{total})");
    }

    fn advance(&self, _: u64, _: &str) {
        // Plain output deliberately avoids one line per unit of work.
    }

    fn notice(&self, tone: Tone, message: &str) {
        let Ok(mut writer) = self.writer.lock() else {
            return;
        };
        let _ = self.console.write_paint(tone, message, &mut *writer);
        let _ = writeln!(writer);
    }

    fn finish(&self, verdict: Verdict, message: &str) {
        let Ok(mut writer) = self.writer.lock() else {
            return;
        };
        let _ = verdict.write_to(self.console, message, &mut *writer);
        let _ = writeln!(writer);
    }
}

/// A standard-error progress sink that selects an interactive bar when available.
pub enum TerminalProgress {
    /// No visible progress output.
    Silent(SilentProgress),
    /// Line-oriented progress for a pipe or when the optional backend is disabled.
    Plain(PlainProgress<std::io::Stderr>),
    /// Interactive progress backed by `indicatif`.
    #[cfg(feature = "progress")]
    Indicatif(IndicatifProgress),
}

impl TerminalProgress {
    /// Creates a progress sink on standard error.
    ///
    /// With the `progress` feature enabled, `Auto` selects an interactive bar
    /// only for a terminal. Otherwise, the sink writes a compact start/notice/
    /// finish lifecycle suitable for redirected logs.
    pub fn stderr(mode: ProgressMode, console: Console, _is_terminal: bool) -> Self {
        if mode == ProgressMode::Never {
            return Self::Silent(SilentProgress);
        }

        #[cfg(feature = "progress")]
        if mode.is_interactive(_is_terminal) {
            return Self::Indicatif(IndicatifProgress::new(console));
        }

        Self::Plain(PlainProgress::new(console, std::io::stderr()))
    }
}

impl ProgressSink for TerminalProgress {
    fn start(&self, total: u64, message: &str) {
        match self {
            Self::Silent(progress) => progress.start(total, message),
            Self::Plain(progress) => progress.start(total, message),
            #[cfg(feature = "progress")]
            Self::Indicatif(progress) => progress.start(total, message),
        }
    }

    fn advance(&self, position: u64, message: &str) {
        match self {
            Self::Silent(progress) => progress.advance(position, message),
            Self::Plain(progress) => progress.advance(position, message),
            #[cfg(feature = "progress")]
            Self::Indicatif(progress) => progress.advance(position, message),
        }
    }

    fn notice(&self, tone: Tone, message: &str) {
        match self {
            Self::Silent(progress) => progress.notice(tone, message),
            Self::Plain(progress) => progress.notice(tone, message),
            #[cfg(feature = "progress")]
            Self::Indicatif(progress) => progress.notice(tone, message),
        }
    }

    fn finish(&self, verdict: Verdict, message: &str) {
        match self {
            Self::Silent(progress) => progress.finish(verdict, message),
            Self::Plain(progress) => progress.finish(verdict, message),
            #[cfg(feature = "progress")]
            Self::Indicatif(progress) => progress.finish(verdict, message),
        }
    }
}

/// An interactive `indicatif` progress-bar implementation.
#[cfg(feature = "progress")]
pub struct IndicatifProgress {
    console: Console,
    bar: indicatif::ProgressBar,
}

#[cfg(feature = "progress")]
impl IndicatifProgress {
    /// Creates an initially hidden progress bar.
    pub fn new(console: Console) -> Self {
        Self {
            console,
            bar: indicatif::ProgressBar::new(0),
        }
    }
}

#[cfg(feature = "progress")]
impl ProgressSink for IndicatifProgress {
    fn start(&self, total: u64, message: &str) {
        self.bar.set_length(total);
        self.bar.set_style(
            indicatif::ProgressStyle::with_template(
                "{spinner:.cyan} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}",
            )
            .expect("the built-in progress template is valid")
            .progress_chars("#>-"),
        );
        self.bar.set_message(message.to_string());
    }

    fn advance(&self, position: u64, message: &str) {
        self.bar.set_position(position);
        self.bar.set_message(message.to_string());
    }

    fn notice(&self, tone: Tone, message: &str) {
        self.bar.println(self.console.paint(tone, message));
    }

    fn finish(&self, verdict: Verdict, message: &str) {
        self.bar
            .finish_with_message(verdict.render(self.console, message));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ColorMode;

    #[test]
    fn plain_progress_emits_bounded_lifecycle_lines() {
        let progress = PlainProgress::new(Console::new(ColorMode::Never, false), Vec::new());

        progress.start(4, "Auditing URLs");
        progress.advance(1, "https://example.com");
        progress.notice(Tone::Warning, "One URL failed");
        progress.finish(Verdict::Warning, "Audit complete");

        assert_eq!(
            String::from_utf8(progress.into_inner()).unwrap(),
            "Auditing URLs (0/4)\nOne URL failed\n[WARN] Audit complete\n"
        );
    }

    #[test]
    fn auto_mode_is_interactive_only_for_terminals() {
        assert!(ProgressMode::Auto.is_interactive(true));
        assert!(!ProgressMode::Auto.is_interactive(false));
        assert!(ProgressMode::Always.is_interactive(false));
    }
}
