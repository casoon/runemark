//! Color policy, semantic tones, and symbol themes.

use std::fmt::Display;
use std::io::{IsTerminal, Write};

/// Controls whether semantic ANSI styles are emitted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ColorMode {
    /// Emit styles only for an interactive terminal and when `NO_COLOR` is unset.
    #[default]
    Auto,
    /// Always emit styles.
    Always,
    /// Never emit styles.
    Never,
}

impl ColorMode {
    /// Resolves this policy for a specific output stream capability.
    pub fn enabled(self, is_terminal: bool) -> bool {
        self.enabled_with(is_terminal, std::env::var_os("NO_COLOR").is_none())
    }

    fn enabled_with(self, is_terminal: bool, no_color_unset: bool) -> bool {
        match self {
            Self::Auto => is_terminal && no_color_unset,
            Self::Always => true,
            Self::Never => false,
        }
    }
}

/// Semantic emphasis independent of a concrete terminal color theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Tone {
    Title,
    Muted,
    Info,
    Success,
    Warning,
    Error,
}

/// Symbol output mode for icons (Unicode vs ASCII fallback).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum SymbolTheme {
    /// Use modern UTF-8 symbols (✓, ⚠, ✖, ℹ, etc.)
    #[default]
    Unicode,
    /// Use ASCII fallbacks for plain or legacy environments (`[OK]`, `[WARN]`, `[FAIL]`, `[INFO]`)
    Ascii,
}

/// Applies Runemark's color policy and semantic theme to one terminal stream.
#[derive(Debug, Clone, Copy)]
pub struct Console {
    color: bool,
    theme: SymbolTheme,
}

impl Console {
    /// Creates a console for a stream with the supplied terminal capability.
    pub fn new(color: ColorMode, is_terminal: bool) -> Self {
        let enabled = color.enabled(is_terminal);
        Self {
            color: enabled,
            theme: if enabled {
                SymbolTheme::Unicode
            } else {
                SymbolTheme::Ascii
            },
        }
    }

    /// Creates a console intended for standard output.
    pub fn stdout(color: ColorMode) -> Self {
        Self::new(color, std::io::stdout().is_terminal())
    }

    /// Creates a console intended for standard error.
    pub fn stderr(color: ColorMode) -> Self {
        Self::new(color, std::io::stderr().is_terminal())
    }

    /// Explicitly override the symbol theme.
    pub fn with_theme(mut self, theme: SymbolTheme) -> Self {
        self.theme = theme;
        self
    }

    /// Returns whether this console emits ANSI styles.
    pub fn color_enabled(self) -> bool {
        self.color
    }

    /// Returns the symbol theme used by this console.
    pub fn symbol_theme(self) -> SymbolTheme {
        self.theme
    }

    /// Writes styled, control-character-safe text directly into a writer.
    pub fn write_paint(
        self,
        tone: Tone,
        value: impl Display,
        writer: &mut (impl Write + ?Sized),
    ) -> std::io::Result<()> {
        let text = value.to_string();
        let sanitized = sanitize_visible_text(&text);
        if !self.color {
            write!(writer, "{sanitized}")
        } else {
            let style = match tone {
                Tone::Title => "\x1b[1;96m",
                Tone::Muted => "\x1b[2m",
                Tone::Info => "\x1b[36m",
                Tone::Success => "\x1b[1;92m",
                Tone::Warning => "\x1b[1;93m",
                Tone::Error => "\x1b[1;91m",
            };
            write!(writer, "{style}{sanitized}\x1b[0m")
        }
    }

    /// Styles a value according to Runemark's semantic theme into a String.
    pub fn paint(self, tone: Tone, value: impl Display) -> String {
        crate::internal::collect_to_string(|buf| self.write_paint(tone, &value, buf))
    }
}

pub(crate) fn is_dangerous_control(ch: char) -> bool {
    matches!(ch, '\x1b' | '\x07' | '\x7f' | '\u{0080}'..='\u{009f}')
        || (ch.is_ascii_control() && !matches!(ch, '\n' | '\t'))
}

pub(crate) fn sanitize_visible_text(input: &str) -> std::borrow::Cow<'_, str> {
    if !input.chars().any(is_dangerous_control) {
        return std::borrow::Cow::Borrowed(input);
    }

    let mut sanitized = String::with_capacity(input.len());
    for ch in input.chars() {
        if !is_dangerous_control(ch) {
            sanitized.push(ch);
        } else if ch == '\x1b' {
            sanitized.push_str("^[");
        } else if ch == '\x07' {
            sanitized.push_str("^G");
        } else if ch == '\x7f' {
            sanitized.push_str("^?");
        } else if ch.is_ascii_control() {
            let caret_char = ((ch as u8) + b'@') as char;
            sanitized.push('^');
            sanitized.push(caret_char);
        }
    }
    std::borrow::Cow::Owned(sanitized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_mode_always_and_never_ignore_the_terminal() {
        assert!(ColorMode::Always.enabled(false));
        assert!(!ColorMode::Never.enabled(true));
    }

    #[test]
    fn color_mode_auto_requires_a_terminal() {
        assert!(!ColorMode::Auto.enabled(false));
    }

    #[test]
    fn color_mode_auto_honours_no_color() {
        assert!(!ColorMode::Auto.enabled_with(true, false));
        assert!(ColorMode::Auto.enabled_with(true, true));
    }

    #[test]
    fn console_disabled_color_falls_back_to_ascii_theme() {
        let console = Console::new(ColorMode::Never, true);
        assert!(!console.color_enabled());
        assert_eq!(console.symbol_theme(), SymbolTheme::Ascii);
    }

    #[test]
    fn console_enabled_color_defaults_to_unicode_theme() {
        let console = Console::new(ColorMode::Always, false);
        assert!(console.color_enabled());
        assert_eq!(console.symbol_theme(), SymbolTheme::Unicode);
    }

    #[test]
    fn with_theme_overrides_the_default_theme() {
        let console = Console::new(ColorMode::Always, false).with_theme(SymbolTheme::Ascii);
        assert_eq!(console.symbol_theme(), SymbolTheme::Ascii);
    }

    #[test]
    fn paint_returns_plain_text_when_color_is_disabled() {
        let console = Console::new(ColorMode::Never, false);
        assert_eq!(console.paint(Tone::Error, "boom"), "boom");
    }

    #[test]
    fn paint_wraps_the_value_in_ansi_codes_when_color_is_enabled() {
        let console = Console::new(ColorMode::Always, false);
        assert_eq!(console.paint(Tone::Error, "boom"), "\x1b[1;91mboom\x1b[0m");
    }

    #[test]
    fn paint_neutralizes_embedded_escape_sequences() {
        let console = Console::new(ColorMode::Never, false);
        let malicious = "\x1b[32mFake Green\x1b[0m\x07\x7f";
        assert_eq!(
            console.paint(Tone::Info, malicious),
            "^[[32mFake Green^[[0m^G^?"
        );
    }

    #[test]
    fn paint_preserves_newlines_and_tabs_but_neutralizes_carriage_returns() {
        let console = Console::new(ColorMode::Never, false);
        let formatted = "Line 1\n\tIndented Line 2\r\nDone";
        assert_eq!(
            console.paint(Tone::Info, formatted),
            "Line 1\n\tIndented Line 2^M\nDone"
        );
    }
}
