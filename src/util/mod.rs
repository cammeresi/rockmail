use std::io;
use std::process::{Child, ExitCode, ExitStatus};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use nix::sys::signal::{Signal, kill};
use nix::sys::stat::{self, Mode};
use nix::unistd::Pid;

use crate::variables::DEF_UMASK;

mod error;
pub mod signals;

#[cfg(test)]
mod tests;

pub use error::*;

/// Light is green; trap is clean.
pub const EX_OK: u8 = 0;
/// Bad usage (command-line error).
pub const EX_USAGE: u8 = 64;
/// Temporary failure (retry later).
pub const EX_TEMPFAIL: u8 = 75;
/// Service unavailable.
pub const EX_UNAVAILABLE: u8 = 69;
/// OS error.
pub const EX_OSERR: u8 = 71;
/// Cannot create output file.
pub const EX_CANTCREAT: u8 = 73;
/// Input file not found.
pub const EX_NOINPUT: u8 = 66;

/// Set the default umask to 077, matching procmail's INIT_UMASK.
pub fn init_umask() {
    stat::umask(Mode::from_bits_truncate(DEF_UMASK));
}

/// Convert a `u8` exit code to an `ExitCode`.
pub fn exit(code: u8) -> ExitCode {
    ExitCode::from(code)
}

/// SIGTERM, then SIGKILL after 1s. Bounded wait after SIGKILL avoids
/// hanging forever on processes in uninterruptible sleep (D state).
fn terminate(child: &mut Child, cmd: &str) -> io::Result<ExitStatus> {
    let pid = Pid::from_raw(child.id() as i32);
    if kill(pid, Signal::SIGTERM).is_ok() {
        eprintln!("Timeout, terminating \"{}\"", cmd);
    } else {
        eprintln!("Timeout, was waiting for \"{}\"", cmd);
    }
    thread::sleep(Duration::from_secs(1));
    if let Some(s) = child.try_wait()? {
        return Ok(s);
    }
    let _ = kill(pid, Signal::SIGKILL);
    for _ in 0..50 {
        thread::sleep(Duration::from_millis(100));
        if let Some(s) = child.try_wait()? {
            return Ok(s);
        }
    }
    Err(io::Error::new(io::ErrorKind::TimedOut, "child unkillable"))
}

/// Polls `try_wait` with exponential backoff. On timeout, sends SIGTERM
/// then SIGKILL. Returns the exit status (which may reflect the signal).
pub fn wait_timeout(
    child: &mut Child, timeout: Duration, cmd: &str,
) -> io::Result<ExitStatus> {
    let start = Instant::now();
    let cap = Duration::from_millis(100);
    let mut poll = Duration::from_millis(1);

    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if start.elapsed() >= timeout {
            return terminate(child, cmd);
        }
        thread::sleep(poll);
        poll = (poll * 2).min(cap);
    }
}

/// Current time as seconds since the Unix epoch.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before 1970")
        .as_secs()
}
