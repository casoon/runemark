//! Verdict models and renderers for CLI commands and audit outcomes.

use crate::color::sanitize_visible_text;
use crate::color::{Console, SymbolTheme, Tone};
use std::io::Write;

/// The outcome status of a CLI command, audit run, or validation pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Verdict {
    /// Operation completed successfully with no findings or issues.
    Passed,
    /// Operation completed with non-blocking warnings.
    Warning,
    /// Operation failed with error findings or broken assertions.
    Failed,
    /// Manual user action or configuration step is required.
    ActionRequired,
    /// Step or check was skipped.
    Skipped,
    /// Information-only outcome.
    Info,
}

impl Verdict {
    /// Returns the semantic tone corresponding to this verdict.
    pub fn tone(self) -> Tone {
        match self {
            Self::Passed => Tone::Success,
            Self::Warning => Tone::Warning,
            Self::Failed => Tone::Error,
            Self::ActionRequired => Tone::Warning,
            Self::Skipped => Tone::Muted,
            Self::Info => Tone::Info,
        }
    }

    /// Returns the symbol representation according to the active theme.
    pub fn symbol(self, theme: SymbolTheme) -> &'static str {
        match (self, theme) {
            (Self::Passed, SymbolTheme::Unicode) => "✔",
            (Self::Passed, SymbolTheme::Ascii) => "[OK]",

            (Self::Warning, SymbolTheme::Unicode) => "⚠",
            (Self::Warning, SymbolTheme::Ascii) => "[WARN]",

            (Self::Failed, SymbolTheme::Unicode) => "✖",
            (Self::Failed, SymbolTheme::Ascii) => "[FAIL]",

            (Self::ActionRequired, SymbolTheme::Unicode) => "➜",
            (Self::ActionRequired, SymbolTheme::Ascii) => "[ACT]",

            (Self::Skipped, SymbolTheme::Unicode) => "⊝",
            (Self::Skipped, SymbolTheme::Ascii) => "[SKIP]",

            (Self::Info, SymbolTheme::Unicode) => "ℹ",
            (Self::Info, SymbolTheme::Ascii) => "[INFO]",
        }
    }

    /// Default text label for the verdict.
    pub fn label(self) -> &'static str {
        match self {
            Self::Passed => "PASSED",
            Self::Warning => "WARNING",
            Self::Failed => "FAILED",
            Self::ActionRequired => "ACTION REQUIRED",
            Self::Skipped => "SKIPPED",
            Self::Info => "INFO",
        }
    }

    /// Writes a verdict line formatted with symbol, optional label, and message directly to a writer.
    pub fn write_to(
        self,
        console: Console,
        message: &str,
        writer: &mut (impl Write + ?Sized),
    ) -> std::io::Result<()> {
        let sym = self.symbol(console.symbol_theme());
        console.write_paint(self.tone(), sym, writer)?;
        write!(writer, " {}", sanitize_visible_text(message))
    }

    /// Renders a verdict line formatted with symbol, optional label, and message as a String.
    pub fn render(self, console: Console, message: &str) -> String {
        crate::internal::collect_to_string(|buf| self.write_to(console, message, buf))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::ColorMode;

    #[test]
    fn verdict_ascii_rendering() {
        let console = Console::new(ColorMode::Never, false).with_theme(SymbolTheme::Ascii);
        assert_eq!(
            Verdict::Passed.render(console, "All checks green"),
            "[OK] All checks green"
        );
        assert_eq!(
            Verdict::Failed.render(console, "2 errors found"),
            "[FAIL] 2 errors found"
        );
    }
}
