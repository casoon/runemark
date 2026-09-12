//! Location formatting and terminal hyperlink support (OSC 8).

use crate::color::{Console, Tone, sanitize_visible_text};
use std::fmt::{self, Display};
use std::path::PathBuf;

/// A location within a file, document, or remote resource.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Location {
    /// Local file path with optional line and column numbers.
    #[non_exhaustive]
    File {
        path: PathBuf,
        line: Option<usize>,
        column: Option<usize>,
    },
    /// Remote or web URL.
    Url(String),
    /// CSS or DOM selector / node target.
    Selector(String),
    /// Output artifact or destination path.
    Artifact(PathBuf),
}

impl Location {
    /// Creates a file location without a line or column number.
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self::File {
            path: path.into(),
            line: None,
            column: None,
        }
    }

    /// Creates a file location with line and optional column.
    pub fn file_line(path: impl Into<PathBuf>, line: usize) -> Self {
        Self::File {
            path: path.into(),
            line: Some(line),
            column: None,
        }
    }

    /// Creates a file location with line and column.
    pub fn file_line_col(path: impl Into<PathBuf>, line: usize, column: usize) -> Self {
        Self::File {
            path: path.into(),
            line: Some(line),
            column: Some(column),
        }
    }

    /// Renders the location as a plain text span (e.g. `src/lib.rs:42:10`).
    pub fn to_plain_string(&self) -> String {
        let plain = match self {
            Self::File { path, line, column } => match (line, column) {
                (Some(l), Some(c)) => format!("{}:{l}:{c}", path.display()),
                (Some(l), None) => format!("{}:{l}", path.display()),
                (None, _) => path.display().to_string(),
            },
            Self::Url(url) => url.clone(),
            Self::Selector(sel) => format!("`{sel}`"),
            Self::Artifact(path) => path.display().to_string(),
        };
        sanitize_visible_text(&plain).into_owned()
    }

    /// Renders the location, opting into OSC 8 terminal hyperlinks if colors/TTY are enabled.
    pub fn render(&self, console: Console) -> String {
        let plain = self.to_plain_string();
        if !console.color_enabled() {
            return plain;
        }

        match self {
            Self::File { path, line, column } => {
                let abs_path = path.canonicalize().unwrap_or_else(|_| {
                    if path.is_absolute() {
                        path.clone()
                    } else {
                        std::env::current_dir()
                            .map(|current_dir| current_dir.join(path))
                            .unwrap_or_else(|_| path.clone())
                    }
                });
                let mut url_path = abs_path.to_string_lossy().replace('\\', "/");
                if !url_path.starts_with('/') {
                    url_path.insert(0, '/');
                }
                let file_url = match (line, column) {
                    (Some(l), Some(c)) => format!("file://{}#{l}:{c}", percent_encode(&url_path)),
                    (Some(l), None) => format!("file://{}#{l}", percent_encode(&url_path)),
                    (None, _) => format!("file://{}", percent_encode(&url_path)),
                };
                let styled_text = console.paint(Tone::Muted, &plain);
                format_osc8_with_trusted_text(&file_url, &styled_text)
            }
            Self::Url(url) => {
                let styled_url = console.paint(Tone::Info, url);
                if is_allowed_url_scheme(url) && is_safe_osc8_target(url) {
                    format_osc8_with_trusted_text(url, &styled_url)
                } else {
                    styled_url
                }
            }
            Self::Selector(sel) => console.paint(Tone::Info, format!("`{sel}`")),
            Self::Artifact(path) => console.paint(Tone::Success, path.display()),
        }
    }
}

impl Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_plain_string())
    }
}

/// Checks whether a URL scheme is allowed for terminal hyperlinking.
///
/// Only `http://` and `https://` are permitted for remote targets.
pub fn is_allowed_url_scheme(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Checks whether an OSC 8 hyperlink target contains no control characters,
/// escape characters, or embedded termination sequences.
pub fn is_safe_osc8_target(url: &str) -> bool {
    !url.is_empty()
        && !url.chars().any(|ch| {
            ch.is_ascii_control() || ch == '\x7f' || matches!(ch, '\u{0080}'..='\u{009f}')
        })
}

/// Formats an OSC 8 hyperlinked string for terminal emulators.
///
/// If `url` contains control characters, escape codes, or embedded terminators,
/// hyperlink formatting is safely suppressed and `text` is returned unmodified.
pub fn format_osc8(url: &str, text: &str) -> String {
    let safe_text = sanitize_visible_text(text);
    if !is_safe_osc8_target(url) {
        return safe_text.into_owned();
    }
    format_osc8_with_trusted_text(url, &safe_text)
}

fn format_osc8_with_trusted_text(url: &str, text: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
}

fn percent_encode(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'.' | b'_' | b'~' | b':') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::ColorMode;

    #[test]
    fn location_plain_formatting() {
        let loc = Location::file_line_col("src/main.rs", 12, 4);
        assert_eq!(loc.to_plain_string(), "src/main.rs:12:4");
    }

    #[test]
    fn location_osc8_rendering() {
        let loc = Location::Url("https://example.com".into());
        let console = Console::new(ColorMode::Always, false);
        let rendered = loc.render(console);
        assert!(rendered.contains("\x1b]8;;https://example.com\x1b\\"));
    }

    #[test]
    fn file_location_osc8_rendering_uses_an_absolute_encoded_url() {
        let loc = Location::file_line("a path#with special.rs", 12);
        let console = Console::new(ColorMode::Always, false);
        let rendered = loc.render(console);

        assert!(rendered.contains("\x1b]8;;file:///"));
        assert!(rendered.contains("a%20path%23with%20special.rs#12\x1b\\"));
    }

    #[test]
    fn location_url_rejects_disallowed_schemes() {
        let loc = Location::Url("javascript:alert(1)".into());
        let console = Console::new(ColorMode::Always, false);
        let rendered = loc.render(console);
        assert!(!rendered.contains("\x1b]8;;"));
        assert!(rendered.contains("javascript:alert(1)"));
    }

    #[test]
    fn location_url_rejects_embedded_escape_characters() {
        let loc = Location::Url("https://example.com/\x1b]8;;\x07evil".into());
        let console = Console::new(ColorMode::Always, false);
        let rendered = loc.render(console);
        assert!(!rendered.contains("\x1b]8;;https://"));
    }

    #[test]
    fn format_osc8_suppresses_unsafe_target() {
        let text = "click here";
        assert_eq!(
            format_osc8("https://safe.com", text),
            "\x1b]8;;https://safe.com\x1b\\click here\x1b]8;;\x1b\\"
        );
        assert_eq!(format_osc8("https://evil.com\x07echo", text), text);
        assert_eq!(format_osc8("https://evil.com\x1b\\pwn", text), text);
    }

    #[test]
    fn plain_locations_and_public_osc8_text_neutralize_control_characters() {
        let loc = Location::Url("https://example.com/\x1b[31m\rspoof".into());
        assert_eq!(
            loc.render(Console::new(ColorMode::Never, false)),
            "https://example.com/^[[31m^Mspoof"
        );

        let rendered = format_osc8("https://example.com", "safe\x1b[31m\rtext");
        assert!(rendered.contains("safe^[[31m^Mtext"));
        assert!(!rendered.contains("safe\x1b[31m"));
    }
}
