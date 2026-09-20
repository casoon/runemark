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

/// How a menu arranges its groups.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Layout {
    /// Every group under its own heading, one list.
    #[default]
    Flat,
    /// Groups as a row of tabs, with only the active one's entries below.
    ///
    /// For a list that would otherwise be taller than the terminal. The
    /// application decides when that is — a menu knows how many entries it
    /// has, not how much of the screen its caller is willing to spend.
    ///
    /// Only the interactive path arranges itself this way. [`Menu::render`]
    /// lists every group, because nothing on the other end of a pipe can
    /// press a key to reach the second tab.
    Tabs,
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
    /// The tab this group shares with its neighbours, under [`Layout::Tabs`].
    ///
    /// Without one a group is its own tab. Consecutive groups naming the same
    /// tab collapse into it and keep their labels as headings inside it.
    pub tab: Option<String>,
    /// Whether the tab row draws a divider in front of this group's tab.
    pub divider: bool,
}

impl Group {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            items: Vec::new(),
            tab: None,
            divider: false,
        }
    }

    pub fn add_item(mut self, item: Item) -> Self {
        self.items.push(item);
        self
    }

    /// Puts this group in a shared tab rather than one of its own.
    ///
    /// For a run of groups that are the same kind of thing and would otherwise
    /// flood the tab row — a workspace's packages against a handful of
    /// actions. Inside the tab each keeps its label as a heading, so nothing
    /// about the grouping is lost; only the row gets its length back.
    pub fn in_tab(mut self, tab: impl Into<String>) -> Self {
        self.tab = Some(tab.into());
        self
    }

    /// Marks this group's tab as the start of a different kind of thing.
    ///
    /// A divider says the tabs on either side answer different questions —
    /// actions on one side, packages on the other — rather than being peers.
    pub fn with_divider(mut self) -> Self {
        self.divider = true;
        self
    }
}

/// One tab: a run of groups drawn as a single entry in the tab row.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Tab<'a> {
    label: &'a str,
    /// The groups it draws, as indices into the menu's own.
    groups: std::ops::Range<usize>,
    divider: bool,
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

/// Above this many entries, the filter is worth a line in the footer.
const SEARCH_WORTH_MENTIONING: usize = 5;

/// Columns the cursor marker and its trailing space occupy.
const MARKER_WIDTH: usize = 2;
/// Columns between the label column and the description.
const GAP_WIDTH: usize = 2;
/// Below this, a description says nothing and is left out instead.
const MIN_DESCRIPTION: usize = 12;
/// Columns between two tabs.
const TAB_GAP: usize = 3;
/// Columns between two keys in the footer.
const FOOTER_GAP: usize = 3;
/// What a dropped run of tabs costs to mark: the ellipsis and its gap.
const TAB_ELLIPSIS: usize = 1 + TAB_GAP;
/// What a divider between two tabs costs beyond the gap already there.
const TAB_DIVIDER: usize = 1 + TAB_GAP;
/// Tabs beyond this many have no digit left to select them; the arrow keys
/// still reach them.
const TAB_KEYS: usize = 9;

/// Shortens `text` to `max` columns, marking the cut with an ellipsis.
///
/// Entries are shortened rather than wrapped. A wrapped line would change the
/// number of lines the frame occupies, which the redraw counts on, and a
/// description spilling to column zero is what makes a long list unreadable in
/// the first place.
fn shorten(text: &str, max: usize) -> std::borrow::Cow<'_, str> {
    if text.width() <= max {
        return std::borrow::Cow::Borrowed(text);
    }
    if max <= 1 {
        return std::borrow::Cow::Borrowed("");
    }

    let mut out = String::new();
    let mut used = 0;
    for character in text.chars() {
        let next = character.to_string().width();
        if used + next > max - 1 {
            break;
        }
        out.push(character);
        used += next;
    }
    out.push('…');
    std::borrow::Cow::Owned(out)
}

/// The stretch of tabs to draw, always containing `active`.
///
/// Tabs are dropped from the ends rather than shortened: half a group name is
/// no longer the word its digit belongs to. The active tab is the seed and the
/// window grows outwards from it, so switching along the row scrolls it rather
/// than jumping the whole thing.
fn tab_window(
    labels: &[(String, bool)],
    active: usize,
    columns: Option<usize>,
) -> std::ops::Range<usize> {
    let all = 0..labels.len();
    let Some(columns) = columns else {
        return all;
    };
    let room = columns.saturating_sub(MARKER_WIDTH);

    // Measured the same way the row is written, dividers included, or a window
    // that fits on paper overflows on screen.
    let width = |range: &std::ops::Range<usize>| {
        let mut width = 0;
        for (drawn, index) in range.clone().enumerate() {
            let (label, divider) = &labels[index];
            if drawn > 0 {
                width += TAB_GAP;
                if *divider {
                    width += TAB_DIVIDER;
                }
            }
            width += label.width();
        }
        if range.start > 0 {
            width += TAB_ELLIPSIS;
        }
        if range.end < labels.len() {
            width += TAB_ELLIPSIS;
        }
        width
    };

    if width(&all) <= room {
        return all;
    }

    let mut window = active..active + 1;
    loop {
        let mut grew = false;
        if window.end < labels.len() {
            let wider = window.start..window.end + 1;
            if width(&wider) <= room {
                window = wider;
                grew = true;
            }
        }
        if window.start > 0 {
            let wider = window.start - 1..window.end;
            if width(&wider) <= room {
                window = wider;
                grew = true;
            }
        }
        if !grew {
            return window;
        }
    }
}

/// How the footer's keys break across lines at `columns`.
///
/// The footer is laid out rather than written straight out because a line that
/// wrapped would occupy more rows than the frame counted, and the redraw moves
/// the cursor up by that count — a wrapped footer leaves a stale line behind on
/// every keypress. Keys are never dropped to avoid it: an unmentioned key is a
/// key nobody presses, which is the problem the footer exists to solve.
fn footer_lines(keys: &[(String, &str)], columns: Option<usize>) -> Vec<std::ops::Range<usize>> {
    let all = 0..keys.len();
    let Some(columns) = columns else {
        return vec![all];
    };

    let width = |(key, label): &(String, &str)| key.width() + 1 + label.width();
    let mut lines = Vec::new();
    let mut start = 0;
    let mut used = 0;

    for (at, key) in keys.iter().enumerate() {
        let next = if at == start {
            width(key)
        } else {
            used + FOOTER_GAP + width(key)
        };
        // A single key wider than the terminal still gets its own line: there
        // is nowhere narrower to put it.
        if at > start && next > columns {
            lines.push(start..at);
            start = at;
            used = width(key);
        } else {
            used = next;
        }
    }
    if start < keys.len() {
        lines.push(start..keys.len());
    }
    lines
}

/// How well an item answers a search, lower being better.
///
/// The tiers matter more than the numbers: a name the user is typing beats a
/// description that happens to contain the same letters, and a run of adjacent
/// characters beats the same letters scattered through the name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Score(u32);

impl Score {
    const LABEL_SUBSTRING: u32 = 0;
    const LABEL_SUBSEQUENCE: u32 = 1_000;
    const DESCRIPTION: u32 = 10_000;
}

/// Scores `item` against a lowercased, non-empty `query`, or `None` if it does
/// not match.
fn score(item: &Item, query: &str) -> Option<Score> {
    let label = item.label.to_lowercase();
    if let Some(at) = label.find(query) {
        return Some(Score(
            Score::LABEL_SUBSTRING + u32::try_from(at).unwrap_or(u32::MAX),
        ));
    }
    // Typing "bl" for "build:landings" should still find it.
    if let Some(span) = subsequence_span(&label, query) {
        return Some(Score(
            Score::LABEL_SUBSEQUENCE + u32::try_from(span).unwrap_or(u32::MAX),
        ));
    }
    let description = item.description.as_ref()?.to_lowercase();
    let at = description.find(query)?;
    Some(Score(
        Score::DESCRIPTION + u32::try_from(at).unwrap_or(u32::MAX),
    ))
}

/// The span `query` occupies in `text` as a subsequence, if it occurs at all.
///
/// The span is what separates a tight match from a lucky one: `dl` spans two
/// characters in `dl-report` and fourteen in `deploy:landings`.
fn subsequence_span(text: &str, query: &str) -> Option<usize> {
    let mut chars = text.char_indices();
    let mut first = None;
    let mut last = 0;

    for wanted in query.chars() {
        let (at, _) = chars.find(|(_, character)| *character == wanted)?;
        first.get_or_insert(at);
        last = at;
    }

    Some(last - first.unwrap_or(last) + 1)
}

/// What the body is showing: a search, a tab, or everything.
#[derive(Debug, Clone, Copy, Default)]
struct View<'a> {
    query: Option<&'a str>,
    /// The active tab, or `None` for the flat listing. A non-interactive
    /// caller always passes `None`: it cannot switch tabs, so it is shown
    /// every group.
    tab: Option<usize>,
}

/// One drawn line of the menu body.
enum Row<'a> {
    Group(&'a str),
    /// An entry, with its position among selectable items.
    Item(&'a Item, usize),
}

/// The visible area, for a terminal that cannot show the whole menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Viewport {
    /// First body row to draw.
    start: usize,
    /// Body rows available, before any scroll indicator is subtracted.
    height: usize,
    /// Columns available. Entries are shortened to fit rather than wrapped:
    /// a wrapped line would change the frame height and break the redraw.
    width: Option<usize>,
}

impl Viewport {
    pub(crate) const fn new(start: usize, height: usize) -> Self {
        Self {
            start,
            height,
            width: None,
        }
    }

    pub(crate) const fn with_width(mut self, width: Option<usize>) -> Self {
        self.width = width;
        self
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
    /// A second heading line, for what the menu counted up.
    summary: Option<String>,
    groups: Vec<Group>,
    hints: Vec<Hint>,
    layout: Layout,
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

    /// A second heading line, under the first.
    ///
    /// For what the menu adds up to — how many entries, how many groups —
    /// which the list itself only says by being counted.
    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = Some(summary.into());
        self
    }

    /// How the groups are arranged. See [`Layout`].
    pub fn with_layout(mut self, layout: Layout) -> Self {
        self.layout = layout;
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

    /// Which tab `view` is showing, or `None` for the flat listing.
    ///
    /// A single group is no reason for a tab row: there is nothing to switch
    /// to, and the row would cost two lines to say what the heading says.
    fn active_tab(&self, view: View<'_>) -> Option<usize> {
        if self.layout != Layout::Tabs || view.query.is_some() {
            return None;
        }
        let tabs = self.tabs().len();
        if tabs < 2 {
            return None;
        }
        // A tab index outside the menu is clamped rather than refused: the
        // caller holds it across redraws, and a menu may be rebuilt smaller.
        Some(view.tab?.min(tabs - 1))
    }

    /// The tab row, derived from the groups.
    ///
    /// A group with no [`Group::tab`] is its own tab. Consecutive groups
    /// naming the same one collapse into it, and the first of the run decides
    /// whether a divider precedes it.
    fn tabs(&self) -> Vec<Tab<'_>> {
        let mut tabs: Vec<Tab<'_>> = Vec::with_capacity(self.groups.len());
        for (index, group) in self.groups.iter().enumerate() {
            let label = group.tab.as_deref().unwrap_or(&group.label);
            match tabs.last_mut() {
                Some(last) if group.tab.is_some() && last.label == label => {
                    last.groups.end = index + 1;
                }
                _ => tabs.push(Tab {
                    label,
                    groups: index..index + 1,
                    divider: group.divider,
                }),
            }
        }
        tabs
    }

    /// How many tabs there are, for the interactive path's arithmetic.
    #[cfg(any(all(feature = "select", unix), test))]
    fn tab_count(&self) -> usize {
        self.tabs().len()
    }

    /// Each tab as drawn, with the digit that selects it.
    fn tab_labels(&self) -> Vec<(String, bool)> {
        self.tabs()
            .into_iter()
            .enumerate()
            .map(|(index, tab)| {
                let label = if index < TAB_KEYS {
                    format!("{} {}", index + 1, tab.label)
                } else {
                    tab.label.to_owned()
                };
                (label, tab.divider)
            })
            .collect()
    }

    /// Writes the tab row and the rule marking the active tab.
    ///
    /// The rule is not decoration. With colour off it is the only thing on the
    /// screen saying which group the entries below belong to.
    fn write_tabs(
        &self,
        writer: &mut (impl std::io::Write + ?Sized),
        console: Console,
        active: usize,
        columns: Option<usize>,
    ) -> std::io::Result<()> {
        let labels = self.tab_labels();
        let window = tab_window(&labels, active, columns);

        // The row lines up with the entry labels below it, past the marker.
        let mut column = MARKER_WIDTH;
        let mut underline = None;
        write!(writer, "{:MARKER_WIDTH$}", "")?;

        if window.start > 0 {
            console.write_paint(Tone::Muted, "…", writer)?;
            write!(writer, "{:TAB_GAP$}", "")?;
            column += TAB_ELLIPSIS;
        }
        for index in window.clone() {
            let (label, divider) = &labels[index];
            if index > window.start {
                write!(writer, "{:TAB_GAP$}", "")?;
                column += TAB_GAP;
                // Only between two drawn tabs. Leading a row with a divider
                // would separate the tabs from nothing, and after an ellipsis
                // the break is already marked.
                if *divider {
                    console.write_paint(Tone::Muted, "│", writer)?;
                    write!(writer, "{:TAB_GAP$}", "")?;
                    column += TAB_DIVIDER;
                }
            }
            if index == active {
                console.write_paint(Tone::Title, label, writer)?;
                underline = Some((column, label.width()));
            } else {
                console.write_paint(Tone::Muted, label, writer)?;
            }
            column += label.width();
        }
        if window.end < labels.len() {
            write!(writer, "{:TAB_GAP$}", "")?;
            console.write_paint(Tone::Muted, "…", writer)?;
        }
        writeln!(writer)?;

        if let Some((at, width)) = underline {
            write!(writer, "{:at$}", "")?;
            console.write_paint(Tone::Success, "─".repeat(width), writer)?;
        }
        writeln!(writer)
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
        crate::internal::collect_to_string(|buf| {
            self.write_frame(buf, console, None, None, View::default())
        })
    }

    /// The body as a flat list of lines, so a viewport can window over it.
    ///
    /// With a `query`, only matching items appear, best first, and a group with
    /// nothing left disappears with them. Item indices are positions in that
    /// filtered order, which is what the cursor counts.
    fn body_rows(&self, view: View<'_>) -> Vec<Row<'_>> {
        // An empty query is not a search result: ranking it would sort the menu
        // alphabetically, which is the arrangement the grouping exists to avoid.
        let query = view.query.filter(|query| !query.is_empty());

        let Some(query) = query else {
            if let Some(active) = self.active_tab(view) {
                let tab = self.tabs().swap_remove(active);
                let mut rows = Vec::new();
                let mut index = 0;
                for group in &self.groups[tab.groups] {
                    // A group that gave the tab its name carries no heading:
                    // the row above says it already, and repeating it spends a
                    // line on the screen that ran out of lines. One sharing a
                    // tab keeps its heading, or the tab would be a flat run of
                    // entries from several packages with nothing telling them
                    // apart.
                    if group.label != tab.label {
                        rows.push(Row::Group(&group.label));
                    }
                    for item in &group.items {
                        rows.push(Row::Item(item, index));
                        index += 1;
                    }
                }
                return rows;
            }

            let mut rows = Vec::with_capacity(self.groups.len() + self.len());
            let mut index = 0;
            for group in &self.groups {
                rows.push(Row::Group(&group.label));
                for item in &group.items {
                    rows.push(Row::Item(item, index));
                    index += 1;
                }
            }
            return rows;
        };

        // Ranking across the whole menu, not within each group: the best answer
        // to what was typed should be the first thing the cursor sits on.
        let mut ranked: Vec<(Score, &str, &Item)> =
            self.groups
                .iter()
                .flat_map(|group| {
                    group.items.iter().filter_map(move |item| {
                        Some((score(item, query)?, group.label.as_str(), item))
                    })
                })
                .collect();
        ranked.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.2.label.cmp(&b.2.label)));

        let mut rows = Vec::with_capacity(ranked.len() + 1);
        let mut last_group = None;
        for (index, (_, group, item)) in ranked.iter().enumerate() {
            if last_group != Some(*group) {
                rows.push(Row::Group(group));
                last_group = Some(*group);
            }
            rows.push(Row::Item(item, index));
        }
        rows
    }

    /// The items a `query` matches, in the order they are drawn.
    fn matching_items(&self, view: View<'_>) -> Vec<&Item> {
        self.body_rows(view)
            .into_iter()
            .filter_map(|row| match row {
                Row::Item(item, _) => Some(item),
                Row::Group(_) => None,
            })
            .collect()
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
        view: View<'_>,
    ) -> std::io::Result<()> {
        let columns = viewport.and_then(|viewport| viewport.width);
        let query = view.query;

        if let Some(heading) = &self.heading {
            let note_room = self
                .note
                .as_ref()
                .map_or(0, |note| note.width() + GAP_WIDTH);
            let room = columns.map_or(usize::MAX, |columns| columns.saturating_sub(note_room));
            console.write_paint(Tone::Title, shorten(heading, room), writer)?;
            if let Some(note) = &self.note {
                write!(writer, "  ")?;
                console.write_paint(Tone::Muted, note, writer)?;
            }
            writeln!(writer)?;
        }
        if let Some(summary) = &self.summary {
            let room = columns.map_or(usize::MAX, |columns| columns.saturating_sub(MARKER_WIDTH));
            write!(writer, "{:MARKER_WIDTH$}", "")?;
            console.write_paint(Tone::Muted, shorten(summary, room), writer)?;
            writeln!(writer)?;
        }
        if self.heading.is_some() || self.summary.is_some() {
            writeln!(writer)?;
        }

        if let Some(active) = self.active_tab(view) {
            self.write_tabs(writer, console, active, columns)?;
        }

        let width = self.label_width().min(
            // A label column wider than the terminal leaves nothing for the
            // description and pushes it off screen entirely.
            columns.map_or(usize::MAX, |columns| columns.saturating_sub(MARKER_WIDTH)),
        );
        let rows = self.body_rows(view);
        let window = viewport.map_or(0..rows.len(), |viewport| viewport.window(rows.len()));

        if window.start > 0 {
            console.write_paint(Tone::Muted, format!("  ↑ {} more", window.start), writer)?;
            writeln!(writer)?;
        }

        for row in &rows[window.clone()] {
            match row {
                Row::Group(label) => {
                    let room = columns.unwrap_or(usize::MAX);
                    console.write_paint(Tone::Info, shorten(label, room), writer)?;
                }
                Row::Item(item, index) => {
                    let selected = cursor == Some(*index);
                    let marker = if selected { "›" } else { " " };
                    let label = shorten(&item.label, width);
                    let padding = width.saturating_sub(label.width());

                    write!(writer, "{marker} ")?;
                    console.write_paint(
                        if selected { Tone::Success } else { Tone::Info },
                        &label,
                        writer,
                    )?;

                    if let Some(description) = &item.description {
                        // What is left after the marker, the label column and
                        // the gap. Below a readable minimum the description is
                        // dropped rather than cut to a stub.
                        let room = columns.map_or(usize::MAX, |columns| {
                            columns.saturating_sub(MARKER_WIDTH + width + GAP_WIDTH)
                        });
                        if room >= MIN_DESCRIPTION {
                            write!(writer, "{:padding$}  ", "")?;
                            console.write_paint(Tone::Muted, shorten(description, room), writer)?;
                        }
                    }
                }
            }
            writeln!(writer)?;
        }

        if rows.is_empty() && query.is_some() {
            console.write_paint(Tone::Muted, "  no matches", writer)?;
            writeln!(writer)?;
        }

        let remaining = rows.len() - window.end;
        if remaining > 0 {
            console.write_paint(Tone::Muted, format!("  ↓ {remaining} more"), writer)?;
            writeln!(writer)?;
        }

        if let Some(query) = query {
            writeln!(writer)?;
            console.write_paint(Tone::Success, "/", writer)?;
            write!(writer, " ")?;
            // Two columns went to "/ ". What is left bounds the query, so a
            // long one cannot wrap and throw the redraw's line count off.
            let room = columns.map_or(usize::MAX, |columns| columns.saturating_sub(2));
            if query.is_empty() {
                console.write_paint(Tone::Muted, shorten("type to filter", room), writer)?;
            } else {
                console.write_paint(Tone::Title, shorten(query, room), writer)?;
            }
            writeln!(writer)?;
        } else {
            let keys = self.footer_keys(view, cursor);
            if !keys.is_empty() {
                writeln!(writer)?;
                for line in footer_lines(&keys, columns) {
                    for (at, (key, label)) in keys[line].iter().enumerate() {
                        if at > 0 {
                            write!(writer, "{:FOOTER_GAP$}", "")?;
                        }
                        console.write_paint(Tone::Success, key, writer)?;
                        write!(writer, " ")?;
                        console.write_paint(Tone::Muted, label, writer)?;
                    }
                    writeln!(writer)?;
                }
            }
        }

        Ok(())
    }

    /// The keys the footer offers, in the order they are shown.
    ///
    /// `/` is reserved for the filter and cannot be bound as a hint, so
    /// nothing else can advertise it. A key the menu answers to but never
    /// mentions is a key nobody presses.
    fn footer_keys(&self, view: View<'_>, cursor: Option<usize>) -> Vec<(String, &str)> {
        let mut keys = Vec::with_capacity(self.hints.len() + 2);
        if self.active_tab(view).is_some() {
            keys.push(("\u{2190}\u{2192}".to_owned(), "group"));
        }
        if self.offers_search(cursor) {
            keys.push(("/".to_owned(), "search"));
        }
        for hint in &self.hints {
            keys.push((hint.key.to_string(), hint.label.as_str()));
        }
        keys
    }

    /// Whether this frame should advertise the filter.
    ///
    /// A cursor means the menu is being driven from a keyboard; `render` passes
    /// none, and offering a key to a pipe would be a lie.
    ///
    /// Short menus do not advertise it. Filtering still works — the key is
    /// never taken away — but a yes/no question that offers to search itself
    /// reads as clutter, and below a handful of entries every one of them is
    /// already on screen.
    fn offers_search(&self, cursor: Option<usize>) -> bool {
        cursor.is_some() && self.len() > SEARCH_WORTH_MENTIONING
    }

    /// The body row showing item `index`, for keeping the cursor in view.
    #[cfg(any(all(feature = "select", unix), test))]
    fn row_of_item(&self, index: usize, view: View<'_>) -> usize {
        self.body_rows(view)
            .iter()
            .position(|row| matches!(row, Row::Item(_, item) if *item == index))
            .unwrap_or(0)
    }

    /// Body rows in total, for sizing a viewport.
    #[cfg(any(all(feature = "select", unix), test))]
    fn body_height(&self, view: View<'_>) -> usize {
        // The "no matches" line occupies the body when nothing is left.
        self.body_rows(view)
            .len()
            .max(usize::from(view.query.is_some()))
    }

    /// Lines the menu spends on anything but the body.
    #[cfg(any(all(feature = "select", unix), test))]
    fn chrome_height(&self, view: View<'_>, columns: Option<usize>) -> usize {
        let lines = usize::from(self.heading.is_some()) + usize::from(self.summary.is_some());
        // Whatever the heading block wrote, plus the blank line after it.
        let heading = if lines > 0 { lines + 1 } else { 0 };
        // The tab row and the rule under the active tab.
        let tabs = usize::from(self.active_tab(view).is_some()) * 2;
        // The footer is measured with the same layout that writes it. Guessing
        // one line where two get written is what makes the redraw eat a row.
        let footer = if view.query.is_some() {
            2
        } else {
            let keys = self.footer_keys(view, Some(0));
            if keys.is_empty() {
                0
            } else {
                1 + footer_lines(&keys, columns).len()
            }
        };
        heading + tabs + footer
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

    /// The flat listing: every group, no search.
    fn flat() -> View<'static> {
        View::default()
    }

    fn searching(query: &str) -> View<'_> {
        View {
            query: Some(query),
            tab: None,
        }
    }

    fn on_tab(tab: usize) -> View<'static> {
        View {
            query: None,
            tab: Some(tab),
        }
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
    fn a_keyboard_frame_advertises_the_filter() {
        // `/` cannot be bound as a hint, so if the menu does not mention it,
        // nothing will.
        let menu = long_menu(20).add_hint(Hint::new('U', "Updates"));
        let interactive = crate::internal::collect_to_string(|buf| {
            menu.write_frame(buf, plain(), Some(0), None, flat())
        });
        assert!(interactive.contains("/ search"));
        assert!(
            interactive.contains("U Updates"),
            "and the menu's own hints"
        );
    }

    #[test]
    fn a_rendered_frame_offers_no_keys() {
        // Printed to a pipe there is no keyboard, so offering one would lie.
        assert!(!menu().render(plain()).contains("/ search"));
    }

    #[test]
    fn an_empty_menu_advertises_nothing() {
        let empty = Menu::new().with_heading("nothing");
        let shown = crate::internal::collect_to_string(|buf| {
            empty.write_frame(buf, plain(), Some(0), None, flat())
        });
        assert!(!shown.contains("search"));
    }

    #[test]
    fn a_short_menu_does_not_offer_to_search_itself() {
        // A yes/no question advertising a filter reads as clutter.
        let confirm = Menu::new().with_heading("Run deploy?").add_group(
            Group::new("Confirm")
                .add_item(Item::new("no", "Cancel"))
                .add_item(Item::new("yes", "Run deploy")),
        );
        let shown = crate::internal::collect_to_string(|buf| {
            confirm.write_frame(buf, plain(), Some(0), None, flat())
        });
        assert!(!shown.contains("search"));
    }

    #[test]
    fn filtering_a_short_menu_still_works() {
        // Not advertising the key is not the same as removing it.
        let short = long_menu(3);
        assert_eq!(searched(&short, "t2"), ["t2"]);
        let shown = crate::internal::collect_to_string(|buf| {
            short.write_frame(buf, plain(), Some(0), None, searching("t2"))
        });
        assert!(shown.contains("/ t2"));
    }

    #[test]
    fn a_long_menu_still_advertises_it() {
        let long = long_menu(20);
        let shown = crate::internal::collect_to_string(|buf| {
            long.write_frame(buf, plain(), Some(0), None, flat())
        });
        assert!(shown.contains("/ search"));
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
            menu.write_frame(
                buf,
                plain(),
                Some(0),
                Some(Viewport::new(start, height)),
                flat(),
            )
        })
    }

    fn at_width(menu: &Menu, columns: usize) -> String {
        crate::internal::collect_to_string(|buf| {
            menu.write_frame(
                buf,
                plain(),
                Some(0),
                Some(Viewport::new(0, 999).with_width(Some(columns))),
                flat(),
            )
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
        let chrome = menu.chrome_height(flat(), None);
        for start in [0, 1, 7, 20, 39] {
            for height in [3, 5, 10, 25] {
                let body = windowed(&menu, start, height).lines().count() - chrome;
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
        let chrome = menu.chrome_height(flat(), None);
        for height in [1, 2, 3] {
            let body = windowed(&menu, 0, height).lines().count() - chrome;
            assert!(body >= 1, "height {height} drew nothing");
        }
    }

    #[test]
    fn row_lookup_accounts_for_group_labels() {
        let menu = menu();
        // Rows: Development, dev, dev:landings, Build, build
        assert_eq!(menu.row_of_item(0, flat()), 1);
        assert_eq!(menu.row_of_item(2, flat()), 4);
        assert_eq!(menu.body_height(flat()), 5);
    }

    #[test]
    fn chrome_height_counts_heading_and_hints() {
        assert_eq!(menu().chrome_height(flat(), None), 4);
        assert_eq!(Menu::new().chrome_height(flat(), None), 0);
        assert_eq!(
            Menu::new().with_heading("h").chrome_height(flat(), None),
            2,
            "heading plus its blank line"
        );
        assert_eq!(
            Menu::new().chrome_height(searching(""), None),
            2,
            "the query line needs room even without hints"
        );
    }

    #[test]
    fn scrolling_keeps_every_cursor_position_in_view() {
        // The bug this pins: stepping to the last entry left the cursor one
        // row below the window, because the scroll maths and the window maths
        // disagreed about how many lines the indicators cost.
        let menu = long_menu(40);
        let rows = menu.body_height(flat());

        for height in [4, 6, 11, 21, 30] {
            let mut start = 0usize;
            for index in 0..menu.len() {
                let cursor = menu.row_of_item(index, flat());
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
    fn nothing_exceeds_the_given_width() {
        // The defect this pins: without a width, a long description wrapped to
        // column zero and destroyed the two-column layout.
        let menu = Menu::new()
            .with_heading("a-rather-long-project-name")
            .with_note("pnpm")
            .add_group(Group::new("Quality").add_item(
                Item::new("type-check", "type-check").with_description(
                    "Führt den TypeScript-Check in allen Packages des Workspace aus",
                ),
            ));

        for columns in [20, 40, 60, 80, 100] {
            for line in at_width(&menu, columns).lines() {
                assert!(
                    line.width() <= columns,
                    "width {columns}: line of {} columns: {line:?}",
                    line.width()
                );
            }
        }
    }

    #[test]
    fn a_shortened_entry_is_marked_as_cut() {
        let menu = Menu::new().add_group(Group::new("G").add_item(
            Item::new("x", "x").with_description("eine sehr lange Beschreibung, die nicht passt"),
        ));
        assert!(at_width(&menu, 30).contains('…'));
    }

    #[test]
    fn a_description_with_no_room_is_dropped_rather_than_stubbed() {
        let menu = Menu::new().add_group(Group::new("G").add_item(
            Item::new("a-long-script-name", "a-long-script-name").with_description("beschreibung"),
        ));
        let narrow = at_width(&menu, 24);
        assert!(!narrow.contains("besch"), "no room left, so no description");
        assert!(
            narrow.contains("a-long-script-name"),
            "the name still shows"
        );
    }

    #[test]
    fn shortening_counts_display_columns_not_bytes() {
        // German descriptions are the normal case here; multi-byte characters
        // must not be counted twice.
        assert_eq!(shorten("äöüß", 10), "äöüß");
        assert_eq!(shorten("äöüß", 3).width(), 3);
        assert!(shorten("äöüß", 3).ends_with('…'));
        assert_eq!(shorten("abc", 1), "");
    }

    fn searched(menu: &Menu, query: &str) -> Vec<String> {
        menu.matching_items(searching(query))
            .into_iter()
            .map(|item| item.label.clone())
            .collect()
    }

    fn script_menu() -> Menu {
        Menu::new()
            .add_group(
                Group::new("Development")
                    .add_item(Item::new("dev", "dev").with_description("Start the site"))
                    .add_item(Item::new("dev:landings", "dev:landings")),
            )
            .add_group(
                Group::new("Deploy")
                    .add_item(Item::new("deploy", "deploy").with_description("Ship everything"))
                    .add_item(Item::new("deploy:landings", "deploy:landings")),
            )
            .add_group(
                Group::new("Quality")
                    .add_item(Item::new("check", "check").with_description("Lint and format")),
            )
    }

    #[test]
    fn an_empty_query_keeps_the_menu_as_it_was() {
        // Backspacing a query away must restore the meaning-first order, not
        // leave the menu sorted alphabetically by a rank everything ties on.
        let menu = script_menu();
        let unsearched: Vec<String> = menu.items().map(|item| item.label.clone()).collect();
        assert_eq!(searched(&menu, ""), unsearched);
    }

    #[test]
    fn a_substring_in_the_name_wins_over_one_in_a_description() {
        // "s" appears in "Start the site" and in "deploy:landings"; the name
        // is what the user is typing towards.
        let hits = searched(&script_menu(), "landings");
        assert_eq!(hits, ["dev:landings", "deploy:landings"]);
    }

    #[test]
    fn scattered_letters_still_find_a_name() {
        assert!(searched(&script_menu(), "dpl").contains(&"deploy".to_owned()));
    }

    #[test]
    fn a_tight_match_ranks_before_a_scattered_one() {
        let hits = searched(&script_menu(), "dep");
        assert_eq!(hits.first().map(String::as_str), Some("deploy"));
    }

    #[test]
    fn a_description_match_is_found_when_no_name_matches() {
        let hits = searched(&script_menu(), "lint");
        assert_eq!(hits, ["check"]);
    }

    #[test]
    fn a_query_that_matches_nothing_yields_nothing() {
        assert!(searched(&script_menu(), "qqqq").is_empty());
    }

    #[test]
    fn a_query_that_matches_nothing_says_so() {
        // An empty area under a query reads as a broken menu rather than an
        // answer.
        let menu = script_menu();
        let shown = crate::internal::collect_to_string(|buf| {
            menu.write_frame(buf, plain(), Some(0), None, searching("qqqq"))
        });
        assert!(shown.contains("no matches"));
        assert_eq!(
            menu.body_height(searching("qqqq")),
            1,
            "the notice needs a line"
        );
    }

    #[test]
    fn searching_is_case_insensitive() {
        assert_eq!(
            searched(&script_menu(), "dev"),
            searched(&script_menu(), "dev")
        );
        assert!(
            !searched(&script_menu(), "start").is_empty(),
            "matches a capitalised description"
        );
    }

    #[test]
    fn a_group_with_no_matches_is_not_drawn() {
        let menu = script_menu();
        let rows = menu.body_rows(searching("check"));
        let groups: Vec<&str> = rows
            .iter()
            .filter_map(|row| match row {
                Row::Group(label) => Some(*label),
                Row::Item(..) => None,
            })
            .collect();
        assert_eq!(groups, ["Quality"]);
    }

    #[test]
    fn filtered_item_indices_are_positions_in_the_result() {
        let menu = script_menu();
        let rows = menu.body_rows(searching("landings"));
        let indices: Vec<usize> = rows
            .iter()
            .filter_map(|row| match row {
                Row::Item(_, index) => Some(*index),
                Row::Group(_) => None,
            })
            .collect();
        assert_eq!(indices, [0, 1], "the cursor counts matches, not all items");
    }

    fn tabbed(groups: usize) -> Menu {
        let mut menu = Menu::new()
            .with_heading("web-casoon")
            .with_layout(Layout::Tabs);
        for n in 0..groups {
            menu = menu.add_group(
                Group::new(format!("Group{n}"))
                    .add_item(Item::new(format!("a{n}"), format!("a{n}")))
                    .add_item(Item::new(format!("b{n}"), format!("b{n}"))),
            );
        }
        menu
    }

    fn framed(menu: &Menu, view: View<'_>) -> String {
        crate::internal::collect_to_string(|buf| {
            menu.write_frame(buf, plain(), Some(0), None, view)
        })
    }

    #[test]
    fn a_tab_shows_only_its_own_group() {
        let menu = tabbed(3);
        let shown = framed(&menu, on_tab(1));
        assert!(shown.contains("a1") && shown.contains("b1"));
        assert!(!shown.contains("a0"), "the other tabs' entries stay hidden");
        assert!(!shown.contains("a2"));
    }

    #[test]
    fn a_tab_repeats_no_group_heading() {
        // The tab row already names the group; a heading under it would spend
        // a line saying so twice on the screen that ran out of lines.
        let menu = tabbed(3);
        let rows = menu.body_rows(on_tab(0));
        assert!(rows.iter().all(|row| matches!(row, Row::Item(..))));
    }

    #[test]
    fn a_tab_counts_its_own_entries_from_zero() {
        let indices: Vec<usize> = tabbed(3)
            .body_rows(on_tab(2))
            .iter()
            .filter_map(|row| match row {
                Row::Item(_, index) => Some(*index),
                Row::Group(_) => None,
            })
            .collect();
        assert_eq!(indices, [0, 1], "the cursor counts what is on screen");
    }

    #[test]
    fn a_pipe_is_shown_every_group_despite_the_tab_layout() {
        // Nothing on the other end of a pipe can press a key to reach the
        // second tab, so hiding it there would lose entries.
        let shown = tabbed(3).render(plain());
        for n in 0..3 {
            assert!(shown.contains(&format!("a{n}")), "group {n} is listed");
        }
        assert!(!shown.contains("───"), "and no tab row is drawn");
    }

    #[test]
    fn the_active_tab_is_marked_without_colour() {
        // With colour off the rule is the only thing saying which group the
        // entries below belong to.
        let lines: Vec<String> = framed(&tabbed(3), on_tab(1))
            .lines()
            .map(str::to_owned)
            .collect();
        let row = lines.iter().position(|l| l.contains("2 Group1")).unwrap();
        let rule = &lines[row + 1];
        let at = lines[row].find("2 Group1").unwrap();
        assert_eq!(
            rule.find('─'),
            Some(at),
            "the rule starts under the active tab"
        );
        assert_eq!(rule.trim().chars().count(), "2 Group1".width());
    }

    #[test]
    fn tabs_are_numbered_up_to_nine() {
        let shown = framed(&tabbed(11), on_tab(0));
        let row = shown.lines().find(|l| l.contains("1 Group0")).unwrap();
        assert!(row.contains("9 Group8"));
        assert!(
            !row.contains("10 Group9"),
            "past nine there is no digit left to offer"
        );
    }

    #[test]
    fn one_group_gets_no_tab_row() {
        // There is nothing to switch to, and the row would cost two lines to
        // repeat the heading.
        let menu = Menu::new()
            .with_heading("one")
            .with_layout(Layout::Tabs)
            .add_group(Group::new("Scripts").add_item(Item::new("dev", "dev")));
        assert!(menu.active_tab(on_tab(0)).is_none());
        assert!(framed(&menu, on_tab(0)).contains("Scripts"), "as a heading");
    }

    #[test]
    fn a_tab_index_past_the_end_is_clamped() {
        // The caller holds the index across redraws, and a menu may be rebuilt
        // smaller between them.
        assert_eq!(tabbed(3).active_tab(on_tab(9)), Some(2));
    }

    #[test]
    fn searching_leaves_the_tabs_and_spans_all_of_them() {
        let menu = tabbed(3);
        assert!(menu.active_tab(searching("a")).is_none());
        let shown = framed(&menu, searching("a"));
        for n in 0..3 {
            assert!(
                shown.contains(&format!("a{n}")),
                "group {n} is searched too"
            );
        }
    }

    /// Action tabs, then a run of packages sharing one, as opi builds it.
    fn mixed(packages: usize) -> Menu {
        let mut menu = tabbed(3).with_layout(Layout::Tabs);
        for n in 0..packages {
            let mut group = Group::new(format!("@scope/pkg{n}"))
                .in_tab("Packages")
                .add_item(Item::new(format!("p{n}"), format!("p{n}")));
            if n == 0 {
                group = group.with_divider();
            }
            menu = menu.add_group(group);
        }
        menu
    }

    #[test]
    fn groups_sharing_a_tab_collapse_into_one() {
        // 52 packages against 6 actions is what the tab row cannot carry.
        let menu = mixed(52);
        assert_eq!(menu.groups.len(), 55);
        assert_eq!(menu.tabs().len(), 4, "three actions and one Packages");
        assert_eq!(menu.tabs()[3].label, "Packages");
    }

    #[test]
    fn a_shared_tab_keeps_each_group_heading() {
        // Without them the tab is a flat run of entries from several packages
        // with nothing telling them apart.
        let menu = mixed(3);
        let rows = menu.body_rows(on_tab(3));
        let headings: Vec<&str> = rows
            .iter()
            .filter_map(|row| match row {
                Row::Group(label) => Some(*label),
                Row::Item(..) => None,
            })
            .collect();
        assert_eq!(headings, ["@scope/pkg0", "@scope/pkg1", "@scope/pkg2"]);
    }

    #[test]
    fn a_shared_tab_counts_its_entries_across_the_groups() {
        let indices: Vec<usize> = mixed(3)
            .body_rows(on_tab(3))
            .iter()
            .filter_map(|row| match row {
                Row::Item(_, index) => Some(*index),
                Row::Group(_) => None,
            })
            .collect();
        assert_eq!(indices, [0, 1, 2], "the cursor counts the whole tab");
    }

    #[test]
    fn a_group_that_names_its_own_tab_still_carries_no_heading() {
        let menu = mixed(3);
        let rows = menu.body_rows(on_tab(0));
        assert!(rows.iter().all(|row| matches!(row, Row::Item(..))));
    }

    #[test]
    fn the_divider_marks_where_the_kind_changes() {
        let shown = framed(&mixed(3), on_tab(0));
        let row = shown.lines().find(|l| l.contains("1 Group0")).unwrap();
        assert!(row.contains("│"), "{row:?}");
        let (before, after) = row.split_once('│').unwrap();
        assert!(before.contains("3 Group2"), "actions on one side");
        assert!(after.contains("4 Packages"), "packages on the other");
    }

    #[test]
    fn a_divider_never_leads_the_row() {
        // It would separate the tabs from nothing. With the window starting on
        // the Packages tab, the ellipsis already marks the break.
        let shown = crate::internal::collect_to_string(|buf| {
            mixed(3).write_frame(
                buf,
                plain(),
                Some(0),
                Some(Viewport::new(0, 999).with_width(Some(24))),
                on_tab(3),
            )
        });
        let row = shown.lines().find(|l| l.contains("Packages")).unwrap();
        assert!(!row.trim_start().starts_with('│'), "{row:?}");
    }

    #[test]
    fn a_divider_is_counted_in_the_row_width() {
        let menu = mixed(3);
        for columns in [20, 28, 36, 50, 80] {
            let shown = crate::internal::collect_to_string(|buf| {
                menu.write_frame(
                    buf,
                    plain(),
                    Some(0),
                    Some(Viewport::new(0, 999).with_width(Some(columns))),
                    on_tab(0),
                )
            });
            for line in shown.lines() {
                assert!(
                    line.width() <= columns,
                    "width {columns}: line of {} columns: {line:?}",
                    line.width()
                );
            }
        }
    }

    #[test]
    fn a_menu_whose_groups_all_share_one_tab_gets_no_tab_row() {
        // One tab is nothing to switch to, however many groups feed it.
        let menu = Menu::new()
            .with_layout(Layout::Tabs)
            .add_group(Group::new("a").in_tab("All").add_item(Item::new("1", "1")))
            .add_group(Group::new("b").in_tab("All").add_item(Item::new("2", "2")));
        assert!(menu.active_tab(on_tab(0)).is_none());
    }

    #[test]
    fn a_narrow_tab_row_keeps_the_active_tab_visible() {
        let labels: Vec<(String, bool)> = (0..9)
            .map(|n| (format!("{} Group{n}", n + 1), false))
            .collect();
        for active in 0..labels.len() {
            for columns in [20, 30, 45, 80] {
                let window = tab_window(&labels, active, Some(columns));
                assert!(
                    window.contains(&active),
                    "width {columns}, tab {active}: {window:?}"
                );
            }
        }
    }

    #[test]
    fn a_tab_row_never_exceeds_the_given_width() {
        let menu = tabbed(9);
        for columns in [20, 30, 45, 80, 120] {
            let shown = crate::internal::collect_to_string(|buf| {
                menu.write_frame(
                    buf,
                    plain(),
                    Some(0),
                    Some(Viewport::new(0, 999).with_width(Some(columns))),
                    on_tab(4),
                )
            });
            for line in shown.lines() {
                assert!(
                    line.width() <= columns,
                    "width {columns}: line of {} columns: {line:?}",
                    line.width()
                );
            }
        }
    }

    #[test]
    fn a_summary_gets_its_own_line_under_the_heading() {
        let menu = Menu::new()
            .with_heading("web-casoon")
            .with_note("pnpm")
            .with_summary("27 scripts · 7 groups")
            .add_group(Group::new("G").add_item(Item::new("dev", "dev")));
        let rendered = menu.render(plain());
        let lines: Vec<&str> = rendered.lines().collect();
        assert_eq!(lines[0], "web-casoon  pnpm");
        assert_eq!(lines[1].trim(), "27 scripts · 7 groups");
        assert_eq!(lines[2], "", "the blank line stays under the block");
    }

    #[test]
    fn chrome_height_counts_the_summary_and_the_tab_row() {
        let menu = tabbed(3).with_summary("6 scripts · 3 groups");
        // Heading, summary, blank, tab row, rule, blank, footer.
        assert_eq!(menu.chrome_height(on_tab(0), None), 7);
        assert_eq!(
            menu.chrome_height(searching("a"), None),
            5,
            "the tab row goes away while searching"
        );
    }

    #[test]
    fn a_tabbed_window_never_draws_more_body_lines_than_it_was_given() {
        // The frame must not outgrow the terminal, or the redraw moves the
        // cursor further up than there are lines.
        let menu = tabbed(9).with_summary("18 scripts · 9 groups");
        let chrome = menu.chrome_height(on_tab(4), None);
        for height in [3, 5, 10, 25] {
            let shown = crate::internal::collect_to_string(|buf| {
                menu.write_frame(
                    buf,
                    plain(),
                    Some(0),
                    Some(Viewport::new(0, height)),
                    on_tab(4),
                )
            });
            let body = shown.lines().count() - chrome;
            assert!(body <= height, "height {height}: drew {body} body lines");
        }
    }

    #[test]
    fn a_footer_too_wide_breaks_rather_than_wrapping() {
        // The bug this pins: a footer wider than the terminal wrapped, so the
        // frame occupied more rows than it reported, and the redraw — which
        // moves the cursor up by that count — left a stale line behind on
        // every keypress. Adding the group keys made it reachable at 60
        // columns with four hints.
        let menu = tabbed(3)
            .add_hint(Hint::new('H', "Health"))
            .add_hint(Hint::new('C', "Clean"))
            .add_hint(Hint::new('S', "Security"))
            .add_hint(Hint::new('U', "Updates"));

        for columns in [30, 40, 60, 80, 120] {
            let shown = crate::internal::collect_to_string(|buf| {
                menu.write_frame(
                    buf,
                    plain(),
                    Some(0),
                    Some(Viewport::new(0, 999).with_width(Some(columns))),
                    on_tab(0),
                )
            });
            for line in shown.lines() {
                assert!(
                    line.width() <= columns,
                    "width {columns}: line of {} columns: {line:?}",
                    line.width()
                );
            }
            for key in ["←→ group", "H Health", "C Clean", "S Security", "U Updates"] {
                assert!(shown.contains(key), "width {columns} dropped {key}");
            }
        }
    }

    #[test]
    fn the_frame_is_exactly_as_tall_as_it_says() {
        // The redraw moves the cursor up by the height the frame reports. Any
        // disagreement between that and what is written shows up as a stale
        // line or an eaten one.
        let menu = tabbed(3)
            .with_summary("6 entries · 3 groups")
            .add_hint(Hint::new('H', "Health"))
            .add_hint(Hint::new('C', "Clean"))
            .add_hint(Hint::new('S', "Security"))
            .add_hint(Hint::new('U', "Updates"));

        for columns in [30, 60, 120] {
            for view in [on_tab(0), on_tab(2), searching("a"), searching("")] {
                let shown = crate::internal::collect_to_string(|buf| {
                    menu.write_frame(
                        buf,
                        plain(),
                        Some(0),
                        Some(Viewport::new(0, 999).with_width(Some(columns))),
                        view,
                    )
                });
                let body = menu.body_height(view).min(999);
                assert_eq!(
                    shown.lines().count(),
                    menu.chrome_height(view, Some(columns)) + body,
                    "width {columns}, view {view:?}"
                );
            }
        }
    }

    #[test]
    fn a_long_query_cannot_wrap_the_frame() {
        let menu = tabbed(3);
        let query = "a".repeat(200);
        let shown = crate::internal::collect_to_string(|buf| {
            menu.write_frame(
                buf,
                plain(),
                Some(0),
                Some(Viewport::new(0, 999).with_width(Some(40))),
                searching(&query),
            )
        });
        for line in shown.lines() {
            assert!(line.width() <= 40, "line of {} columns", line.width());
        }
    }

    #[test]
    fn a_tabbed_frame_advertises_the_group_keys() {
        // A key the menu answers to but never mentions is a key nobody presses.
        assert!(framed(&tabbed(3), on_tab(0)).contains("←→ group"));
        assert!(
            !framed(&tabbed(3), flat()).contains("←→ group"),
            "and not where they do nothing"
        );
    }

    #[test]
    fn a_subsequence_span_measures_tightness() {
        assert_eq!(subsequence_span("deploy", "dep"), Some(3));
        assert_eq!(subsequence_span("deploy", "dy"), Some(6));
        assert_eq!(subsequence_span("deploy", "dz"), None);
    }

    #[test]
    fn the_query_line_is_drawn_while_searching() {
        let menu = script_menu();
        let shown = crate::internal::collect_to_string(|buf| {
            menu.write_frame(buf, plain(), Some(0), None, searching("dep"))
        });
        assert!(shown.contains("/ dep"));
    }

    #[test]
    fn an_opened_search_prompts_before_anything_is_typed() {
        let menu = script_menu();
        let shown = crate::internal::collect_to_string(|buf| {
            menu.write_frame(buf, plain(), Some(0), None, searching(""))
        });
        assert!(shown.contains("type to filter"));
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
