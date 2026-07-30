//! Location formatting and terminal hyperlink support (OSC 8).

use crate::color::{Console, Tone};
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
        match self {
            Self::File { path, line, column } => match (line, column) {
                (Some(l), Some(c)) => format!("{}:{l}:{c}", path.display()),
                (Some(l), None) => format!("{}:{l}", path.display()),
                (None, _) => path.display().to_string(),
            },
            Self::Url(url) => url.clone(),
            Self::Selector(sel) => format!("`{sel}`"),
            Self::Artifact(path) => path.display().to_string(),
        }
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
                format_osc8(&file_url, &styled_text)
            }
            Self::Url(url) => {
                let styled_url = console.paint(Tone::Info, url);
                format_osc8(url, &styled_url)
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

/// Formats an OSC 8 hyperlinked string for terminal emulators.
pub fn format_osc8(url: &str, text: &str) -> String {
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
}
