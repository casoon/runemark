//! Raw mode and key decoding over `termios`, without a terminal crate.
//!
//! The menu needs very little from the terminal: raw mode, four escape
//! sequences, and a handful of keys. All of it is a thin layer over `libc`.
//!
//! Input comes from `/dev/tty` rather than stdin, so the menu still works when
//! the caller's stdin is redirected — `opi < /dev/null` should not disable
//! navigation.

use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;

/// Escape sequences the menu writes. Kept together so they are easy to audit.
pub mod escape {
    pub const HIDE_CURSOR: &str = "\x1b[?25l";
    pub const SHOW_CURSOR: &str = "\x1b[?25h";
    pub const CLEAR_TO_END: &str = "\x1b[J";

    /// Moves the cursor to the start of the line `lines` above.
    pub fn move_up(lines: u16) -> String {
        format!("\x1b[{lines}F")
    }
}

/// A key the menu acts on. Anything else is deliberately not modelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    /// Tab, and Shift-Tab as [`Key::BackTab`].
    Tab,
    BackTab,
    Enter,
    Escape,
    Interrupt,
    Backspace,
    Char(char),
    /// A recognised but unused key, or an escape sequence we skip.
    Other,
}

/// Whether failing to open `/dev/tty` with `error` means there is no terminal.
///
/// `ENXIO` is how a process detached from its terminal (after `setsid`) is
/// refused. Rust reports it as an uncategorised error, so the kind alone
/// cannot recognise it.
fn means_no_terminal(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
    ) || error.raw_os_error() == Some(libc::ENXIO)
}

/// The terminal, in raw mode for as long as this value lives.
///
/// Restoring is tied to the destructor so it happens on every path out,
/// including an unwinding panic. A signal that kills the process outright runs
/// no destructor; that limitation is documented on `Menu::run`.
pub struct RawTerminal {
    tty: File,
    original: libc::termios,
}

impl RawTerminal {
    /// Opens the controlling terminal and switches it to raw mode.
    ///
    /// Returns `Ok(None)` when there is no controlling terminal to open. A
    /// process detached from its terminal cannot be given a menu, and that is
    /// a fallback for the caller to render around — not an error it could act
    /// on. Failures that follow a successful open are reported, since those
    /// mean the terminal is there but did not behave.
    pub fn acquire() -> io::Result<Option<Self>> {
        let tty = match File::options().read(true).write(true).open("/dev/tty") {
            Ok(tty) => tty,
            Err(error) if means_no_terminal(&error) => return Ok(None),
            Err(error) => return Err(error),
        };
        let fd = tty.as_raw_fd();

        // SAFETY: `fd` is an open descriptor for the lifetime of `tty`, and
        // `termios` is a plain C struct the call fully initialises.
        let original = unsafe {
            let mut current: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(fd, &mut current) != 0 {
                return Err(io::Error::last_os_error());
            }
            current
        };

        let mut raw = original;
        // SAFETY: `raw` is a valid, initialised termios.
        unsafe {
            libc::cfmakeraw(&mut raw);
            // Block until at least one byte arrives; no inter-byte timer.
            raw.c_cc[libc::VMIN] = 1;
            raw.c_cc[libc::VTIME] = 0;
            // `TCSAFLUSH` drops what was typed before the menu opened, so a stray
            // Enter does not select the first entry. Only here: the later
            // timing changes must keep whatever has been typed since.
            if libc::tcsetattr(fd, libc::TCSAFLUSH, &raw) != 0 {
                return Err(io::Error::last_os_error());
            }
        }

        Ok(Some(Self { tty, original }))
    }

    /// The terminal's size in lines and columns, if it reports one.
    ///
    /// Queried per frame rather than cached. There is no `SIGWINCH` handler, so
    /// a resize is picked up on the next keypress, when the next frame is drawn.
    pub fn size(&self) -> Option<(usize, usize)> {
        // SAFETY: `winsize` is a plain C struct the ioctl fills completely.
        let size = unsafe {
            let mut size: libc::winsize = std::mem::zeroed();
            if libc::ioctl(self.tty.as_raw_fd(), libc::TIOCGWINSZ, &raw mut size) != 0 {
                return None;
            }
            size
        };
        (size.ws_row > 0).then_some((usize::from(size.ws_row), usize::from(size.ws_col)))
    }

    /// Reads one byte, retrying when a signal interrupts the call.
    fn read_byte(&self) -> io::Result<Option<u8>> {
        let mut byte = 0u8;
        loop {
            // SAFETY: writing one byte into a local we own.
            let read = unsafe {
                libc::read(
                    self.tty.as_raw_fd(),
                    std::ptr::from_mut(&mut byte).cast::<libc::c_void>(),
                    1,
                )
            };
            return match read {
                1 => Ok(Some(byte)),
                0 => Ok(None),
                _ => {
                    let error = io::Error::last_os_error();
                    if error.kind() == io::ErrorKind::Interrupted {
                        continue;
                    }
                    Err(error)
                }
            };
        }
    }

    /// Applies `VMIN`/`VTIME` to the terminal.
    fn set_read_timing(&self, min: u8, time: u8) -> io::Result<()> {
        let mut settings = self.original;
        // SAFETY: `settings` is a valid termios copy we own.
        unsafe {
            libc::cfmakeraw(&mut settings);
            settings.c_cc[libc::VMIN] = min;
            settings.c_cc[libc::VTIME] = time;
            if libc::tcsetattr(self.tty.as_raw_fd(), libc::TCSANOW, &settings) != 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    /// Switches reads to time out after roughly 100ms, until the guard drops.
    ///
    /// The kernel does the timing through `VTIME`, which is the portable way
    /// to bound a terminal read. `poll` and `select` are not usable here: on
    /// macOS a pty reports `POLLNVAL` rather than readability, so a poll-based
    /// timeout reads a byte that never arrives and hangs on a bare `Escape`.
    fn timed_reads(&self) -> io::Result<TimedReads<'_>> {
        self.set_read_timing(0, 1)?;
        Ok(TimedReads { terminal: self })
    }

    /// Blocks until the next key.
    pub fn read_key(&self) -> io::Result<Key> {
        let first = self.read_byte()?;
        decode_key(first, || self.timed_reads())
    }
}

/// Where the bytes after a key's first one come from.
///
/// A trait rather than the terminal itself so the decoder runs against a
/// scripted byte stream in tests. A read that yields `None` is a timeout or a
/// closed stream: no more bytes are coming.
trait ByteSource {
    fn read_byte(&mut self) -> io::Result<Option<u8>>;
}

/// Decodes the key that starts with `first`.
///
/// `first` is `None` when a blocking read returned nothing, which means the
/// terminal went away. Treating that as a cancel closes the menu rather than
/// spinning. `follow_up` supplies the bounded source for the bytes after the
/// first, and is only called for keys that have any.
fn decode_key<S: ByteSource>(
    first: Option<u8>,
    follow_up: impl FnOnce() -> io::Result<S>,
) -> io::Result<Key> {
    let Some(byte) = first else {
        return Ok(Key::Interrupt);
    };

    match byte {
        0x03 => Ok(Key::Interrupt),
        b'\r' | b'\n' => Ok(Key::Enter),
        0x09 => Ok(Key::Tab),
        0x1b => read_escape(&mut follow_up()?),
        // Terminals send either for Backspace depending on their settings.
        0x08 | 0x7f => Ok(Key::Backspace),
        // C0 controls other than the ones above carry no meaning here.
        0x00..=0x1f => Ok(Key::Other),
        _ => read_utf8(byte, &mut follow_up()?),
    }
}

/// Decodes what follows `0x1b`.
///
/// A bare `Escape` and the start of an arrow key are the same first byte;
/// only a bounded read tells them apart.
fn read_escape(source: &mut impl ByteSource) -> io::Result<Key> {
    let Some(second) = source.read_byte()? else {
        return Ok(Key::Escape);
    };

    // CSI (`ESC [`) and SS3 (`ESC O`) both introduce cursor keys; terminals
    // switch to SS3 in application cursor mode, which tmux and some others
    // turn on. Handling only CSI would break arrows there.
    if !matches!(second, b'[' | b'O') {
        return Ok(Key::Other);
    }

    let Some(third) = source.read_byte()? else {
        return Ok(Key::Other);
    };

    match third {
        b'A' => Ok(Key::Up),
        b'B' => Ok(Key::Down),
        b'C' => Ok(Key::Right),
        b'D' => Ok(Key::Left),
        b'H' => Ok(Key::Home),
        b'F' => Ok(Key::End),
        // Shift-Tab. It is the one CSI sequence here with no parameters,
        // so it is recognised beside the cursor keys rather than skipped.
        b'Z' => Ok(Key::BackTab),
        // `ESC [ 5 ~` and its kin: one digit and a tilde name a key. Anything
        // longer, such as `ESC [ 1 ; 2 A`, is consumed so its tail is not
        // mistaken for typed characters.
        b'0'..=b'9' => match source.read_byte()? {
            Some(b'~') => Ok(match third {
                b'1' => Key::Home,
                b'4' => Key::End,
                b'5' => Key::PageUp,
                b'6' => Key::PageDown,
                _ => Key::Other,
            }),
            Some(0x40..=0x7e) | None => Ok(Key::Other),
            Some(_) => {
                skip_sequence_tail(source)?;
                Ok(Key::Other)
            }
        },
        b';' => {
            skip_sequence_tail(source)?;
            Ok(Key::Other)
        }
        _ => Ok(Key::Other),
    }
}

/// Consumes the rest of a parameterised escape sequence, so its tail is not
/// mistaken for typed characters.
fn skip_sequence_tail(source: &mut impl ByteSource) -> io::Result<()> {
    // Parameter and intermediate bytes, then one final byte in 0x40..=0x7e.
    for _ in 0..16 {
        match source.read_byte()? {
            Some(0x40..=0x7e) | None => return Ok(()),
            Some(_) => {}
        }
    }
    Ok(())
}

/// Completes a UTF-8 character whose first byte is `first`.
fn read_utf8(first: u8, source: &mut impl ByteSource) -> io::Result<Key> {
    let width = match first {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        // A stray continuation byte: not the start of anything.
        _ => return Ok(Key::Other),
    };

    let mut bytes = vec![first];
    for _ in 1..width {
        match source.read_byte()? {
            Some(byte) => bytes.push(byte),
            None => return Ok(Key::Other),
        }
    }

    Ok(std::str::from_utf8(&bytes)
        .ok()
        .and_then(|text| text.chars().next())
        .map_or(Key::Other, Key::Char))
}

impl Drop for RawTerminal {
    fn drop(&mut self) {
        // SAFETY: `self.tty` is still open, and `original` is the struct
        // `tcgetattr` produced for this same descriptor.
        unsafe {
            libc::tcsetattr(self.tty.as_raw_fd(), libc::TCSANOW, &self.original);
        }
    }
}

/// Bounded reads, restoring blocking behaviour when dropped.
struct TimedReads<'a> {
    terminal: &'a RawTerminal,
}

impl ByteSource for TimedReads<'_> {
    /// Reads one byte, or `None` once the kernel's timer expires.
    fn read_byte(&mut self) -> io::Result<Option<u8>> {
        self.terminal.read_byte()
    }
}

impl Drop for TimedReads<'_> {
    fn drop(&mut self) {
        let _ = self.terminal.set_read_timing(1, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::VecDeque;

    impl ByteSource for &mut VecDeque<u8> {
        fn read_byte(&mut self) -> io::Result<Option<u8>> {
            Ok(self.pop_front())
        }
    }

    /// The key `bytes` decode to, and the bytes the decoder left unread.
    fn decode(bytes: &[u8]) -> (Key, Vec<u8>) {
        let mut rest: VecDeque<u8> = bytes.iter().skip(1).copied().collect();
        let key = decode_key(bytes.first().copied(), || Ok(&mut rest)).unwrap();
        (key, rest.into_iter().collect())
    }

    fn key(bytes: &[u8]) -> Key {
        decode(bytes).0
    }

    #[test]
    fn a_bare_escape_is_escape() {
        assert_eq!(key(b"\x1b"), Key::Escape);
    }

    #[test]
    fn the_arrows_decode_in_both_encodings() {
        for (tail, expected) in [
            (b'A', Key::Up),
            (b'B', Key::Down),
            (b'C', Key::Right),
            (b'D', Key::Left),
        ] {
            assert_eq!(key(&[0x1b, b'[', tail]), expected, "CSI");
            assert_eq!(key(&[0x1b, b'O', tail]), expected, "SS3");
        }
    }

    #[test]
    fn home_and_end_decode_in_both_encodings() {
        assert_eq!(key(b"\x1b[H"), Key::Home);
        assert_eq!(key(b"\x1b[F"), Key::End);
        assert_eq!(key(b"\x1bOH"), Key::Home);
        assert_eq!(key(b"\x1bOF"), Key::End);
        assert_eq!(key(b"\x1b[1~"), Key::Home);
        assert_eq!(key(b"\x1b[4~"), Key::End);
    }

    #[test]
    fn page_up_and_page_down_decode() {
        assert_eq!(decode(b"\x1b[5~"), (Key::PageUp, vec![]));
        assert_eq!(decode(b"\x1b[6~"), (Key::PageDown, vec![]));
    }

    #[test]
    fn the_tilde_keys_leave_what_follows_for_the_next_key() {
        assert_eq!(decode(b"\x1b[5~x"), (Key::PageUp, vec![b'x']));
    }

    #[test]
    fn other_tilde_keys_and_modified_ones_are_skipped_whole() {
        // Delete and Insert mean nothing here; a modified Home is not Home.
        assert_eq!(decode(b"\x1b[3~"), (Key::Other, vec![]));
        assert_eq!(decode(b"\x1b[2~"), (Key::Other, vec![]));
        assert_eq!(decode(b"\x1b[1;5H"), (Key::Other, vec![]));
        assert_eq!(decode(b"\x1b[15~"), (Key::Other, vec![]));
        assert_eq!(decode(b"\x1b[5;2~"), (Key::Other, vec![]));
    }

    #[test]
    fn ctrl_z_arrives_as_a_key_that_means_nothing() {
        // Raw mode clears ISIG, so the terminal does not suspend the menu.
        assert_eq!(key(&[0x1a]), Key::Other);
    }

    #[test]
    fn shift_tab_is_back_tab() {
        assert_eq!(key(b"\x1b[Z"), Key::BackTab);
    }

    #[test]
    fn a_parameterised_sequence_is_skipped_whole() {
        // Its tail must not be left behind to be read as typed characters.
        assert_eq!(decode(b"\x1b[1;2A"), (Key::Other, vec![]));
        assert_eq!(decode(b"\x1b[1;2B"), (Key::Other, vec![]));
        assert_eq!(decode(b"\x1b[1;2Bx"), (Key::Other, vec![b'x']));
    }

    #[test]
    fn an_escape_sequence_cut_short_is_skipped() {
        assert_eq!(key(b"\x1b["), Key::Other);
        assert_eq!(key(b"\x1b[1;"), Key::Other);
        assert_eq!(key(b"\x1bx"), Key::Other);
        assert_eq!(key(b"\x1b[Q"), Key::Other);
    }

    #[test]
    fn multi_byte_characters_decode_whole() {
        assert_eq!(key("ä".as_bytes()), Key::Char('ä'));
        assert_eq!(key("€".as_bytes()), Key::Char('€'));
        assert_eq!(key("😀".as_bytes()), Key::Char('😀'));
        assert_eq!(key(b"a"), Key::Char('a'));
    }

    #[test]
    fn a_truncated_or_invalid_character_is_skipped() {
        assert_eq!(key(&"€".as_bytes()[..2]), Key::Other, "cut short");
        assert_eq!(key(&[0xc3, 0x28]), Key::Other, "bad continuation byte");
    }

    #[test]
    fn a_stray_continuation_byte_is_not_the_start_of_anything() {
        assert_eq!(decode(&[0x80, b'a']), (Key::Other, vec![b'a']));
    }

    #[test]
    fn the_control_keys_decode() {
        assert_eq!(key(&[0x03]), Key::Interrupt);
        assert_eq!(key(b"\r"), Key::Enter);
        assert_eq!(key(b"\n"), Key::Enter);
        assert_eq!(key(b"\t"), Key::Tab);
        assert_eq!(key(&[0x7f]), Key::Backspace);
        assert_eq!(key(&[0x08]), Key::Backspace);
        assert_eq!(key(&[0x01]), Key::Other, "other controls mean nothing");
    }

    #[test]
    fn a_closed_terminal_reads_as_a_cancel() {
        // A zero-length blocking read means the terminal went away.
        assert_eq!(key(b""), Key::Interrupt);
    }

    #[test]
    fn a_key_with_no_follow_up_bytes_never_asks_for_them() {
        // Asking switches the terminal's read timing, which is not free.
        let never = || -> io::Result<&mut VecDeque<u8>> { panic!("asked for follow-up bytes") };
        assert_eq!(decode_key(Some(b'\r'), never).unwrap(), Key::Enter);
    }

    #[test]
    fn a_missing_controlling_terminal_is_not_an_error() {
        assert!(means_no_terminal(&io::Error::from_raw_os_error(
            libc::ENXIO
        )));
        assert!(means_no_terminal(&io::Error::from(io::ErrorKind::NotFound)));
        assert!(means_no_terminal(&io::Error::from(
            io::ErrorKind::PermissionDenied
        )));
    }

    #[test]
    fn other_failures_to_open_the_terminal_are_reported() {
        assert!(!means_no_terminal(&io::Error::from_raw_os_error(libc::EIO)));
    }
}
