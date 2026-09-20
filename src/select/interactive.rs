//! The interactive loop: draw, read a key, act, redraw.

use std::io::{self, Write};

use super::terminal::{Key, RawTerminal, escape};

use super::{Menu, Outcome, SelectMode, View, Viewport};
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
        if !mode.is_interactive(is_terminal) || self.is_empty() {
            return Ok(Outcome::Unavailable);
        }

        let Some(terminal) = RawTerminal::acquire()? else {
            return Ok(Outcome::Unavailable);
        };
        let cursor = HiddenCursor::hide();

        let outcome = self.event_loop(console, &terminal);

        drop(cursor);
        drop(terminal);

        // Leave the final frame behind rather than a half-erased one.
        let _ = writeln!(io::stderr());
        outcome
    }

    fn event_loop(&self, console: Console, terminal: &RawTerminal) -> io::Result<Outcome> {
        let mut state = State::default();
        let mut view = Scroll::default();

        loop {
            self.draw(console, terminal, &state, &mut view)?;

            match self.act_on(terminal.read_key()?, &state) {
                Action::Update(next) => state = next,
                Action::Finish(outcome) => {
                    self.draw(console, terminal, &state, &mut view)?;
                    return Ok(outcome);
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
        // Inside a search every printable key is part of the query, so a
        // script named "quality" can be typed without 'q' cancelling.
        if let Some(query) = &state.query {
            let mut query = query.clone();
            query.push(pressed.to_ascii_lowercase());
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
        // Hint keys win over the built-in 'q', so a menu may bind 'q'.
        if let Some(hint) = self
            .hints
            .iter()
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
        let tab = digit.checked_sub(1).filter(|tab| *tab < self.group_count());
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
        let last = self.group_count() - 1;
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
    fn draw(
        &self,
        console: Console,
        terminal: &RawTerminal,
        state: &State,
        view: &mut Scroll,
    ) -> io::Result<()> {
        let viewport = view.advance(self, terminal, state);

        // Raw mode drops the implicit carriage return on newline, so the frame
        // is rendered normally and then given explicit ones.
        let frame = crate::internal::collect_to_string(|buf| {
            self.write_frame(buf, console, Some(state.index), viewport, state.view())
        });
        let lines: Vec<&str> = frame.lines().collect();

        let mut stderr = io::stderr();
        if let Some(previous) = view.drawn {
            write!(
                stderr,
                "{}{}",
                escape::move_up(previous),
                escape::CLEAR_TO_END
            )?;
        }
        for line in &lines {
            write!(stderr, "{line}\r\n")?;
        }
        stderr.flush()?;

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
    fn advance(&mut self, menu: &Menu, terminal: &RawTerminal, state: &State) -> Option<Viewport> {
        // A terminal that reports no size gets the whole menu, as before.
        let (height, columns) = terminal.size()?;
        let columns = (columns > 0).then_some(columns);
        let view = state.view();
        let rows = menu.body_height(view);
        // One line stays free so the frame does not push its own top off screen.
        let body = height.saturating_sub(menu.chrome_height(view, columns) + 1);

        if body == 0 || rows == 0 || rows <= body {
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
}

impl State {
    fn view(&self) -> View<'_> {
        View {
            query: self.query.as_deref(),
            tab: Some(self.tab),
        }
    }

    fn at(&self, index: usize) -> Self {
        Self {
            index,
            query: self.query.clone(),
            tab: self.tab,
        }
    }

    /// Changing the query resets the cursor: the best match is now first, and
    /// leaving the cursor where it was would land it on something unrelated.
    fn searching(&self, query: String) -> Self {
        Self {
            index: 0,
            query: Some(query),
            tab: self.tab,
        }
    }

    /// Back to the list, on the tab the search was started from.
    fn browsing(&self) -> Self {
        Self {
            index: 0,
            query: None,
            tab: self.tab,
        }
    }

    fn on_tab(&self, tab: usize) -> Self {
        Self {
            index: 0,
            query: self.query.clone(),
            tab,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Action {
    Update(State),
    Finish(Outcome),
    Ignore,
}

#[cfg(test)]
mod tests {
    use super::*;
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
        }
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
}
