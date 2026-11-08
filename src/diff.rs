//! Diff and file generation preview components.

use crate::color::{Console, SymbolTheme, Tone, sanitize_visible_text};
use std::io::Write;
use std::path::PathBuf;

/// The action taken on a file by a scaffolder, generator, or auto-fixer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FileAction {
    /// File was created.
    Added,
    /// Existing file was modified.
    Modified,
    /// File was removed.
    Deleted,
    /// File was moved or renamed.
    Renamed,
}

impl FileAction {
    pub fn symbol(self, theme: SymbolTheme) -> &'static str {
        match (self, theme) {
            (Self::Added, _) => "+",
            (Self::Modified, _) => "~",
            (Self::Deleted, _) => "-",
            (Self::Renamed, SymbolTheme::Unicode) => "→",
            (Self::Renamed, SymbolTheme::Ascii) => "->",
        }
    }

    pub fn tone(self) -> Tone {
        match self {
            Self::Added => Tone::Success,
            Self::Modified => Tone::Warning,
            Self::Deleted => Tone::Error,
            Self::Renamed => Tone::Info,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Added => "CREATE",
            Self::Modified => "UPDATE",
            Self::Deleted => "DELETE",
            Self::Renamed => "RENAME",
        }
    }
}

/// A record of a single file modification.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct FileChange {
    pub path: PathBuf,
    pub action: FileAction,
    pub delta: Option<String>,
}

impl FileChange {
    /// Creates a file change entry.
    pub fn new(action: FileAction, path: impl Into<PathBuf>) -> Self {
        Self {
            action,
            path: path.into(),
            delta: None,
        }
    }

    /// Attaches a byte or line size delta string (e.g. `+1.2 kB`).
    pub fn with_delta(mut self, delta: impl Into<String>) -> Self {
        self.delta = Some(delta.into());
        self
    }
}

/// A block rendering generator or fixer file changes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct DiffBlock {
    pub changes: Vec<FileChange>,
}

impl DiffBlock {
    /// Creates a new empty diff block.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a file change to the block.
    pub fn add_change(mut self, change: FileChange) -> Self {
        self.changes.push(change);
        self
    }

    /// Writes the diff block directly to a writer.
    pub fn write_to(
        &self,
        console: Console,
        writer: &mut (impl Write + ?Sized),
    ) -> std::io::Result<()> {
        for c in &self.changes {
            write!(writer, "  ")?;
            console.write_paint(
                c.action.tone(),
                c.action.symbol(console.symbol_theme()),
                writer,
            )?;
            write!(writer, " ")?;
            console.write_paint(c.action.tone(), c.action.label(), writer)?;
            write!(
                writer,
                " {}",
                sanitize_visible_text(&c.path.display().to_string())
            )?;
            if let Some(d) = c.delta.as_deref() {
                console.write_paint(Tone::Muted, format!(" ({d})"), writer)?;
            }
            writeln!(writer)?;
        }
        Ok(())
    }

    /// Renders the diff block.
    pub fn render(&self, console: Console) -> String {
        crate::internal::collect_to_string(|buf| self.write_to(console, buf))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::ColorMode;

    #[test]
    fn diff_block_rendering() {
        let diff = DiffBlock::new()
            .add_change(
                FileChange::new(FileAction::Added, "src/components/Header.tsx")
                    .with_delta("+1.4 kB"),
            )
            .add_change(FileChange::new(FileAction::Modified, "src/App.tsx"));

        let console = Console::new(ColorMode::Never, false);
        let output = diff.render(console);

        assert!(output.contains("+ CREATE src/components/Header.tsx (+1.4 kB)"));
        assert!(output.contains("~ UPDATE src/App.tsx"));
    }

    #[test]
    fn diff_uses_ascii_rename_marker_for_plain_output() {
        let diff = DiffBlock::new().add_change(FileChange::new(FileAction::Renamed, "src/new.rs"));
        let console = Console::new(ColorMode::Never, false);

        assert_eq!(diff.render(console), "  -> RENAME src/new.rs\n");
    }
}
