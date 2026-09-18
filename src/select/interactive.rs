//! The interactive loop: draw, read a key, act, redraw.

use std::io::{self, Write};

use super::terminal::{Key, RawTerminal, escape};

use super::{Menu, Outcome, SelectMode};
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
        let mut index = 0usize;
        let last = self.len() - 1;
        let mut drawn = false;

        loop {
            self.draw(console, index, drawn)?;
            drawn = true;

            match self.act_on(terminal.read_key()?, index, last) {
                Action::Move(next) => index = next,
                Action::Finish(outcome) => {
                    self.draw(console, index, true)?;
                    return Ok(outcome);
                }
                Action::Ignore => {}
            }
        }
    }

    fn act_on(&self, key: Key, index: usize, last: usize) -> Action {
        match key {
            // Wrapping beats stopping at the ends: the list is short and a dead
            // key at the bottom is the more annoying failure.
            Key::Up => Action::Move(if index == 0 { last } else { index - 1 }),
            Key::Down => Action::Move(if index == last { 0 } else { index + 1 }),
            Key::Enter => self.items().nth(index).map_or(Action::Ignore, |item| {
                Action::Finish(Outcome::Selected(item.id.clone()))
            }),
            Key::Escape | Key::Interrupt => Action::Finish(Outcome::Cancelled),
            Key::Char(pressed) => {
                // Hint keys win over the built-in 'q', so a menu may bind 'q'.
                if let Some(hint) = self
                    .hints
                    .iter()
                    .find(|hint| hint.key.eq_ignore_ascii_case(&pressed))
                {
                    Action::Finish(Outcome::Hotkey(hint.key))
                } else if pressed == 'q' {
                    Action::Finish(Outcome::Cancelled)
                } else {
                    Action::Ignore
                }
            }
            Key::Other => Action::Ignore,
        }
    }

    /// Draws the menu in place, overwriting the previous frame.
    fn draw(&self, console: Console, index: usize, redraw: bool) -> io::Result<()> {
        let mut stderr = io::stderr();

        if redraw {
            write!(
                stderr,
                "{}{}",
                escape::move_up(self.frame_height()),
                escape::CLEAR_TO_END
            )?;
        }

        // Raw mode drops the implicit carriage return on newline, so the frame
        // is rendered normally and then given explicit ones.
        let frame =
            crate::internal::collect_to_string(|buf| self.write_frame(buf, console, Some(index)));
        for line in frame.lines() {
            write!(stderr, "{line}\r\n")?;
        }

        stderr.flush()
    }
}

enum Action {
    Move(usize),
    Finish(Outcome),
    Ignore,
}
