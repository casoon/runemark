//! Internal helpers shared by the writer-based renderers.

use crate::color::{Console, Tone};
use std::fmt::Display;
use std::io::Write;

/// Runs `write` against an in-memory buffer and returns the result as a `String`.
///
/// Writing to a `Vec<u8>` cannot fail, and every writer in this crate only emits
/// UTF-8 text, so both steps are documented invariants rather than runtime risks.
pub(crate) fn collect_to_string(write: impl FnOnce(&mut Vec<u8>) -> std::io::Result<()>) -> String {
    let mut buf = Vec::new();
    write(&mut buf).expect("writing to a Vec<u8> is infallible");
    String::from_utf8(buf).expect("runemark writers only emit UTF-8 text")
}

/// Writes `prefix` followed by a single toned span and a trailing newline.
///
/// This is the recurring "indented, single-tone line" shape used across the
/// report and error-block renderers (e.g. a two-space-indented muted note).
pub(crate) fn write_toned_line(
    prefix: &str,
    console: Console,
    tone: Tone,
    text: impl Display,
    writer: &mut (impl Write + ?Sized),
) -> std::io::Result<()> {
    write!(writer, "{prefix}")?;
    console.write_paint(tone, text, writer)?;
    writeln!(writer)
}

/// Writes a leading space then a single toned span, with no trailing newline.
///
/// This is the recurring "optional same-line badge" shape (e.g. `[rule-id]`,
/// `(confidence)`, `[advisory]`) appended after other content on the same line.
pub(crate) fn write_toned_suffix(
    console: Console,
    tone: Tone,
    text: impl Display,
    writer: &mut (impl Write + ?Sized),
) -> std::io::Result<()> {
    write!(writer, " ")?;
    console.write_paint(tone, text, writer)
}
