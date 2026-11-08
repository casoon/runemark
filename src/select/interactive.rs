//! The interactive loop: draw, read a key, act, redraw.

use std::collections::BTreeSet;
use std::io::{self, Write};

use super::terminal::{Key, RawTerminal, escape};

use super::{Menu, Outcome, Picked, SelectMode, View, Viewport};
use crate::color::Console;

/// Hides the cursor for as long as this value lives.
///
/// Separate from [`RawTerminal`] so that dropping them in order restores the
/// cursor before termios, and so a failure to hide never leaves raw mode set.
struct HiddenCursor;

impl HiddenCursor {
    fn hide() -> Self {
        let mut stderr = io::stderr();
        let _ = write!(stderr, "{}", escape::HIDE_CURSOR);
        let _ = stderr.flush();
        Self
    }
}

impl Drop for HiddenCursor {
    fn drop(&mut self) {
        let mut stderr = io::stderr();
        let _ = write!(stderr, "{}", escape::SHOW_CURSOR);
        let _ = stderr.flush();
    }
}

impl Menu {
    /// Runs the menu, returning what the user did.
    ///
    /// Writes to stderr, so a caller's stdout stays clean for piping. Keys are
    /// read from `/dev/tty`, so a redirected stdin does not disable navigation.
    ///
    /// Returns [`Outcome::Unavailable`] without reading anything when `mode`
    /// and `is_terminal` rule out interaction, when the menu has no entries, or
    /// when the process has no controlling terminal to open.
    /// It never blocks in that case — a menu in a pipeline or in CI must not
    /// wait for a keypress that cannot come.
    ///
    /// `is_terminal` is supplied by the caller rather than detected here,
    /// matching the rest of the crate: the application owns the decision about
    /// its own streams.
    ///
    /// `Ctrl-C` arrives as a byte in raw mode and is reported as
    /// [`Outcome::Cancelled`], so the terminal is restored normally.
    ///
    /// A signal that kills the process outright — `SIGTERM`, `SIGHUP` — leaves
    /// the terminal in raw mode, because no destructor runs. This crate
    /// installs no signal handlers; an application that needs to survive that
    /// must install its own.
    pub fn run(
        &self,
        console: Console,
        mode: SelectMode,
        is_terminal: bool,
    ) -> io::Result<Outcome> {
        Ok(
            match self.session(console, mode, is_terminal, State::default())? {
                None => Outcome::Unavailable,
                Some(Ending::Finished(outcome)) => outcome,
                // Only a state that ticks can confirm, and this one does not.
                Some(Ending::Confirmed(_)) => Outcome::Cancelled,
            },
        )
    }

    /// Runs the menu to pick any number of entries, returning which.
    ///
    /// Space ticks the entry under the cursor and Enter confirms the lot, so
    /// confirming with nothing ticked is an answer too — [`Picked::Chosen`]
    /// with an empty list, which is not the same as cancelling.
    ///
    /// Entries named by [`Menu::with_ticked`] start ticked.
    ///
    /// Hints are neither shown nor answered: a key ending the menu would have
    /// to drop what was ticked. Everything else — the filter, tabs, and when
    /// the menu declines to run — is as in [`Menu::run`].
    pub fn run_multi(
        &self,
        console: Console,
        mode: SelectMode,
        is_terminal: bool,
    ) -> io::Result<Picked> {
        let start = State {
            checked: Some(self.ticked.clone()),
            ..State::default()
        };
        Ok(match self.session(console, mode, is_terminal, start)? {
            None => Picked::Unavailable,
            Some(Ending::Confirmed(checked)) => Picked::Chosen(
                self.items()
                    .filter(|item| checked.contains(&item.id))
                    .map(|item| item.id.clone())
                    .collect(),
            ),
            // Hints are not answered here, so nothing but a cancel finishes.
            Some(Ending::Finished(_)) => Picked::Cancelled,
        })
    }

    /// Takes over the terminal for one run, or returns `None` where it cannot.
    fn session(
        &self,
        console: Console,
        mode: SelectMode,
        is_terminal: bool,
        start: State,
    ) -> io::Result<Option<Ending>> {
        if !mode.is_interactive(is_terminal) || self.is_empty() {
            return Ok(None);
        }

        let Some(terminal) = RawTerminal::acquire()? else {
            return Ok(None);
        };
        let cursor = HiddenCursor::hide();

        let ending = self.event_loop(console, &terminal, start);

        drop(cursor);
        drop(terminal);

        // Leave the final frame behind rather than a half-erased one.
        let _ = writeln!(io::stderr());
        ending.map(Some)
    }

    fn event_loop(
        &self,
        console: Console,
        terminal: &RawTerminal,
        mut state: State,
    ) -> io::Result<Ending> {
        let mut view = Scroll::default();
        let mut stderr = io::stderr();

        loop {
            self.draw(console, terminal.size(), &mut stderr, &state, &mut view)?;

            match self.act_on(terminal.read_key()?, &state) {
                Action::Update(next) => state = next,
                Action::Finish(outcome) => {
                    self.draw(console, terminal.size(), &mut stderr, &state, &mut view)?;
                    return Ok(Ending::Finished(outcome));
                }
                Action::Confirm => {
                    self.draw(console, terminal.size(), &mut stderr, &state, &mut view)?;
                    return Ok(Ending::Confirmed(state.checked.unwrap_or_default()));
                }
                Action::Ignore => {}
            }
        }
    }

    fn act_on(&self, key: Key, state: &State) -> Action {
        let matches = self.matching_items(state.view());
        // An empty result set still draws; it just has nothing to act on.
        let last = matches.len().saturating_sub(1);

        match key {
            Key::Left | Key::BackTab => self.step_tab(state, false),
            Key::Right | Key::Tab => self.step_tab(state, true),
            // Wrapping beats stopping at the ends: the list is short and a dead
            // key at the bottom is the more annoying failure.
            Key::Up => Action::Update(state.at(if state.index == 0 {
                last
            } else {
                state.index - 1
            })),
            Key::Down => Action::Update(state.at(if state.index >= last {
                0
            } else {
                state.index + 1
            })),
            Key::Enter if state.checked.is_some() => Action::Confirm,
            Key::Enter => matches.get(state.index).map_or(Action::Ignore, |item| {
                Action::Finish(Outcome::Selected(item.id.clone()))
            }),
            // Escape leaves the search before it leaves the menu, so a mistyped
            // query costs one key rather than the whole selection.
            Key::Escape => match state.query {
                Some(_) => Action::Update(state.browsing()),
                None => Action::Finish(Outcome::Cancelled),
            },
            Key::Interrupt => Action::Finish(Outcome::Cancelled),
            Key::Backspace => match &state.query {
                Some(query) if !query.is_empty() => {
                    let mut query = query.clone();
                    query.pop();
                    Action::Update(state.searching(query))
                }
                // Backspacing out of an empty query leaves the search.
                Some(_) => Action::Update(state.browsing()),
                None => Action::Ignore,
            },
            Key::Char(pressed) => self.act_on_char(pressed, state),
            Key::Other => Action::Ignore,
        }
    }

    fn act_on_char(&self, pressed: char, state: &State) -> Action {
        // Space ticks even inside a search, so a filtered list can be ticked
        // without leaving it. No entry name is worth typing a space for.
        if pressed == ' ' && state.checked.is_some() {
            let matches = self.matching_items(state.view());
            return matches.get(state.index).map_or(Action::Ignore, |item| {
                Action::Update(state.toggled(&item.id))
            });
        }
        // Inside a search every printable key is part of the query, so a
        // script named "quality" can be typed without 'q' cancelling.
        if let Some(query) = &state.query {
            let mut query = query.clone();
            query.extend(pressed.to_lowercase());
            return Action::Update(state.searching(query));
        }

        if pressed == '/' {
            return Action::Update(state.searching(String::new()));
        }
        // Digits before hints: a menu with tabs has given its digits away, and
        // the tab is the one of the two the user can see on screen.
        if let Some(action) = self.act_on_digit(pressed, state) {
            return action;
        }
        // Hint keys win over the built-in 'q', so a menu may bind 'q'. Not
        // while ticking: `run_multi` does not answer them.
        if let Some(hint) = self
            .hints
            .iter()
            .filter(|_| state.checked.is_none())
            .find(|hint| hint.key.eq_ignore_ascii_case(&pressed))
        {
            return Action::Finish(Outcome::Hotkey(hint.key));
        }
        if pressed == 'q' {
            return Action::Finish(Outcome::Cancelled);
        }
        Action::Ignore
    }

    /// What a digit does, or `None` where the menu has no tabs to reach.
    ///
    /// A tabbed menu has given its digits away, so one that names no tab does
    /// nothing rather than falling through to a hint bound to it. That covers
    /// `0`, which no tab carries, and any digit past the last group.
    ///
    /// Written without a `let` chain: those are stable from Rust 1.88, and
    /// this crate compiles on 1.85.
    fn act_on_digit(&self, pressed: char, state: &State) -> Option<Action> {
        self.active_tab(state.view())?;
        let digit = usize::try_from(pressed.to_digit(10)?).ok()?;
        let tab = digit.checked_sub(1).filter(|tab| *tab < self.tab_count());
        Some(tab.map_or(Action::Ignore, |tab| Action::Update(state.on_tab(tab))))
    }

    /// Moves one tab along, wrapping, and puts the cursor on its first entry.
    ///
    /// Leaving the cursor where it was would land it on an unrelated entry of
    /// the new group, or past its end.
    fn step_tab(&self, state: &State, forward: bool) -> Action {
        let Some(active) = self.active_tab(state.view()) else {
            return Action::Ignore;
        };
        let last = self.tab_count() - 1;
        let next = if forward {
            if active == last { 0 } else { active + 1 }
        } else if active == 0 {
            last
        } else {
            active - 1
        };
        Action::Update(state.on_tab(next))
    }

    /// Draws the menu in place, overwriting the previous frame.
    ///
    /// `size` is the terminal's lines and columns, if it reports them. Taking
    /// it and the writer rather than the terminal is what lets the redraw's
    /// bookkeeping be asserted without one.
    fn draw(
        &self,
        console: Console,
        size: Option<(usize, usize)>,
        out: &mut impl Write,
        state: &State,
        view: &mut Scroll,
    ) -> io::Result<()> {
        let viewport = view.advance(self, size, state);

        // Raw mode drops the implicit carriage return on newline, so the frame
        // is rendered normally and then given explicit ones.
        let frame = crate::internal::collect_to_string(|buf| {
            self.write_frame(buf, console, Some(state.index), viewport, state.view())
        });
        let lines: Vec<&str> = frame.lines().collect();

        if let Some(previous) = view.drawn {
            write!(out, "{}{}", escape::move_up(previous), escape::CLEAR_TO_END)?;
        }
        for line in &lines {
            write!(out, "{line}\r\n")?;
        }
        out.flush()?;

        // Counting what was actually written is what keeps the redraw exact.
        // Deriving the height a second time invites the two to disagree, which
        // shows up as a stale line or an eaten one.
        view.drawn = Some(u16::try_from(lines.len()).unwrap_or(u16::MAX));
        Ok(())
    }
}

/// Where the body is scrolled to, and how tall the last frame was.
#[derive(Debug, Default)]
struct Scroll {
    start: usize,
    drawn: Option<u16>,
}

impl Scroll {
    /// Scrolls the window the least amount that brings the cursor into view.
    ///
    /// Only moving when the cursor would leave the window keeps the list still
    /// under the cursor; recentring on every keypress makes short moves feel
    /// like the whole screen is sliding.
    fn advance(
        &mut self,
        menu: &Menu,
        size: Option<(usize, usize)>,
        state: &State,
    ) -> Option<Viewport> {
        // A terminal that reports no size gets the whole menu, as before.
        let (height, columns) = size?;
        let columns = (columns > 0).then_some(columns);
        let view = state.view();
        let rows = menu.body_height(view);
        // One line stays free so the frame does not push its own top off screen.
        // A terminal shorter than the chrome still gets one row: the cursor has
        // to be somewhere, and the viewport keeps it in sight.
        let body = height
            .saturating_sub(menu.chrome_height(view, columns) + 1)
            .max(1);

        if rows <= body {
            self.start = 0;
            // Still bound the width: a short list can carry long descriptions.
            return Some(Viewport::new(0, rows.max(1)).with_width(columns));
        }

        let cursor = menu.row_of_item(state.index, view);
        let mut start = self.start.min(rows - 1);

        if cursor < start {
            start = cursor;
        }
        // Ask the viewport what it will actually draw rather than predicting
        // it. How many rows fit depends on which scroll indicators appear,
        // which depends on the start — a second calculation of that drifts,
        // and the drift shows up as a cursor scrolled just off the bottom.
        while start < rows - 1 && !Viewport::new(start, body).shows(rows, cursor) {
            start += 1;
        }

        self.start = start;
        Some(Viewport::new(start, body).with_width(columns))
    }
}

/// Where the cursor is, and what is being searched for.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct State {
    index: usize,
    /// `None` outside search mode; `Some("")` once `/` has been pressed.
    query: Option<String>,
    /// The tab being shown. Kept even while searching and while the menu has
    /// no tabs at all, so leaving the search returns to the group it left.
    tab: usize,
    /// The ticked entries by id, `None` when picking one. Ids rather than
    /// positions: a search reorders the list, and a tick has to survive it.
    checked: Option<BTreeSet<String>>,
}

impl State {
    fn view(&self) -> View<'_> {
        View {
            query: self.query.as_deref(),
            checked: self.checked.as_ref(),
            tab: Some(self.tab),
        }
    }

    fn at(&self, index: usize) -> Self {
        Self {
            index,
            ..self.clone()
        }
    }

    fn toggled(&self, id: &str) -> Self {
        let mut next = self.clone();
        if let Some(checked) = &mut next.checked {
            if !checked.remove(id) {
                checked.insert(id.to_owned());
            }
        }
        next
    }

    /// Changing the query resets the cursor: the best match is now first, and
    /// leaving the cursor where it was would land it on something unrelated.
    fn searching(&self, query: String) -> Self {
        Self {
            index: 0,
            query: Some(query),
            ..self.clone()
        }
    }

    /// Back to the list, on the tab the search was started from.
    fn browsing(&self) -> Self {
        Self {
            index: 0,
            query: None,
            ..self.clone()
        }
    }

    fn on_tab(&self, tab: usize) -> Self {
        Self {
            index: 0,
            tab,
            ..self.clone()
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Action {
    Update(State),
    Finish(Outcome),
    /// Enter while ticking: the ticks are the answer.
    Confirm,
    Ignore,
}

/// How a run ended.
enum Ending {
    Finished(Outcome),
    Confirmed(BTreeSet<String>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ColorMode;
    use crate::select::{Group, Hint, Item, Layout};

    fn tabbed() -> Menu {
        let mut menu = Menu::new().with_layout(Layout::Tabs);
        for n in 0..3 {
            menu = menu.add_group(
                Group::new(format!("Group{n}"))
                    .add_item(Item::new(format!("a{n}"), format!("a{n}")))
                    .add_item(Item::new(format!("b{n}"), format!("b{n}"))),
            );
        }
        menu
    }

    fn after(menu: &Menu, state: &State, key: Key) -> State {
        match menu.act_on(key, state) {
            Action::Update(next) => next,
            Action::Ignore => state.clone(),
            Action::Finish(outcome) => panic!("expected no outcome, got {outcome:?}"),
            Action::Confirm => panic!("expected no outcome, got a confirmation"),
        }
    }

    #[test]
    fn a_terminal_shorter_than_the_chrome_still_gets_one_row() {
        // The whole list would not fit, and neither would the frame: with no
        // body left the menu draws the row under the cursor, not everything.
        let mut menu = Menu::new().with_heading("many");
        let mut group = Group::new("Scripts");
        for n in 0..40 {
            group = group.add_item(Item::new(format!("t{n}"), format!("t{n}")));
        }
        menu = menu.add_group(group);

        let state = State::default().at(20);
        let size = Some((menu.chrome_height(state.view(), Some(40)), 40));
        let viewport = Scroll::default()
            .advance(&menu, size, &state)
            .expect("a size was given");
        let rows = menu.body_height(state.view());
        let cursor = menu.row_of_item(20, state.view());
        assert!(viewport.shows(rows, cursor), "the cursor stays in sight");
        assert_eq!(viewport.window(rows).len(), 1, "and only one row is drawn");
    }

    /// A flat menu of `items` entries under a heading.
    fn scripts(items: usize) -> Menu {
        let mut group = Group::new("Scripts");
        for n in 0..items {
            group = group.add_item(Item::new(format!("t{n}"), format!("t{n}")));
        }
        Menu::new().with_heading("many").add_group(group)
    }

    /// A terminal `columns` wide with `body` lines to spare for entries.
    fn terminal_for(
        menu: &Menu,
        state: &State,
        columns: usize,
        body: usize,
    ) -> Option<(usize, usize)> {
        let chrome = menu.chrome_height(state.view(), Some(columns));
        // One line stays free, as `advance` leaves it.
        Some((chrome + 1 + body, columns))
    }

    #[test]
    fn a_terminal_that_reports_no_size_gets_the_whole_menu() {
        let menu = scripts(40);
        assert_eq!(
            Scroll::default().advance(&menu, None, &State::default()),
            None
        );
    }

    #[test]
    fn a_menu_that_fits_is_not_scrolled() {
        let menu = scripts(5);
        let state = State::default();
        let size = terminal_for(&menu, &state, 40, 6);
        let mut scroll = Scroll::default();
        let viewport = scroll.advance(&menu, size, &state).unwrap();
        assert_eq!(viewport.window(menu.body_height(state.view())), 0..6);
        assert_eq!(scroll.start, 0);
    }

    #[test]
    fn the_width_bounds_a_menu_that_fits_too() {
        // A short list can still carry descriptions wider than the terminal.
        let menu = scripts(2);
        let state = State::default();
        let mut scroll = Scroll::default();
        let size = Some((50, 33));
        assert_eq!(scroll.advance(&menu, size, &state).unwrap().width, Some(33));
        // A terminal that reports no columns bounds nothing.
        let size = Some((50, 0));
        assert_eq!(scroll.advance(&menu, size, &state).unwrap().width, None);
    }

    #[test]
    fn the_cursor_is_in_view_at_every_position_going_down_and_back_up() {
        // The bug this pins: stepping to the last entry left the cursor one
        // row below the window, because the scroll maths and the window maths
        // disagreed about how many lines the indicators cost.
        let menu = scripts(40);
        for body in [1, 2, 3, 4, 6, 11, 21, 29] {
            let mut scroll = Scroll::default();
            let positions = (0..menu.len()).chain((0..menu.len()).rev());
            for index in positions {
                let state = State::default().at(index);
                let size = terminal_for(&menu, &state, 40, body);
                let viewport = scroll.advance(&menu, size, &state).unwrap();
                let rows = menu.body_height(state.view());
                let cursor = menu.row_of_item(index, state.view());
                assert!(
                    viewport.shows(rows, cursor),
                    "body {body}, item {index} (row {cursor}) not visible from {}",
                    scroll.start
                );
            }
        }
    }

    #[test]
    fn the_window_holds_still_while_the_cursor_moves_inside_it() {
        // Recentring on every key makes short moves feel like the whole screen
        // is sliding.
        let menu = scripts(40);
        let mut scroll = Scroll::default();
        let mut advance_to = |index: usize| {
            let state = State::default().at(index);
            let size = terminal_for(&menu, &state, 40, 10);
            scroll.advance(&menu, size, &state).unwrap();
            scroll.start
        };

        assert_eq!(advance_to(0), 0);
        assert_eq!(advance_to(5), 0, "still inside the first window");
        let scrolled = advance_to(30);
        assert!(scrolled > 0, "the cursor left the window, so it moved");
        assert_eq!(advance_to(29), scrolled, "one step back stays inside it");
    }

    #[test]
    fn the_first_frame_overwrites_nothing() {
        let menu = scripts(3);
        let mut scroll = Scroll::default();
        let mut out = Vec::new();
        menu.draw(plain(), None, &mut out, &State::default(), &mut scroll)
            .unwrap();
        let shown = String::from_utf8(out).unwrap();

        assert!(
            !shown.contains('\x1b'),
            "nothing to move up over: {shown:?}"
        );
        // Raw mode drops the implicit carriage return, so every line brings one.
        assert_eq!(shown.matches('\n').count(), shown.matches("\r\n").count());
        assert_eq!(usize::from(scroll.drawn.unwrap()), shown.lines().count());
    }

    #[test]
    fn a_redraw_moves_up_over_exactly_the_lines_it_drew() {
        let menu = scripts(3);
        let mut scroll = Scroll::default();
        let state = State::default();

        let mut first = Vec::new();
        menu.draw(plain(), None, &mut first, &state, &mut scroll)
            .unwrap();
        let drawn = scroll.drawn.unwrap();

        let mut second = Vec::new();
        menu.draw(plain(), None, &mut second, &state.at(1), &mut scroll)
            .unwrap();
        let second = String::from_utf8(second).unwrap();
        let moved = format!("{}{}", escape::move_up(drawn), escape::CLEAR_TO_END);
        assert!(second.starts_with(&moved), "{second:?}");
    }

    #[test]
    fn a_frame_that_changes_height_is_counted_afresh() {
        // Searching adds the query line and drops the footer, so the frame is
        // a different height. The next redraw must move up by the new count.
        let menu = scripts(3);
        let mut scroll = Scroll::default();
        let browsing = State::default();
        let searching = browsing.searching(String::new());

        menu.draw(plain(), None, &mut Vec::new(), &browsing, &mut scroll)
            .unwrap();
        let before = scroll.drawn.unwrap();

        let mut out = Vec::new();
        menu.draw(plain(), None, &mut out, &searching, &mut scroll)
            .unwrap();
        let after = scroll.drawn.unwrap();
        let shown = String::from_utf8(out).unwrap();
        assert!(shown.starts_with(&escape::move_up(before)));
        let frame = shown.split(escape::CLEAR_TO_END).nth(1).unwrap();
        assert_eq!(usize::from(after), frame.lines().count());

        let mut out = Vec::new();
        menu.draw(plain(), None, &mut out, &browsing, &mut scroll)
            .unwrap();
        let shown = String::from_utf8(out).unwrap();
        assert!(shown.starts_with(&escape::move_up(after)), "{shown:?}");
    }

    #[test]
    fn a_scrolled_frame_is_no_taller_than_the_terminal_it_was_sized_for() {
        let menu = scripts(40);
        for body in [1, 2, 3, 5, 10] {
            let state = State::default().at(20);
            let size = terminal_for(&menu, &state, 40, body);
            let mut scroll = Scroll::default();
            menu.draw(plain(), size, &mut Vec::new(), &state, &mut scroll)
                .unwrap();
            let height = size.unwrap().0;
            assert!(
                usize::from(scroll.drawn.unwrap()) < height,
                "body {body}: drew {:?} lines for {height}",
                scroll.drawn
            );
        }
    }

    fn plain() -> Console {
        Console::new(ColorMode::Never, false)
    }

    fn flat_menu() -> Menu {
        tabbed().with_layout(Layout::Flat)
    }

    #[test]
    fn up_and_down_wrap_at_both_ends() {
        let menu = flat_menu();
        let last = menu.len() - 1;
        assert_eq!(after(&menu, &State::default(), Key::Up).index, last);
        assert_eq!(after(&menu, &State::default().at(last), Key::Down).index, 0);
        assert_eq!(after(&menu, &State::default().at(2), Key::Up).index, 1);
        assert_eq!(after(&menu, &State::default().at(2), Key::Down).index, 3);
    }

    #[test]
    fn up_and_down_over_no_matches_stay_put() {
        let menu = flat_menu();
        let nothing = State::default().searching("zzzz".into());
        assert_eq!(after(&menu, &nothing, Key::Up).index, 0);
        assert_eq!(after(&menu, &nothing, Key::Down).index, 0);
    }

    #[test]
    fn backspace_shortens_the_query_and_leaves_the_search_when_it_is_empty() {
        let menu = flat_menu();
        let state = State::default().searching("ab".into());
        let state = after(&menu, &state, Key::Backspace);
        assert_eq!(state.query.as_deref(), Some("a"));
        let state = after(&menu, &state, Key::Backspace);
        assert_eq!(state.query.as_deref(), Some(""), "still searching, empty");
        let state = after(&menu, &state, Key::Backspace);
        assert_eq!(state.query, None, "backspacing out of it leaves the search");
    }

    #[test]
    fn backspace_outside_a_search_does_nothing() {
        let menu = flat_menu();
        assert_eq!(
            menu.act_on(Key::Backspace, &State::default()),
            Action::Ignore
        );
    }

    #[test]
    fn escape_cancels_only_when_there_is_no_search_to_leave() {
        let menu = flat_menu();
        assert_eq!(
            menu.act_on(Key::Escape, &State::default()),
            Action::Finish(Outcome::Cancelled)
        );
        let searching = State::default().searching("a".into());
        assert_eq!(after(&menu, &searching, Key::Escape).query, None);
    }

    #[test]
    fn interrupt_cancels_even_in_the_middle_of_a_search() {
        let menu = flat_menu();
        let searching = State::default().searching("a".into());
        assert_eq!(
            menu.act_on(Key::Interrupt, &searching),
            Action::Finish(Outcome::Cancelled)
        );
    }

    #[test]
    fn q_cancels_but_is_typed_inside_a_search() {
        let menu = flat_menu();
        assert_eq!(
            menu.act_on(Key::Char('q'), &State::default()),
            Action::Finish(Outcome::Cancelled)
        );
        let searching = State::default().searching(String::new());
        assert_eq!(
            after(&menu, &searching, Key::Char('q')).query.as_deref(),
            Some("q")
        );
    }

    #[test]
    fn a_hint_bound_to_q_wins_over_cancelling() {
        let menu = flat_menu().add_hint(Hint::new('q', "Quit properly"));
        assert_eq!(
            menu.act_on(Key::Char('q'), &State::default()),
            Action::Finish(Outcome::Hotkey('q'))
        );
    }

    #[test]
    fn a_hint_matches_whatever_the_case_of_the_key() {
        let upper = flat_menu().add_hint(Hint::new('H', "Health"));
        assert_eq!(
            upper.act_on(Key::Char('h'), &State::default()),
            Action::Finish(Outcome::Hotkey('H')),
            "reports the key as the menu declared it"
        );
        let lower = flat_menu().add_hint(Hint::new('h', "Health"));
        assert_eq!(
            lower.act_on(Key::Char('H'), &State::default()),
            Action::Finish(Outcome::Hotkey('h'))
        );
    }

    #[test]
    fn enter_with_nothing_to_select_does_nothing() {
        let menu = flat_menu();
        let nothing = State::default().searching("zzzz".into());
        assert_eq!(menu.act_on(Key::Enter, &nothing), Action::Ignore);
        assert_eq!(
            Menu::new().act_on(Key::Enter, &State::default()),
            Action::Ignore
        );
    }

    #[test]
    fn the_arrows_step_along_the_tabs_and_wrap() {
        let menu = tabbed();
        let state = after(&menu, &State::default(), Key::Right);
        assert_eq!(state.tab, 1);
        let state = after(&menu, &state, Key::Left);
        assert_eq!(state.tab, 0);
        let state = after(&menu, &state, Key::Left);
        assert_eq!(state.tab, 2, "wrapping beats a dead key at the end");
        assert_eq!(after(&menu, &state, Key::Right).tab, 0);
    }

    #[test]
    fn tab_and_shift_tab_do_the_same() {
        let menu = tabbed();
        assert_eq!(after(&menu, &State::default(), Key::Tab).tab, 1);
        assert_eq!(after(&menu, &State::default(), Key::BackTab).tab, 2);
    }

    #[test]
    fn a_digit_jumps_straight_to_its_tab() {
        let menu = tabbed();
        assert_eq!(after(&menu, &State::default(), Key::Char('3')).tab, 2);
    }

    #[test]
    fn a_digit_past_the_last_tab_does_nothing() {
        let menu = tabbed();
        let state = after(&menu, &State::default(), Key::Char('2'));
        assert_eq!(after(&menu, &state, Key::Char('7')).tab, 1, "unchanged");
    }

    #[test]
    fn a_digit_no_tab_carries_is_swallowed_rather_than_passed_on() {
        // The menu has given its digits away; falling through to a hint bound
        // to one would make the same key mean two things on one screen.
        let menu = tabbed().add_hint(Hint::new('0', "Zero"));
        assert_eq!(
            menu.act_on(Key::Char('0'), &State::default()),
            Action::Ignore
        );
        assert_eq!(
            menu.act_on(Key::Char('8'), &State::default()),
            Action::Ignore
        );
    }

    #[test]
    fn a_flat_menu_still_lets_a_digit_reach_its_hint() {
        let menu = tabbed()
            .with_layout(Layout::Flat)
            .add_hint(Hint::new('2', "Two"));
        assert_eq!(
            menu.act_on(Key::Char('2'), &State::default()),
            Action::Finish(Outcome::Hotkey('2'))
        );
    }

    #[test]
    fn switching_tabs_puts_the_cursor_on_the_first_entry() {
        // Keeping the index would land it on an unrelated entry of the new
        // group, or past its end.
        let menu = tabbed();
        let state = after(&menu, &State::default(), Key::Down);
        assert_eq!(state.index, 1);
        assert_eq!(after(&menu, &state, Key::Right).index, 0);
    }

    #[test]
    fn a_flat_menu_ignores_the_tab_keys() {
        let menu = tabbed().with_layout(Layout::Flat);
        let state = after(&menu, &State::default(), Key::Right);
        assert_eq!(state.tab, 0);
        assert_eq!(state.index, 0);
    }

    #[test]
    fn a_digit_inside_a_search_is_typed_rather_than_pressed() {
        // A script named "build2" has to be reachable.
        let menu = tabbed();
        let state = after(&menu, &State::default(), Key::Char('/'));
        let state = after(&menu, &state, Key::Char('2'));
        assert_eq!(state.query.as_deref(), Some("2"));
        assert_eq!(state.tab, 0, "and the tab is untouched");
    }

    #[test]
    fn an_uppercase_non_ascii_key_is_typed_lowercased() {
        // The label is lowercased with Unicode rules, so the query has to be.
        let menu = tabbed();
        let state = after(&menu, &State::default(), Key::Char('/'));
        let state = after(&menu, &state, Key::Char('Ü'));
        assert_eq!(state.query.as_deref(), Some("ü"));
    }

    #[test]
    fn leaving_a_search_returns_to_the_tab_it_started_from() {
        let menu = tabbed();
        let state = after(&menu, &State::default(), Key::Char('3'));
        let state = after(&menu, &state, Key::Char('/'));
        let state = after(&menu, &state, Key::Escape);
        assert_eq!(state.query, None);
        assert_eq!(state.tab, 2);
    }

    #[test]
    fn enter_selects_from_the_active_tab() {
        let menu = tabbed();
        let state = after(&menu, &State::default(), Key::Char('2'));
        let state = after(&menu, &state, Key::Down);
        assert_eq!(
            menu.act_on(Key::Enter, &state),
            Action::Finish(Outcome::Selected("b1".into()))
        );
    }

    fn ticking() -> State {
        State {
            checked: Some(BTreeSet::new()),
            ..State::default()
        }
    }

    fn ticked(state: &State) -> Vec<&str> {
        state.checked.iter().flatten().map(String::as_str).collect()
    }

    #[test]
    fn space_ticks_and_unticks_the_entry_under_the_cursor() {
        let menu = tabbed().with_layout(Layout::Flat);
        let state = after(&menu, &ticking(), Key::Down);
        let state = after(&menu, &state, Key::Char(' '));
        assert_eq!(ticked(&state), ["b0"]);
        let state = after(&menu, &state, Key::Char(' '));
        assert!(ticked(&state).is_empty());
    }

    #[test]
    fn enter_confirms_rather_than_selects_while_ticking() {
        let menu = tabbed();
        assert_eq!(menu.act_on(Key::Enter, &ticking()), Action::Confirm);
    }

    #[test]
    fn a_tick_survives_a_search() {
        // Ticked by id, so the filter reordering the list cannot move it.
        let menu = tabbed().with_layout(Layout::Flat);
        let state = after(&menu, &ticking(), Key::Char('/'));
        let state = after(&menu, &state, Key::Char('b'));
        let state = after(&menu, &state, Key::Char('2'));
        let state = after(&menu, &state, Key::Char(' '));
        assert_eq!(ticked(&state), ["b2"], "space ticks, not types");
        let state = after(&menu, &state, Key::Escape);
        assert_eq!(ticked(&state), ["b2"]);
    }

    #[test]
    fn a_hint_is_not_answered_while_ticking() {
        // Ending the menu on it would drop what was ticked.
        let menu = tabbed()
            .with_layout(Layout::Flat)
            .add_hint(Hint::new('H', "Health"));
        assert_eq!(menu.act_on(Key::Char('h'), &ticking()), Action::Ignore);
    }
}
