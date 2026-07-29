//! Color policy, semantic tones, and symbol themes.

use std::fmt::Display;
use std::io::{IsTerminal, Write};

/// Controls whether semantic ANSI styles are emitted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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
        match self {
            Self::Auto => is_terminal && std::env::var_os("NO_COLOR").is_none(),
            Self::Always => true,
            Self::Never => false,
        }
    }
}

/// Semantic emphasis independent of a concrete terminal color theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
pub enum SymbolTheme {
    /// Use modern UTF-8 symbols (✓, ⚠, ✖, ℹ, etc.)
    #[default]
    Unicode,
    /// Use ASCII fallbacks for plain or legacy environments ([OK], [WARN], [FAIL], [INFO])
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

    /// Writes styled text directly into a writer (zero-allocation writer-first API).
    pub fn write_paint(
        self,
        tone: Tone,
        value: impl Display,
        writer: &mut impl Write,
    ) -> std::io::Result<()> {
        if !self.color {
            write!(writer, "{value}")
        } else {
            let style = match tone {
                Tone::Title => "\x1b[1;96m",
                Tone::Muted => "\x1b[2m",
                Tone::Info => "\x1b[36m",
                Tone::Success => "\x1b[1;92m",
                Tone::Warning => "\x1b[1;93m",
                Tone::Error => "\x1b[1;91m",
            };
            write!(writer, "{style}{value}\x1b[0m")
        }
    }

    /// Styles a value according to Runemark's semantic theme into a String.
    pub fn paint(self, tone: Tone, value: impl Display) -> String {
        let mut buf = Vec::new();
        let _ = self.write_paint(tone, &value, &mut buf);
        String::from_utf8(buf).unwrap_or_else(|_| value.to_string())
    }
}
