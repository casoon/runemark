//! Aligned key-value pairs for compact detail headers.
//!
//! The metadata block above a body of text: the title, the account, the state of
//! whatever a command is about to show. For a verdict with findings and next steps
//! use a [`Report`](crate::Report); this is the lighter shape. Pair it with
//! [`Console::rule`] to separate the block from the body.

use crate::color::{Console, Tone};
use crate::internal::single_line;
use std::io::Write;
use unicode_width::UnicodeWidthStr;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    key: String,
    tone: Option<Tone>,
    value: String,
}

/// Key-value pairs, one per line, with the keys muted and the values aligned.
///
/// Every line is `Key: value`; the values start in the same column, measured with
/// `unicode-width`. Keys and values are single-line: control characters are
/// neutralised and a newline or tab becomes a space.
///
/// ```
/// use runemark::{ColorMode, Console, KeyValues, Tone};
///
/// let header = KeyValues::new()
///     .add("Title", "Quarterly plan")
///     .add_toned("State", Tone::Success, "approved");
///
/// let console = Console::new(ColorMode::Never, false);
/// assert_eq!(header.render(console), "Title: Quarterly plan\nState: approved\n");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyValues {
    entries: Vec<Entry>,
}

impl KeyValues {
    /// Creates an empty list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a pair whose value is plain text.
    pub fn add(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.entries.push(Entry {
            key: key.into(),
            tone: None,
            value: value.into(),
        });
        self
    }

    /// Adds a pair whose value is painted in `tone`.
    pub fn add_toned(
        mut self,
        key: impl Into<String>,
        tone: Tone,
        value: impl Into<String>,
    ) -> Self {
        self.entries.push(Entry {
            key: key.into(),
            tone: Some(tone),
            value: value.into(),
        });
        self
    }

    /// Writes the pairs directly to a writer, one line each.
    pub fn write_to(
        &self,
        console: Console,
        writer: &mut (impl Write + ?Sized),
    ) -> std::io::Result<()> {
        let keys: Vec<String> = self
            .entries
            .iter()
            .map(|entry| single_line(&entry.key))
            .collect();
        let width = keys.iter().map(|key| key.width()).max().unwrap_or(0);

        for (entry, key) in self.entries.iter().zip(&keys) {
            console.write_paint(Tone::Muted, format!("{key}:"), writer)?;
            write!(writer, "{:pad$} ", "", pad = width - key.width())?;
            let value = single_line(&entry.value);
            match entry.tone {
                Some(tone) => console.write_paint(tone, value, writer)?,
                None => write!(writer, "{value}")?,
            }
            writeln!(writer)?;
        }
        Ok(())
    }

    /// Renders the pairs as a String, one line each.
    pub fn render(&self, console: Console) -> String {
        crate::internal::collect_to_string(|buf| self.write_to(console, buf))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::ColorMode;

    fn plain() -> Console {
        Console::new(ColorMode::Never, false)
    }

    #[test]
    fn values_start_in_the_same_column() {
        let out = KeyValues::new()
            .add("Title", "Plan")
            .add("Account", "work")
            .render(plain());
        assert_eq!(out, "Title:   Plan\nAccount: work\n");
    }

    #[test]
    fn wide_keys_are_aligned_by_display_width() {
        let out = KeyValues::new()
            .add("名前", "a")
            .add("id", "b")
            .render(plain());
        assert_eq!(out, "名前: a\nid:   b\n");
    }

    #[test]
    fn keys_are_muted_and_values_toned_only_with_colour() {
        let list = KeyValues::new().add_toned("State", Tone::Success, "ok");
        assert_eq!(
            list.render(Console::new(ColorMode::Always, false)),
            "\x1b[2mState:\x1b[0m \x1b[1;92mok\x1b[0m\n"
        );
        assert_eq!(list.render(plain()), "State: ok\n");
    }

    #[test]
    fn text_is_single_line_and_control_characters_are_neutralised() {
        let out = KeyValues::new().add("a\nb", "x\ty\x1b[31m").render(plain());
        assert_eq!(out, "a b: x y^[[31m\n");
    }

    #[test]
    fn an_empty_list_writes_nothing() {
        assert_eq!(KeyValues::new().render(plain()), "");
    }
}
