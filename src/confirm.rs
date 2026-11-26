//! Interactive confirmation prompts (`select` feature).
//!
//! For the step a CLI must not take by accident: approving a plan, deleting data,
//! sending something that cannot be recalled. A [`Confirm`] asks `y/N`; a
//! [`Challenge`] asks for a short code the user has to read and type.
//!
//! A confirmation that anything can answer is no confirmation. Both prompts read
//! the answer from a terminal and refuse to run without one: [`Answer::Unavailable`]
//! comes back instead of a guess, so a script that pipes `yes` into the command
//! cannot approve on the user's behalf. Treat `Unavailable` as "not confirmed".
//!
//! The prompt is drawn on standard error, like the [menu](crate::Menu), so
//! standard output stays free for a command's result. Input is plain line-based
//! standard input; no raw mode and no dependency beyond the standard library.

use crate::color::{Console, Tone};
use crate::internal::single_line;
use crate::verdict::Verdict;
use std::io::{BufRead, IsTerminal, Write};

/// What the user answered a [`Confirm`] or [`Challenge`].
///
/// Exhaustive on purpose, like [`Outcome`](crate::Outcome): callers match it, and a
/// new case should be a compile error, not a silent fallthrough into "yes".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    /// The user said yes, or typed the expected code.
    Confirmed,
    /// The user said no, typed another code, or closed the input (`Ctrl-D`).
    Declined,
    /// The prompt did not run, because stdin or stderr is not a terminal. Nothing
    /// was asked and nothing was read; the caller decides what to tell the user,
    /// typically that the command has to be run in a terminal.
    Unavailable,
}

/// A semantic line above the prompt, such as `⚠ Approve plan 3f2a1c`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Header {
    verdict: Verdict,
    text: String,
}

impl Header {
    fn write_to(&self, console: Console, out: &mut (impl Write + ?Sized)) -> std::io::Result<()> {
        self.verdict
            .write_to(console, &single_line(&self.text), out)?;
        writeln!(out)
    }
}

fn write_header(
    header: &Option<Header>,
    console: Console,
    out: &mut (impl Write + ?Sized),
) -> std::io::Result<()> {
    match header {
        Some(header) => header.write_to(console, out),
        None => Ok(()),
    }
}

/// Reads one line, or `None` when the input is closed.
fn read_line(input: &mut (impl BufRead + ?Sized)) -> std::io::Result<Option<String>> {
    let mut line = String::new();
    Ok((input.read_line(&mut line)? > 0).then_some(line))
}

/// Both streams a prompt needs: stdin to read the answer from, stderr to show the question on.
fn on_terminal() -> bool {
    std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

/// A yes/no question.
///
/// ```no_run
/// use runemark::{Answer, ColorMode, Confirm, Console, Verdict};
///
/// # fn main() -> std::io::Result<()> {
/// let answer = Confirm::new("Apply 12 changes?")
///     .with_header(Verdict::Warning, "Approve plan 3f2a1c")
///     .run(Console::stderr(ColorMode::Auto))?;
///
/// match answer {
///     Answer::Confirmed => println!("applying"),
///     Answer::Declined => println!("cancelled"),
///     Answer::Unavailable => eprintln!("run this command in a terminal to approve"),
/// }
/// # Ok(())
/// # }
/// ```
///
/// The answer is `y`, `yes`, `n` or `no`, in any case. An empty answer takes the
/// default, which is no unless [`Confirm::with_default`] says otherwise; anything else
/// asks again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Confirm {
    prompt: String,
    default: bool,
    header: Option<Header>,
}

impl Confirm {
    /// Creates a question. The default answer is no.
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            default: false,
            header: None,
        }
    }

    /// Sets what an empty answer means. The prompt shows the default as a capital:
    /// `[y/N]` for no, `[Y/n]` for yes.
    pub fn with_default(mut self, default: bool) -> Self {
        self.default = default;
        self
    }

    /// Adds a line above the prompt, led by the verdict's symbol, to say what is at
    /// stake: `Verdict::Warning` for a change, `Verdict::Failed` for something that
    /// cannot be undone.
    pub fn with_header(mut self, verdict: Verdict, text: impl Into<String>) -> Self {
        self.header = Some(Header {
            verdict,
            text: text.into(),
        });
        self
    }

    /// Asks the question on standard error and reads the answer from standard input.
    ///
    /// Returns [`Answer::Unavailable`] without asking when stdin or stderr is not a
    /// terminal. Pass `Console::stderr(..)`, since that is where the prompt is drawn.
    pub fn run(&self, console: Console) -> std::io::Result<Answer> {
        self.run_on(
            console,
            on_terminal(),
            &mut std::io::stdin().lock(),
            &mut std::io::stderr(),
        )
    }

    /// `run` with the terminal check and the streams supplied, so tests need no terminal.
    fn run_on(
        &self,
        console: Console,
        interactive: bool,
        input: &mut (impl BufRead + ?Sized),
        out: &mut (impl Write + ?Sized),
    ) -> std::io::Result<Answer> {
        if !interactive {
            return Ok(Answer::Unavailable);
        }
        write_header(&self.header, console, out)?;
        let hint = if self.default { "[Y/n]" } else { "[y/N]" };
        loop {
            write!(out, "{} ", single_line(&self.prompt))?;
            console.write_paint(Tone::Muted, hint, out)?;
            write!(out, " ")?;
            out.flush()?;

            let Some(line) = read_line(input)? else {
                writeln!(out)?;
                return Ok(Answer::Declined);
            };
            match line.trim().to_lowercase().as_str() {
                "" if self.default => return Ok(Answer::Confirmed),
                "" | "n" | "no" => return Ok(Answer::Declined),
                "y" | "yes" => return Ok(Answer::Confirmed),
                _ => {
                    console.write_paint(Tone::Warning, "Please answer y or n.", out)?;
                    writeln!(out)?;
                }
            }
        }
    }
}

/// A prompt that is only confirmed by typing a code.
///
/// Use it where `y` is too easy to type without reading: the code is something the
/// user has to take from the screen, such as the first characters of a plan's hash.
/// The comparison is exact after trimming surrounding whitespace. There is one
/// attempt; a different code is [`Answer::Declined`].
///
/// ```no_run
/// use runemark::{Answer, Challenge, ColorMode, Console, Verdict};
///
/// # fn main() -> std::io::Result<()> {
/// let answer = Challenge::new("Type 3f2a1c to approve:", "3f2a1c")
///     .with_header(Verdict::Failed, "This deletes 12 notes and cannot be undone")
///     .run(Console::stderr(ColorMode::Auto))?;
/// # let _ = answer;
/// # Ok(())
/// # }
/// ```
///
/// The prompt text is yours: it says what to type, and the expected code is not
/// added to it. An empty expected code never matches, so pressing Enter cannot confirm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Challenge {
    prompt: String,
    expected: String,
    header: Option<Header>,
}

impl Challenge {
    /// Creates a prompt that is confirmed by typing `expected_code`.
    pub fn new(prompt: impl Into<String>, expected_code: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            expected: expected_code.into(),
            header: None,
        }
    }

    /// Adds a line above the prompt, led by the verdict's symbol, to say what is at stake.
    pub fn with_header(mut self, verdict: Verdict, text: impl Into<String>) -> Self {
        self.header = Some(Header {
            verdict,
            text: text.into(),
        });
        self
    }

    /// Asks on standard error and reads the answer from standard input.
    ///
    /// Returns [`Answer::Unavailable`] without asking when stdin or stderr is not a
    /// terminal. Pass `Console::stderr(..)`, since that is where the prompt is drawn.
    pub fn run(&self, console: Console) -> std::io::Result<Answer> {
        self.run_on(
            console,
            on_terminal(),
            &mut std::io::stdin().lock(),
            &mut std::io::stderr(),
        )
    }

    /// `run` with the terminal check and the streams supplied, so tests need no terminal.
    fn run_on(
        &self,
        console: Console,
        interactive: bool,
        input: &mut (impl BufRead + ?Sized),
        out: &mut (impl Write + ?Sized),
    ) -> std::io::Result<Answer> {
        if !interactive {
            return Ok(Answer::Unavailable);
        }
        write_header(&self.header, console, out)?;
        write!(out, "{} ", single_line(&self.prompt))?;
        out.flush()?;

        let Some(line) = read_line(input)? else {
            writeln!(out)?;
            return Ok(Answer::Declined);
        };
        let typed = line.trim();
        Ok(if !self.expected.is_empty() && typed == self.expected {
            Answer::Confirmed
        } else {
            Answer::Declined
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::ColorMode;

    fn plain() -> Console {
        Console::new(ColorMode::Never, false)
    }

    /// Runs a confirm against canned input; returns the answer and what was drawn.
    fn confirm(question: &Confirm, input: &str) -> (Answer, String) {
        let mut out = Vec::new();
        let answer = question
            .run_on(plain(), true, &mut input.as_bytes(), &mut out)
            .unwrap();
        (answer, String::from_utf8(out).unwrap())
    }

    fn challenge(question: &Challenge, input: &str) -> (Answer, String) {
        let mut out = Vec::new();
        let answer = question
            .run_on(plain(), true, &mut input.as_bytes(), &mut out)
            .unwrap();
        (answer, String::from_utf8(out).unwrap())
    }

    #[test]
    fn yes_and_no_are_accepted_in_any_case() {
        let question = Confirm::new("Go?");
        for yes in ["y\n", "yes\n", "Y\n", " YES \n"] {
            assert_eq!(confirm(&question, yes).0, Answer::Confirmed, "{yes:?}");
        }
        for no in ["n\n", "no\n", "N\n", " No \n"] {
            assert_eq!(confirm(&question, no).0, Answer::Declined, "{no:?}");
        }
    }

    #[test]
    fn an_empty_answer_takes_the_default_and_the_hint_shows_it() {
        let no = Confirm::new("Go?");
        assert_eq!(confirm(&no, "\n"), (Answer::Declined, "Go? [y/N] ".into()));
        let yes = Confirm::new("Go?").with_default(true);
        assert_eq!(
            confirm(&yes, "\n"),
            (Answer::Confirmed, "Go? [Y/n] ".into())
        );
    }

    #[test]
    fn an_unrecognised_answer_asks_again() {
        let (answer, drawn) = confirm(&Confirm::new("Go?"), "maybe\ny\n");
        assert_eq!(answer, Answer::Confirmed);
        assert_eq!(drawn, "Go? [y/N] Please answer y or n.\nGo? [y/N] ");
    }

    #[test]
    fn closed_input_declines_instead_of_looping_or_confirming() {
        let (answer, drawn) = confirm(&Confirm::new("Go?").with_default(true), "");
        assert_eq!(answer, Answer::Declined);
        assert_eq!(drawn, "Go? [Y/n] \n");
        assert_eq!(confirm(&Confirm::new("Go?"), "maybe\n").0, Answer::Declined);
    }

    #[test]
    fn a_header_is_drawn_above_the_prompt() {
        let question = Confirm::new("Apply?").with_header(Verdict::Warning, "Approve plan 3f2a1c");
        let (_, drawn) = confirm(&question, "n\n");
        assert_eq!(drawn, "[WARN] Approve plan 3f2a1c\nApply? [y/N] ");
    }

    #[test]
    fn without_a_terminal_nothing_is_asked_and_nothing_is_read() {
        let question = Confirm::new("Go?");
        let mut input: &[u8] = b"y\n";
        let mut out = Vec::new();
        let answer = question
            .run_on(plain(), false, &mut input, &mut out)
            .unwrap();
        assert_eq!(answer, Answer::Unavailable);
        assert!(out.is_empty());
        assert_eq!(input, b"y\n", "the input must not be consumed");

        let challenge = Challenge::new("Type it:", "abc123");
        let mut input: &[u8] = b"abc123\n";
        let answer = challenge
            .run_on(plain(), false, &mut input, &mut out)
            .unwrap();
        assert_eq!(answer, Answer::Unavailable);
        assert!(out.is_empty());
    }

    #[test]
    fn a_challenge_is_confirmed_only_by_the_exact_code() {
        let question = Challenge::new("Type 3f2a1c:", "3f2a1c");
        assert_eq!(
            challenge(&question, "3f2a1c\n"),
            (Answer::Confirmed, "Type 3f2a1c: ".into())
        );
        assert_eq!(challenge(&question, "  3f2a1c  \n").0, Answer::Confirmed);
        for wrong in ["3F2A1C\n", "3f2a1\n", "y\n", "\n"] {
            assert_eq!(challenge(&question, wrong).0, Answer::Declined, "{wrong:?}");
        }
    }

    #[test]
    fn an_empty_expected_code_can_never_be_confirmed() {
        let question = Challenge::new("Type it:", "");
        assert_eq!(challenge(&question, "\n").0, Answer::Declined);
    }

    #[test]
    fn a_challenge_with_closed_input_declines() {
        let (answer, drawn) = challenge(&Challenge::new("Type it:", "abc"), "");
        assert_eq!(answer, Answer::Declined);
        assert_eq!(drawn, "Type it: \n");
    }

    #[test]
    fn a_challenge_header_is_drawn_above_the_prompt() {
        let question = Challenge::new("Type abc:", "abc")
            .with_header(Verdict::Failed, "This cannot be undone");
        let (_, drawn) = challenge(&question, "abc\n");
        assert_eq!(drawn, "[FAIL] This cannot be undone\nType abc: ");
    }

    #[test]
    fn prompt_and_header_text_is_single_line_and_neutralised() {
        let question = Confirm::new("Go\n\x1b[31m?").with_header(Verdict::Info, "a\nb");
        let (_, drawn) = confirm(&question, "n\n");
        assert_eq!(drawn, "[INFO] a b\nGo ^[[31m? [y/N] ");
    }
}
