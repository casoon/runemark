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
        crate::internal::collect_to_string(|buf| self.write_frame(buf, console, None))
    }

    /// Writes the menu, marking `cursor` when one is given.
    fn write_frame(
        &self,
        writer: &mut (impl std::io::Write + ?Sized),
        console: Console,
        cursor: Option<usize>,
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
        let mut index = 0;

        for group in &self.groups {
            console.write_paint(Tone::Info, &group.label, writer)?;
            writeln!(writer)?;

            for item in &group.items {
                let marker = if cursor == Some(index) { "›" } else { " " };
                let padding = width.saturating_sub(item.label.width());
                let selected = cursor == Some(index);

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

                writeln!(writer)?;
                index += 1;
            }
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

    /// The number of lines [`Menu::write_frame`] produces, for redrawing in place.
    ///
    /// Only the interactive loop redraws, but the test that pins this against
    /// the real line count runs without the feature too.
    #[cfg(any(all(feature = "select", unix), test))]
    fn frame_height(&self) -> u16 {
        let heading = if self.heading.is_some() { 2 } else { 0 };
        let body = self.groups.len() + self.len();
        let hints = if self.hints.is_empty() { 0 } else { 2 };
        u16::try_from(heading + body + hints).unwrap_or(u16::MAX)
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

    #[test]
    fn frame_height_matches_the_rendered_line_count() {
        // Redrawing in place depends on this being exact; an off-by-one leaves
        // a stale line on screen or eats one above the menu.
        for candidate in [
            menu(),
            Menu::new().add_group(Group::new("G").add_item(Item::new("a", "a"))),
            Menu::new()
                .with_heading("H")
                .add_group(Group::new("G").add_item(Item::new("a", "a"))),
            menu().add_hint(Hint::new('C', "Clean")),
        ] {
            let lines = candidate.render(plain()).lines().count();
            assert_eq!(
                usize::from(candidate.frame_height()),
                lines,
                "for {candidate:?}"
            );
        }
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
