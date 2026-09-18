//! Interactive selection from a grouped list.
//!
//! This is the one place where Runemark reads from the terminal instead of only
//! writing to it. Everything else in the crate hands back a string or writes to
//! a writer the application owns; a menu cannot, because a cursor has to react
//! to keys.
//!
//! The boundary still holds in the direction that matters: a [`Menu`] carries
//! labels, descriptions and hints, and nothing about what the entries mean.
//! Grouping, ordering and wording stay with the application.
//!
//! Rendering is available without the `select` feature — [`Menu::render`] is
//! plain formatting. Only `Menu::run` needs the feature.
//!
//! The interactive path talks to the terminal directly: termios for raw mode,
//! four escape sequences for drawing, and a small parser for the handful of
//! keys a menu needs. That keeps the dependency to `libc` and makes the
//! feature Unix-only — on other platforms `Menu::run` reports
//! [`Outcome::Unavailable`] and the caller renders the list instead.

use std::fmt;

use unicode_width::UnicodeWidthStr;

use crate::color::{Console, Tone};

/// Controls whether a menu takes over the terminal.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum SelectMode {
    /// Interactive for a terminal, plain listing otherwise.
    #[default]
    Auto,
    /// Interactive even when the stream is not detected as a terminal.
    Always,
    /// Never interactive.
    Never,
}

impl SelectMode {
    /// Whether this mode should take over the terminal.
    pub const fn is_interactive(self, is_terminal: bool) -> bool {
        match self {
            Self::Auto => is_terminal,
            Self::Always => true,
            Self::Never => false,
        }
    }
}

/// One selectable entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// Returned on selection. The application's own identifier.
    pub id: String,
    /// The text shown in the list.
    pub label: String,
    /// Optional second column.
    pub description: Option<String>,
}

impl Item {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            description: None,
        }
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }
}

/// A titled section of entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub label: String,
    pub items: Vec<Item>,
}

impl Group {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            items: Vec::new(),
        }
    }

    pub fn add_item(mut self, item: Item) -> Self {
        self.items.push(item);
        self
    }
}

/// A key shown in the footer, reported back when pressed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    pub key: char,
    pub label: String,
}

impl Hint {
    pub fn new(key: char, label: impl Into<String>) -> Self {
        Self {
            key,
            label: label.into(),
        }
    }
}

/// What the user did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// An entry was chosen, carrying its [`Item::id`].
    Selected(String),
    /// A footer key was pressed.
    Hotkey(char),
    /// Escape, `q`, or `Ctrl-C`.
    Cancelled,
    /// The menu did not run, because the mode or the stream ruled it out.
    /// The caller decides what to show instead — often [`Menu::render`].
    Unavailable,
}

/// One drawn line of the menu body.
enum Row<'a> {
    Group(&'a str),
    /// An entry, with its position among selectable items.
    Item(&'a Item, usize),
}

/// A window over the body, for a terminal too short to show every entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Viewport {
    /// First body row to draw.
    start: usize,
    /// Body rows available, before any scroll indicator is subtracted.
    height: usize,
}

impl Viewport {
    pub(crate) const fn new(start: usize, height: usize) -> Self {
        Self { start, height }
    }

    /// Whether `row` lands inside what this viewport draws of `rows` rows.
    #[cfg(any(all(feature = "select", unix), test))]
    pub(crate) fn shows(self, rows: usize, row: usize) -> bool {
        self.window(rows).contains(&row)
    }

    /// The rows to draw, leaving room for whichever indicators are needed.
    ///
    /// An indicator costs a body line, and showing one can be what pushes the
    /// other into existence, so the two are resolved together rather than in
    /// sequence.
    fn window(self, rows: usize) -> std::ops::Range<usize> {
        if rows <= self.height {
            return 0..rows;
        }

        let start = self.start.min(rows.saturating_sub(1));
        let above = usize::from(start > 0);
        // Assume a trailing indicator, then confirm: with one line spent above
        // and one below, anything that still does not fit needs both.
        let visible = self.height.saturating_sub(above + 1).max(1);
        let end = (start + visible).min(rows);

        if end == rows {
            // Nothing below after all; that line goes back to the body.
            let visible = self.height.saturating_sub(above).max(1);
            let start = rows.saturating_sub(visible).max(start);
            return start..rows;
        }

        start..end
    }
}

/// A grouped list the user picks from.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Menu {
    heading: Option<String>,
    /// Shown at the end of the heading line, for context such as a detected tool.
    note: Option<String>,
    groups: Vec<Group>,
    hints: Vec<Hint>,
}

impl Menu {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_heading(mut self, heading: impl Into<String>) -> Self {
        self.heading = Some(heading.into());
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    pub fn add_group(mut self, group: Group) -> Self {
        self.groups.push(group);
        self
    }

    pub fn add_hint(mut self, hint: Hint) -> Self {
        self.hints.push(hint);
        self
    }

    /// Every item, in display order.
    pub fn items(&self) -> impl Iterator<Item = &Item> {
        self.groups.iter().flat_map(|group| group.items.iter())
    }

    /// The number of selectable entries.
    pub fn len(&self) -> usize {
        self.items().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The label column width, so descriptions line up across all groups.
    fn label_width(&self) -> usize {
        self.items()
            .map(|item| item.label.width())
            .max()
            .unwrap_or(0)
    }

    /// Renders the menu as plain text, with no cursor and no terminal control.
    ///
    /// This is what a non-interactive caller shows, and what tests assert on.
    pub fn render(&self, console: Console) -> String {
        crate::internal::collect_to_string(|buf| self.write_frame(buf, console, None, None))
    }

    /// The body as a flat list of lines, so a viewport can window over it.
    fn body_rows(&self) -> Vec<Row<'_>> {
        let mut rows = Vec::with_capacity(self.groups.len() + self.len());
        let mut index = 0;
        for group in &self.groups {
            rows.push(Row::Group(&group.label));
            for item in &group.items {
                rows.push(Row::Item(item, index));
                index += 1;
            }
        }
        rows
    }

    /// Writes the menu, marking `cursor` when one is given.
    ///
    /// `viewport` limits the body to a window, for a terminal that cannot show
    /// every entry. Without it the whole menu is written — which is what a
    /// pipe or a file wants, neither having a height to run out of.
    fn write_frame(
        &self,
        writer: &mut (impl std::io::Write + ?Sized),
        console: Console,
        cursor: Option<usize>,
        viewport: Option<Viewport>,
    ) -> std::io::Result<()> {
        if let Some(heading) = &self.heading {
            console.write_paint(Tone::Title, heading, writer)?;
            if let Some(note) = &self.note {
                write!(writer, "  ")?;
                console.write_paint(Tone::Muted, note, writer)?;
            }
            writeln!(writer)?;
            writeln!(writer)?;
        }

        let width = self.label_width();
        let rows = self.body_rows();
        let window = viewport.map_or(0..rows.len(), |viewport| viewport.window(rows.len()));

        if window.start > 0 {
            console.write_paint(Tone::Muted, format!("  ↑ {} more", window.start), writer)?;
            writeln!(writer)?;
        }

        for row in &rows[window.clone()] {
            match row {
                Row::Group(label) => {
                    console.write_paint(Tone::Info, label, writer)?;
                }
                Row::Item(item, index) => {
                    let selected = cursor == Some(*index);
                    let marker = if selected { "›" } else { " " };
                    let padding = width.saturating_sub(item.label.width());

                    write!(writer, "{marker} ")?;
                    console.write_paint(
                        if selected { Tone::Success } else { Tone::Info },
                        &item.label,
                        writer,
                    )?;

                    if let Some(description) = &item.description {
                        write!(writer, "{:padding$}  ", "")?;
                        console.write_paint(Tone::Muted, description, writer)?;
                    }
                }
            }
            writeln!(writer)?;
        }

        let remaining = rows.len() - window.end;
        if remaining > 0 {
            console.write_paint(Tone::Muted, format!("  ↓ {remaining} more"), writer)?;
            writeln!(writer)?;
        }

        if !self.hints.is_empty() {
            writeln!(writer)?;
            for (position, hint) in self.hints.iter().enumerate() {
                if position > 0 {
                    write!(writer, "   ")?;
                }
                console.write_paint(Tone::Success, hint.key, writer)?;
                write!(writer, " ")?;
                console.write_paint(Tone::Muted, &hint.label, writer)?;
            }
            writeln!(writer)?;
        }

        Ok(())
    }

    /// The body row showing item `index`, for keeping the cursor in view.
    #[cfg(any(all(feature = "select", unix), test))]
    fn row_of_item(&self, index: usize) -> usize {
        self.body_rows()
            .iter()
            .position(|row| matches!(row, Row::Item(_, item) if *item == index))
            .unwrap_or(0)
    }

    /// Body rows in total, for sizing a viewport.
    #[cfg(any(all(feature = "select", unix), test))]
    fn body_height(&self) -> usize {
        self.groups.len() + self.len()
    }

    /// Lines the menu spends on anything but the body.
    #[cfg(any(all(feature = "select", unix), test))]
    fn chrome_height(&self) -> usize {
        let heading = if self.heading.is_some() { 2 } else { 0 };
        let hints = if self.hints.is_empty() { 0 } else { 2 };
        heading + hints
    }
}

impl fmt::Display for Menu {
    /// Plain, uncoloured rendering.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.render(Console::new(crate::ColorMode::Never, false)))
    }
}

#[cfg(all(feature = "select", unix))]
mod interactive;
#[cfg(all(feature = "select", unix))]
mod terminal;

/// Without a terminal backend the menu still exists; it just never takes over.
///
/// The interactive path is Unix-only, so a caller can write one code path and
/// fall back to [`Menu::render`] on [`Outcome::Unavailable`] everywhere else.
#[cfg(all(feature = "select", not(unix)))]
impl Menu {
    pub fn run(
        &self,
        _console: Console,
        _mode: SelectMode,
        _is_terminal: bool,
    ) -> std::io::Result<Outcome> {
        Ok(Outcome::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ColorMode;

    fn plain() -> Console {
        Console::new(ColorMode::Never, false)
    }

    fn menu() -> Menu {
        Menu::new()
            .with_heading("casoon.dev")
            .with_note("pnpm")
            .add_group(
                Group::new("Development")
                    .add_item(Item::new("dev", "dev").with_description("Start the site"))
                    .add_item(Item::new("dev:landings", "dev:landings")),
            )
            .add_group(Group::new("Build").add_item(Item::new("build", "build")))
            .add_hint(Hint::new('U', "Updates"))
    }

    #[test]
    fn renders_groups_headings_and_hints() {
        let output = menu().render(plain());
        assert!(output.starts_with("casoon.dev  pnpm\n\n"));
        assert!(output.contains("Development\n"));
        assert!(output.contains("  dev "));
        assert!(output.contains("Build\n"));
        assert!(output.trim_end().ends_with("U Updates"));
    }

    #[test]
    fn plain_rendering_has_no_cursor_marker() {
        assert!(!menu().render(plain()).contains('›'));
    }

    #[test]
    fn descriptions_line_up_across_groups() {
        let output = menu().render(plain());
        let line = output
            .lines()
            .find(|line| line.contains("Start the site"))
            .expect("description line");
        // The label column is as wide as the longest label anywhere in the menu.
        assert_eq!(
            line.find("Start the site"),
            Some(2 + "dev:landings".width() + 2)
        );
    }

    #[test]
    fn an_item_without_a_description_ends_at_its_label() {
        let output = menu().render(plain());
        let line = output
            .lines()
            .find(|line| line.trim_start().starts_with("build"))
            .expect("build line");
        assert_eq!(line, "  build");
    }

    #[test]
    fn items_are_yielded_in_display_order() {
        let menu = menu();
        let ids: Vec<&str> = menu.items().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, ["dev", "dev:landings", "build"]);
    }

    #[test]
    fn length_counts_items_not_groups() {
        assert_eq!(menu().len(), 3);
        assert!(!menu().is_empty());
        assert!(Menu::new().is_empty());
    }

    fn windowed(menu: &Menu, start: usize, height: usize) -> String {
        crate::internal::collect_to_string(|buf| {
            menu.write_frame(buf, plain(), Some(0), Some(Viewport { start, height }))
        })
    }

    fn long_menu(items: usize) -> Menu {
        let mut group = Group::new("Scripts");
        for n in 0..items {
            group = group.add_item(Item::new(format!("t{n}"), format!("t{n}")));
        }
        Menu::new().with_heading("many").add_group(group)
    }

    #[test]
    fn a_body_that_fits_is_shown_whole() {
        let menu = long_menu(3);
        let output = windowed(&menu, 0, 50);
        assert!(!output.contains("more"));
        assert!(output.contains("t2"));
    }

    #[test]
    fn a_body_that_does_not_fit_says_how_much_is_below() {
        let menu = long_menu(40);
        let output = windowed(&menu, 0, 10);
        assert!(output.contains("↓ "));
        assert!(!output.contains("↑ "), "nothing is above the top");
    }

    #[test]
    fn scrolling_into_the_middle_shows_both_directions() {
        let menu = long_menu(40);
        let output = windowed(&menu, 15, 10);
        assert!(output.contains("↑ 15 more"));
        assert!(output.contains("↓ "));
    }

    #[test]
    fn the_end_of_the_list_drops_the_trailing_indicator() {
        let menu = long_menu(40);
        let output = windowed(&menu, 60, 10);
        assert!(output.contains("↑ "));
        assert!(
            !output.contains("↓ "),
            "there is nothing below the last row"
        );
        assert!(output.contains("t39"), "the last entry is visible");
    }

    #[test]
    fn a_window_never_draws_more_body_lines_than_it_was_given() {
        // The whole point: the frame must not outgrow the terminal, or the
        // redraw moves the cursor further up than there are lines.
        let menu = long_menu(40);
        for start in [0, 1, 7, 20, 39] {
            for height in [3, 5, 10, 25] {
                let body = windowed(&menu, start, height)
                    .lines()
                    .skip(2) // heading and its blank line
                    .count();
                assert!(
                    body <= height,
                    "start {start}, height {height}: drew {body} body lines"
                );
            }
        }
    }

    #[test]
    fn a_window_always_draws_something() {
        let menu = long_menu(40);
        for height in [1, 2, 3] {
            let body = windowed(&menu, 0, height).lines().skip(2).count();
            assert!(body >= 1, "height {height} drew nothing");
        }
    }

    #[test]
    fn row_lookup_accounts_for_group_labels() {
        let menu = menu();
        // Rows: Development, dev, dev:landings, Build, build
        assert_eq!(menu.row_of_item(0), 1);
        assert_eq!(menu.row_of_item(2), 4);
        assert_eq!(menu.body_height(), 5);
    }

    #[test]
    fn chrome_height_counts_heading_and_hints() {
        assert_eq!(menu().chrome_height(), 4);
        assert_eq!(Menu::new().chrome_height(), 0);
        assert_eq!(
            Menu::new().with_heading("h").chrome_height(),
            2,
            "heading plus its blank line"
        );
    }

    #[test]
    fn scrolling_keeps_every_cursor_position_in_view() {
        // The bug this pins: stepping to the last entry left the cursor one
        // row below the window, because the scroll maths and the window maths
        // disagreed about how many lines the indicators cost.
        let menu = long_menu(40);
        let rows = menu.body_height();

        for height in [4, 6, 11, 21, 30] {
            let mut start = 0usize;
            for index in 0..menu.len() {
                let cursor = menu.row_of_item(index);
                if cursor < start {
                    start = cursor;
                }
                while start < rows - 1 && !Viewport::new(start, height).shows(rows, cursor) {
                    start += 1;
                }
                assert!(
                    Viewport::new(start, height).shows(rows, cursor),
                    "height {height}, item {index} (row {cursor}) not visible from {start}"
                );
            }
        }
    }

    #[test]
    fn rendering_is_never_windowed() {
        // A pipe or a file has no height to run out of, so truncating there
        // would lose entries for no reason.
        let output = long_menu(40).render(plain());
        assert!(output.contains("t0") && output.contains("t39"));
        assert!(!output.contains("more"));
    }

    #[test]
    fn auto_mode_is_interactive_only_for_terminals() {
        assert!(SelectMode::Auto.is_interactive(true));
        assert!(!SelectMode::Auto.is_interactive(false));
        assert!(SelectMode::Always.is_interactive(false));
        assert!(!SelectMode::Never.is_interactive(true));
    }

    #[test]
    fn display_renders_without_colour() {
        let shown = menu().to_string();
        assert!(!shown.contains('\u{1b}'));
        assert_eq!(shown, menu().render(plain()));
    }

    #[cfg(feature = "select")]
    #[test]
    fn a_non_interactive_stream_returns_without_reading() {
        // The guarantee that matters in CI and in a pipeline: no blocking read.
        assert_eq!(
            menu().run(plain(), SelectMode::Auto, false).expect("run"),
            Outcome::Unavailable
        );
        assert_eq!(
            menu().run(plain(), SelectMode::Never, true).expect("run"),
            Outcome::Unavailable
        );
    }

    #[cfg(feature = "select")]
    #[test]
    fn an_empty_menu_never_takes_over_the_terminal() {
        assert_eq!(
            Menu::new()
                .run(plain(), SelectMode::Always, true)
                .expect("run"),
            Outcome::Unavailable
        );
    }
}
