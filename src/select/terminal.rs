//! Raw mode and key decoding over `termios`, without a terminal crate.
//!
//! The menu needs very little from the terminal: raw mode, four escape
//! sequences, and six keys. All of it is a thin layer over `libc`.
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
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::NotFound | io::ErrorKind::PermissionDenied
                ) =>
            {
                return Ok(None);
            }
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
            if libc::tcsetattr(fd, libc::TCSANOW, &raw) != 0 {
                return Err(io::Error::last_os_error());
            }
        }

        Ok(Some(Self { tty, original }))
    }

    /// The terminal's size in lines and columns, if it reports one.
    ///
    /// Queried per frame rather than cached, so resizing the window while the
    /// menu is open is picked up without a `SIGWINCH` handler.
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
        // In blocking mode a zero-byte read means the terminal went away.
        // Treating that as a cancel closes the menu rather than spinning.
        let Some(byte) = self.read_byte()? else {
            return Ok(Key::Interrupt);
        };

        match byte {
            0x03 => Ok(Key::Interrupt),
            b'\r' | b'\n' => Ok(Key::Enter),
            0x09 => Ok(Key::Tab),
            0x1b => self.read_escape(),
            // Terminals send either for Backspace depending on their settings.
            0x08 | 0x7f => Ok(Key::Backspace),
            // C0 controls other than the ones above carry no meaning here.
            0x00..=0x1f => Ok(Key::Other),
            _ => self.read_utf8(byte),
        }
    }

    /// Decodes what follows `0x1b`.
    ///
    /// A bare `Escape` and the start of an arrow key are the same first byte;
    /// only a bounded read tells them apart.
    fn read_escape(&self) -> io::Result<Key> {
        let timed = self.timed_reads()?;

        let Some(second) = timed.read_byte()? else {
            return Ok(Key::Escape);
        };

        // CSI (`ESC [`) and SS3 (`ESC O`) both introduce cursor keys; terminals
        // switch to SS3 in application cursor mode, which tmux and some others
        // turn on. Handling only CSI would break arrows there.
        if !matches!(second, b'[' | b'O') {
            return Ok(Key::Other);
        }

        let Some(third) = timed.read_byte()? else {
            return Ok(Key::Other);
        };

        match third {
            b'A' => Ok(Key::Up),
            b'B' => Ok(Key::Down),
            b'C' => Ok(Key::Right),
            b'D' => Ok(Key::Left),
            // Shift-Tab. It is the one CSI sequence here with no parameters,
            // so it is recognised beside the cursor keys rather than skipped.
            b'Z' => Ok(Key::BackTab),
            // A parameterised sequence such as `ESC [ 1 ; 2 A` or `ESC [ 5 ~`.
            // Consume it so its tail is not mistaken for typed characters.
            b'0'..=b'9' | b';' => {
                timed.skip_sequence_tail()?;
                Ok(Key::Other)
            }
            _ => Ok(Key::Other),
        }
    }

    /// Completes a UTF-8 character whose first byte is `first`.
    fn read_utf8(&self, first: u8) -> io::Result<Key> {
        let width = match first {
            0x00..=0x7f => 1,
            0xc0..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf7 => 4,
            // A stray continuation byte: not the start of anything.
            _ => return Ok(Key::Other),
        };

        let timed = self.timed_reads()?;
        let mut bytes = vec![first];
        for _ in 1..width {
            match timed.read_byte()? {
                Some(byte) => bytes.push(byte),
                None => return Ok(Key::Other),
            }
        }

        Ok(std::str::from_utf8(&bytes)
            .ok()
            .and_then(|text| text.chars().next())
            .map_or(Key::Other, Key::Char))
    }
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

impl TimedReads<'_> {
    /// Reads one byte, or `None` once the kernel's timer expires.
    fn read_byte(&self) -> io::Result<Option<u8>> {
        self.terminal.read_byte()
    }

    /// Consumes the rest of a parameterised escape sequence, so its tail is
    /// not mistaken for typed characters.
    fn skip_sequence_tail(&self) -> io::Result<()> {
        // Parameter and intermediate bytes, then one final byte in 0x40..=0x7e.
        for _ in 0..16 {
            match self.read_byte()? {
                Some(0x40..=0x7e) | None => return Ok(()),
                Some(_) => {}
            }
        }
        Ok(())
    }
}

impl Drop for TimedReads<'_> {
    fn drop(&mut self) {
        let _ = self.terminal.set_read_timing(1, 0);
    }
}
