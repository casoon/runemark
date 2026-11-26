//! Compact, aligned tables for flat listings.
//!
//! A [`Table`] is for the lists a report has no room for: a folder overview, a mail
//! list, a set of shortcuts. Columns are as wide as their widest cell, measured with
//! `unicode-width`, so CJK and emoji text lines up.

use crate::color::{Console, Tone};
use crate::internal::single_line;
use crate::report::Badge;
use std::io::Write;
use unicode_width::UnicodeWidthStr;

/// Columns of space between two table columns.
const GAP: usize = 2;

/// Horizontal alignment of a column's cells.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Align {
    /// Pad on the right. The default, for text.
    #[default]
    Left,
    /// Pad on the left, for counts and other numbers.
    Right,
}

/// How one table column is laid out: an optional title and an alignment.
///
/// A column that was never declared is left-aligned and has no title, so a table of
/// plain text needs no `Column` at all.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct Column {
    /// The heading above the column, shown in the muted tone.
    pub title: Option<String>,
    /// How the heading and the cells are aligned within the column.
    pub align: Align,
}

impl Column {
    /// Creates a left-aligned column without a title.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the column heading. The header row appears once any column has one.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets the alignment of the heading and the cells.
    pub fn with_align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }
}

/// One table cell: text, optionally toned, optionally followed by a [`Badge`].
///
/// Text is single-line: control characters are neutralised as everywhere else, and a
/// newline or tab becomes a space.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct Cell {
    /// The cell text.
    pub text: String,
    /// The tone the text is painted in; plain text when `None`.
    pub tone: Option<Tone>,
    /// A badge drawn after the text, such as `[locked]`.
    pub badge: Option<Badge>,
}

impl Cell {
    /// Creates a cell with plain text.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            tone: None,
            badge: None,
        }
    }

    /// Paints the text in `tone`.
    pub fn with_tone(mut self, tone: Tone) -> Self {
        self.tone = Some(tone);
        self
    }

    /// Adds a badge after the text, separated by a space.
    pub fn with_badge(mut self, badge: Badge) -> Self {
        self.badge = Some(badge);
        self
    }

    fn text(&self) -> String {
        single_line(&self.text)
    }

    fn badge_text(&self) -> Option<String> {
        self.badge
            .as_ref()
            .map(|badge| format!("[{}]", single_line(&badge.label)))
    }

    /// Display width of the text plus its badge.
    fn width(&self) -> usize {
        let badge = self.badge_text().map_or(0, |badge| 1 + badge.width());
        self.text().width() + badge
    }

    fn write_to(
        &self,
        console: Console,
        writer: &mut (impl Write + ?Sized),
    ) -> std::io::Result<()> {
        let text = self.text();
        match self.tone {
            Some(tone) => console.write_paint(tone, &text, writer)?,
            None => write!(writer, "{text}")?,
        }
        if let (Some(badge), Some(label)) = (&self.badge, self.badge_text()) {
            write!(writer, " ")?;
            console.write_paint(badge.tone, label, writer)?;
        }
        Ok(())
    }
}

impl From<&str> for Cell {
    fn from(text: &str) -> Self {
        Self::new(text)
    }
}

impl From<String> for Cell {
    fn from(text: String) -> Self {
        Self::new(text)
    }
}

/// An aligned table of rows and columns.
///
/// The column count is the larger of the declared columns and the longest row; a short
/// row leaves its remaining cells empty. Rows never end in trailing spaces, so plain
/// output compares cleanly in golden tests.
///
/// ```
/// use runemark::{Align, Badge, Cell, Column, ColorMode, Console, Table, Tone};
///
/// let table = Table::new()
///     .add_column(Column::new().with_align(Align::Right))
///     .add_row(["12", "Inbox"])
///     .add_row([Cell::new("3"), Cell::new("Archive").with_badge(Badge::new("locked", Tone::Warning))]);
///
/// let console = Console::new(ColorMode::Never, false);
/// assert_eq!(table.render(console), "12  Inbox\n 3  Archive [locked]\n");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Table {
    columns: Vec<Column>,
    rows: Vec<Vec<Cell>>,
}

impl Table {
    /// Creates an empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Declares the layout of the next column. Columns are declared left to right.
    pub fn add_column(mut self, column: Column) -> Self {
        self.columns.push(column);
        self
    }

    /// Adds a row. Anything that converts into a [`Cell`] works, so `["12", "Inbox"]`
    /// is a row of plain text.
    pub fn add_row<C: Into<Cell>>(mut self, cells: impl IntoIterator<Item = C>) -> Self {
        self.rows.push(cells.into_iter().map(Into::into).collect());
        self
    }

    /// Writes the table directly to a writer, one line per row.
    pub fn write_to(
        &self,
        console: Console,
        writer: &mut (impl Write + ?Sized),
    ) -> std::io::Result<()> {
        let count = self
            .rows
            .iter()
            .map(Vec::len)
            .max()
            .unwrap_or(0)
            .max(self.columns.len());
        let aligns: Vec<Align> = (0..count)
            .map(|index| self.columns.get(index).map_or(Align::Left, |c| c.align))
            .collect();
        let has_header = self.columns.iter().any(|column| column.title.is_some());

        let titles: Vec<Cell> = (0..count)
            .map(|index| {
                let title = self.columns.get(index).and_then(|c| c.title.clone());
                Cell::new(title.unwrap_or_default()).with_tone(Tone::Muted)
            })
            .collect();

        let mut widths = vec![0; count];
        for row in self.rows.iter().chain(has_header.then_some(&titles)) {
            for (index, cell) in row.iter().enumerate() {
                widths[index] = widths[index].max(cell.width());
            }
        }

        if has_header {
            write_row(&titles, &widths, &aligns, console, writer)?;
        }
        for row in &self.rows {
            write_row(row, &widths, &aligns, console, writer)?;
        }
        Ok(())
    }

    /// Renders the table as a String, one line per row.
    pub fn render(&self, console: Console) -> String {
        crate::internal::collect_to_string(|buf| self.write_to(console, buf))
    }
}

/// Writes one row, padding each cell to its column's width.
fn write_row(
    row: &[Cell],
    widths: &[usize],
    aligns: &[Align],
    console: Console,
    writer: &mut (impl Write + ?Sized),
) -> std::io::Result<()> {
    // Spaces owed before the next visible text. Padding after the last visible
    // cell is never written, so lines carry no trailing spaces.
    let mut owed = 0;
    for (index, (width, align)) in widths.iter().zip(aligns).enumerate() {
        let cell = row.get(index);
        let cell_width = cell.map_or(0, Cell::width);
        let slack = width - cell_width;
        let (lead, trail) = match align {
            Align::Left => (0, slack),
            Align::Right => (slack, 0),
        };
        owed += if index == 0 { 0 } else { GAP } + lead;
        if let Some(cell) = cell.filter(|_| cell_width > 0) {
            write!(writer, "{:owed$}", "")?;
            owed = 0;
            cell.write_to(console, writer)?;
        }
        owed += trail;
    }
    writeln!(writer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::ColorMode;

    fn plain() -> Console {
        Console::new(ColorMode::Never, false)
    }

    #[test]
    fn columns_are_as_wide_as_their_widest_cell() {
        let table = Table::new()
            .add_row(["a", "one"])
            .add_row(["long text", "two"]);
        assert_eq!(table.render(plain()), "a          one\nlong text  two\n");
    }

    #[test]
    fn right_alignment_pads_on_the_left() {
        let table = Table::new()
            .add_column(Column::new().with_align(Align::Right))
            .add_row(["7", "x"])
            .add_row(["1234", "y"]);
        assert_eq!(table.render(plain()), "   7  x\n1234  y\n");
    }

    #[test]
    fn wide_characters_are_measured_by_display_width() {
        // "日本語" is three characters but six columns wide, an emoji is two.
        let table = Table::new()
            .add_row(["日本語", "a"])
            .add_row(["abc", "b"])
            .add_row(["\u{1f4c1}", "c"]);
        assert_eq!(
            table.render(plain()),
            "日本語  a\nabc     b\n\u{1f4c1}      c\n"
        );
    }

    #[test]
    fn titles_make_a_header_row_aligned_like_the_column() {
        let table = Table::new()
            .add_column(Column::new().with_title("Count").with_align(Align::Right))
            .add_column(Column::new().with_title("Path"))
            .add_row(["3", "/Inbox"]);
        assert_eq!(table.render(plain()), "Count  Path\n    3  /Inbox\n");
    }

    #[test]
    fn a_badge_follows_the_text_and_counts_towards_the_width() {
        let table = Table::new()
            .add_row([
                Cell::new("Inbox").with_badge(Badge::new("locked", Tone::Warning)),
                Cell::new("work"),
            ])
            .add_row(["Archive", "home"]);
        assert_eq!(
            table.render(plain()),
            "Inbox [locked]  work\nArchive         home\n"
        );
    }

    #[test]
    fn lines_have_no_trailing_spaces_even_with_empty_cells() {
        let table = Table::new()
            .add_row(["a", "", ""])
            .add_row(["b", "", "end"])
            .add_row(["c"]);
        let out = table.render(plain());
        assert_eq!(out, "a\nb    end\nc\n");
        assert!(out.lines().all(|line| line == line.trim_end()));
    }

    #[test]
    fn a_row_longer_than_the_declared_columns_adds_left_aligned_columns() {
        let table = Table::new()
            .add_column(Column::new().with_align(Align::Right))
            .add_row(["1", "b", "c"])
            .add_row(["22", "bb", "cc"]);
        assert_eq!(table.render(plain()), " 1  b   c\n22  bb  cc\n");
    }

    #[test]
    fn tone_and_badge_are_painted_only_when_colour_is_on() {
        let table = Table::new().add_row([
            Cell::new("ok").with_tone(Tone::Success),
            Cell::new("x").with_badge(Badge::new("new", Tone::Info)),
        ]);
        let coloured = table.render(Console::new(ColorMode::Always, false));
        assert_eq!(coloured, "\x1b[1;92mok\x1b[0m  x \x1b[36m[new]\x1b[0m\n");
        assert_eq!(table.render(plain()), "ok  x [new]\n");
    }

    #[test]
    fn cell_text_is_single_line_and_control_characters_are_neutralised() {
        let table = Table::new().add_row(["a\nb\tc", "\x1b[31mred"]);
        assert_eq!(table.render(plain()), "a b c  ^[[31mred\n");
    }

    #[test]
    fn an_empty_table_writes_nothing() {
        assert_eq!(Table::new().render(plain()), "");
    }
}
