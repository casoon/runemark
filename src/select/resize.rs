//! Noticing a terminal resize while the menu waits for a key.
//!
//! A `SIGWINCH` handler writes one byte to a pipe, and the wait for the next
//! key watches the terminal and that pipe together. The handler does nothing
//! else: `write` is async-signal-safe, and everything that is not (reading the
//! size, redrawing) happens back in the menu's own loop.
//!
//! The wait uses `select` rather than `poll`: on macOS `poll` reports
//! `POLLNVAL` for `/dev/tty` at once, which would read as input that never
//! arrives. `select` is the portable choice there, at the price of its
//! descriptor limit, which [`ResizeWatch::install`] respects.

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::time::Duration;

/// Write end of the pipe the handler pokes; `-1` while no watch is installed.
static WAKE_FD: AtomicI32 = AtomicI32::new(-1);
/// The handler that was installed before ours, as `sa_sigaction`; `0` or `1`
/// (`SIG_DFL`, `SIG_IGN`) when there is nothing to chain to.
static PREVIOUS: AtomicUsize = AtomicUsize::new(0);
/// Whether `PREVIOUS` takes the three `SA_SIGINFO` arguments.
static PREVIOUS_TAKES_INFO: AtomicBool = AtomicBool::new(false);
/// Whether a watch exists. The handler and its pipe are process-wide, so only
/// one menu at a time can have them.
static INSTALLED: AtomicBool = AtomicBool::new(false);

type PlainHandler = extern "C" fn(libc::c_int);
type InfoHandler = extern "C" fn(libc::c_int, *mut libc::siginfo_t, *mut libc::c_void);

extern "C" fn on_winch(
    signal: libc::c_int,
    info: *mut libc::siginfo_t,
    context: *mut libc::c_void,
) {
    let fd = WAKE_FD.load(Ordering::Relaxed);
    if fd >= 0 {
        let byte = 1u8;
        // SAFETY: `write` is async-signal-safe and `byte` outlives the call.
        // The pipe is non-blocking, so a full pipe drops this byte — one
        // pending wake-up is all the menu needs. A successful `write` leaves
        // `errno` alone, and the pipe is drained on every wake-up, so the
        // failing case cannot arise in practice.
        unsafe {
            libc::write(fd, (&raw const byte).cast::<libc::c_void>(), 1);
        }
    }

    // The host's own handler still hears about the resize.
    let previous = PREVIOUS.load(Ordering::Relaxed);
    if previous > 1 {
        // SAFETY: `previous` came from `sa_sigaction` of an action that was
        // installed with, or without, `SA_SIGINFO` as recorded beside it.
        unsafe {
            if PREVIOUS_TAKES_INFO.load(Ordering::Relaxed) {
                std::mem::transmute::<usize, InfoHandler>(previous)(signal, info, context);
            } else {
                std::mem::transmute::<usize, PlainHandler>(previous)(signal);
            }
        }
    }
}

/// A `SIGWINCH` handler installed for as long as this value lives.
///
/// Restoring the previous action is tied to the destructor, like
/// `RawTerminal`'s termios, so it happens on every path out including an
/// unwinding panic.
pub struct ResizeWatch {
    wake: OwnedFd,
    _notify: OwnedFd,
    previous: libc::sigaction,
}

impl ResizeWatch {
    /// Installs the handler, or returns `None` where the menu should do
    /// without: another watch is live, no pipe could be made, or a descriptor
    /// is too large for `select`. Without a watch the menu still works and
    /// notices a resize at the next keypress.
    pub fn install() -> Option<Self> {
        if INSTALLED
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return None;
        }
        let watch = Self::set_up();
        if watch.is_none() {
            INSTALLED.store(false, Ordering::Release);
        }
        watch
    }

    fn set_up() -> Option<Self> {
        let (wake, notify) = pipe()?;
        let limit = RawFd::try_from(libc::FD_SETSIZE).unwrap_or(RawFd::MAX);
        if wake.as_raw_fd() >= limit {
            return None;
        }

        WAKE_FD.store(notify.as_raw_fd(), Ordering::Relaxed);

        // SAFETY: both structs are plain C data the calls fill or read, and
        // `on_winch` has the `SA_SIGINFO` signature the flag promises.
        let previous = unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = on_winch as *const () as usize;
            libc::sigemptyset(&mut action.sa_mask);
            // `SA_RESTART` keeps the host's other blocking calls from seeing
            // `EINTR` it would not otherwise get; the menu's own wait copes
            // with it either way.
            action.sa_flags = libc::SA_SIGINFO | libc::SA_RESTART;

            let mut previous: libc::sigaction = std::mem::zeroed();
            if libc::sigaction(libc::SIGWINCH, &action, &mut previous) != 0 {
                WAKE_FD.store(-1, Ordering::Relaxed);
                return None;
            }
            previous
        };

        PREVIOUS_TAKES_INFO.store(previous.sa_flags & libc::SA_SIGINFO != 0, Ordering::Relaxed);
        PREVIOUS.store(previous.sa_sigaction, Ordering::Relaxed);

        Some(Self {
            wake,
            _notify: notify,
            previous,
        })
    }

    /// The descriptor that becomes readable once a resize has been signalled.
    fn fd(&self) -> RawFd {
        self.wake.as_raw_fd()
    }

    /// Empties the pipe, so the next wait blocks again.
    fn drain(&self) {
        let mut sink = [0u8; 64];
        loop {
            // SAFETY: reading into a buffer we own, from a descriptor we own.
            let read = unsafe {
                libc::read(
                    self.fd(),
                    sink.as_mut_ptr().cast::<libc::c_void>(),
                    sink.len(),
                )
            };
            if read <= 0 || usize::try_from(read).is_ok_and(|n| n < sink.len()) {
                return;
            }
        }
    }
}

impl Drop for ResizeWatch {
    fn drop(&mut self) {
        // SAFETY: `previous` is the action `sigaction` reported when ours
        // replaced it. A handler already running on another thread may still
        // write once more; the pipe is closed after the fd is withdrawn, which
        // narrows that to a window no menu will hit.
        unsafe {
            libc::sigaction(libc::SIGWINCH, &self.previous, std::ptr::null_mut());
        }
        WAKE_FD.store(-1, Ordering::Relaxed);
        PREVIOUS.store(0, Ordering::Relaxed);
        PREVIOUS_TAKES_INFO.store(false, Ordering::Relaxed);
        INSTALLED.store(false, Ordering::Release);
    }
}

/// A non-blocking, close-on-exec pipe: `(read end, write end)`.
fn pipe() -> Option<(OwnedFd, OwnedFd)> {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: `fds` has room for the two descriptors `pipe` writes.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return None;
    }
    // SAFETY: both descriptors are fresh from `pipe` and owned by nobody else.
    let (read, write) = unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) };
    for fd in [&read, &write] {
        // SAFETY: `fd` is open; these only change its flags.
        let failed = unsafe {
            libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) != 0
                || libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) != 0
        };
        if failed {
            return None;
        }
    }
    Some((read, write))
}

/// What a wait found ready.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Ready {
    pub input: bool,
    pub resize: bool,
}

/// Whether `fd` can go into a `select` set.
pub fn selectable(fd: RawFd) -> bool {
    usize::try_from(fd).is_ok_and(|fd| fd < libc::FD_SETSIZE)
}

/// Blocks until `input` has something to read or a resize was signalled.
///
/// A signal that interrupts the wait returns nothing ready; the caller waits
/// again, and a resize it announced is still in the pipe.
pub fn wait(input: RawFd, watch: &ResizeWatch) -> io::Result<Ready> {
    select_ready(input, watch, None)
}

/// [`wait`], giving up after `timeout` if there is one. Tests use a zero
/// timeout to ask what is ready right now.
fn select_ready(input: RawFd, watch: &ResizeWatch, timeout: Option<Duration>) -> io::Result<Ready> {
    let pipe = watch.fd();
    let mut limit = timeout.map(|timeout| libc::timeval {
        tv_sec: libc::time_t::try_from(timeout.as_secs()).unwrap_or(libc::time_t::MAX),
        tv_usec: libc::suseconds_t::try_from(timeout.subsec_micros()).unwrap_or(0),
    });
    // SAFETY: `set` is initialised by `FD_ZERO` before use, both descriptors
    // are open and below `FD_SETSIZE` (checked when the watch was made and
    // when the terminal was acquired).
    let (input_ready, resize) = unsafe {
        let mut set: libc::fd_set = std::mem::zeroed();
        libc::FD_ZERO(&mut set);
        libc::FD_SET(input, &mut set);
        libc::FD_SET(pipe, &mut set);
        let ready = libc::select(
            input.max(pipe) + 1,
            &mut set,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            limit
                .as_mut()
                .map_or(std::ptr::null_mut(), std::ptr::from_mut),
        );
        if ready < 0 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                return Ok(Ready::default());
            }
            return Err(error);
        }
        (libc::FD_ISSET(input, &set), libc::FD_ISSET(pipe, &set))
    };
    if resize {
        watch.drain();
    }
    Ok(Ready {
        input: input_ready,
        resize,
    })
}

/// What the menu is woken by.
#[derive(Debug, PartialEq, Eq)]
pub enum Event<T> {
    Input(T),
    Resize,
}

/// Waits until there is input or a resize, and reads the input if that is it.
///
/// Input wins when both are ready: the redraw that follows the key reads the
/// new size anyway, so a separate one would only draw the same frame twice.
pub fn next_event<T>(
    mut wait: impl FnMut() -> io::Result<Ready>,
    read_input: impl FnOnce() -> io::Result<T>,
) -> io::Result<Event<T>> {
    loop {
        let ready = wait()?;
        if ready.input {
            return read_input().map(Event::Input);
        }
        if ready.resize {
            return Ok(Event::Resize);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Mutex;
    use std::sync::atomic::AtomicU32;

    /// The handler and its pipe are process-wide, so tests that install one
    /// take turns.
    static SERIAL: Mutex<()> = Mutex::new(());

    fn exclusive() -> std::sync::MutexGuard<'static, ()> {
        SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn current_action() -> libc::sigaction {
        // SAFETY: querying only; `action` is filled by the call.
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            libc::sigaction(libc::SIGWINCH, std::ptr::null(), &mut action);
            action
        }
    }

    fn set_action(handler: usize) {
        // SAFETY: a zeroed action with only the handler set is valid.
        unsafe {
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = handler;
            libc::sigemptyset(&mut action.sa_mask);
            libc::sigaction(libc::SIGWINCH, &action, std::ptr::null_mut());
        }
    }

    static HOST_CALLS: AtomicU32 = AtomicU32::new(0);

    extern "C" fn host_handler(_: libc::c_int) {
        HOST_CALLS.fetch_add(1, Ordering::SeqCst);
    }

    fn raise() {
        // SAFETY: delivers to this thread, and the handler returns before
        // `raise` does.
        unsafe {
            libc::raise(libc::SIGWINCH);
        }
    }

    fn ready(input: bool, resize: bool) -> io::Result<Ready> {
        Ok(Ready { input, resize })
    }

    #[test]
    fn a_resize_alone_is_a_resize_event() {
        let mut waits = vec![ready(false, true)].into_iter();
        let event = next_event(|| waits.next().unwrap(), || Ok('x')).unwrap();
        assert_eq!(event, Event::Resize);
    }

    #[test]
    fn input_is_read_when_it_is_ready() {
        let mut waits = vec![ready(true, false)].into_iter();
        let event = next_event(|| waits.next().unwrap(), || Ok('x')).unwrap();
        assert_eq!(event, Event::Input('x'));
    }

    #[test]
    fn input_wins_over_a_resize_that_arrived_with_it() {
        let mut waits = vec![ready(true, true)].into_iter();
        let event = next_event(|| waits.next().unwrap(), || Ok('x')).unwrap();
        assert_eq!(event, Event::Input('x'));
    }

    #[test]
    fn a_wait_that_found_nothing_ready_waits_again() {
        let mut waits =
            vec![ready(false, false), ready(false, false), ready(false, true)].into_iter();
        let mut asked = 0;
        let event = next_event(
            || {
                asked += 1;
                waits.next().unwrap()
            },
            || Ok('x'),
        )
        .unwrap();
        assert_eq!(event, Event::Resize);
        assert_eq!(asked, 3);
    }

    #[test]
    fn a_failing_wait_or_read_is_reported() {
        let failed = next_event(
            || Err(io::Error::from_raw_os_error(libc::EBADF)),
            || Ok('x'),
        );
        assert!(failed.is_err());
        let failed = next_event(
            || ready(true, false),
            || -> io::Result<char> { Err(io::Error::from_raw_os_error(libc::EIO)) },
        );
        assert!(failed.is_err());
    }

    #[test]
    fn input_is_not_read_while_only_a_resize_is_ready() {
        let never = || -> io::Result<char> { panic!("read with nothing to read") };
        let event = next_event(|| ready(false, true), never).unwrap();
        assert_eq!(event, Event::Resize);
    }

    #[test]
    fn a_signalled_resize_wakes_the_wait_and_is_drained() {
        let _turn = exclusive();
        let watch = ResizeWatch::install().expect("nothing else is installed");
        let (input, _keep_open) = pipe().unwrap();

        assert_eq!(wait_now(&input, &watch), Ready::default(), "quiet at first");
        raise();
        raise();
        assert_eq!(
            wait_now(&input, &watch),
            Ready {
                input: false,
                resize: true
            }
        );
        assert_eq!(
            wait_now(&input, &watch),
            Ready::default(),
            "two signals, one drained wake-up"
        );
    }

    /// What is ready right now, without waiting for more.
    fn wait_now(input: &OwnedFd, watch: &ResizeWatch) -> Ready {
        select_ready(input.as_raw_fd(), watch, Some(Duration::ZERO)).unwrap()
    }

    #[test]
    fn input_and_a_resize_are_reported_together() {
        let _turn = exclusive();
        let watch = ResizeWatch::install().unwrap();
        let (input, feed) = pipe().unwrap();
        static BYTE: u8 = 0;
        // SAFETY: writing one byte from a static to an open descriptor.
        unsafe {
            libc::write(
                feed.as_raw_fd(),
                (&raw const BYTE).cast::<libc::c_void>(),
                1,
            );
        }
        raise();
        assert_eq!(
            wait_now(&input, &watch),
            Ready {
                input: true,
                resize: true
            }
        );
    }

    #[test]
    fn dropping_the_watch_restores_the_action_that_was_there() {
        let _turn = exclusive();
        HOST_CALLS.store(0, Ordering::SeqCst);
        set_action(host_handler as *const () as usize);
        let before = current_action().sa_sigaction;

        let watch = ResizeWatch::install().unwrap();
        assert_ne!(current_action().sa_sigaction, before, "ours is in place");
        drop(watch);

        assert_eq!(current_action().sa_sigaction, before, "and then gone");
        raise();
        assert_eq!(HOST_CALLS.load(Ordering::SeqCst), 1, "host handler is back");

        set_action(libc::SIG_DFL);
    }

    #[test]
    fn the_hosts_handler_still_runs_while_the_watch_is_installed() {
        let _turn = exclusive();
        HOST_CALLS.store(0, Ordering::SeqCst);
        set_action(host_handler as *const () as usize);

        let watch = ResizeWatch::install().unwrap();
        raise();
        assert_eq!(HOST_CALLS.load(Ordering::SeqCst), 1);
        drop(watch);

        set_action(libc::SIG_DFL);
    }

    #[test]
    fn a_default_action_is_restored_too() {
        let _turn = exclusive();
        set_action(libc::SIG_DFL);
        drop(ResizeWatch::install().unwrap());
        assert_eq!(current_action().sa_sigaction, libc::SIG_DFL);
        // And with the handler gone, a resize signal is ignored by default
        // rather than reaching a stale pipe.
        raise();
    }

    #[test]
    fn a_panic_restores_the_action() {
        let _turn = exclusive();
        set_action(host_handler as *const () as usize);
        let before = current_action().sa_sigaction;

        let result = std::panic::catch_unwind(|| {
            let _watch = ResizeWatch::install().unwrap();
            panic!("the menu blew up");
        });

        assert!(result.is_err());
        assert_eq!(current_action().sa_sigaction, before);
        set_action(libc::SIG_DFL);
    }

    #[test]
    fn only_one_watch_exists_at_a_time() {
        let _turn = exclusive();
        let first = ResizeWatch::install().unwrap();
        assert!(ResizeWatch::install().is_none());
        drop(first);
        assert!(ResizeWatch::install().is_some(), "free again once dropped");
    }

    #[test]
    fn a_descriptor_past_the_select_limit_is_not_selectable() {
        assert!(selectable(0));
        assert!(selectable(i32::try_from(libc::FD_SETSIZE).unwrap() - 1));
        assert!(!selectable(i32::try_from(libc::FD_SETSIZE).unwrap()));
        assert!(!selectable(-1));
    }
}
