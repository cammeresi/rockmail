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

pub const EX_OK: u8 = 0;
pub const EX_USAGE: u8 = 64;
pub const EX_TEMPFAIL: u8 = 75;
pub const EX_UNAVAILABLE: u8 = 69;
pub const EX_OSERR: u8 = 71;
pub const EX_CANTCREAT: u8 = 73;
pub const EX_NOINPUT: u8 = 66;

/// Set the default umask to 077, matching procmail's INIT_UMASK.
pub fn init_umask() {
    stat::umask(Mode::from_bits_truncate(DEF_UMASK));
}

pub fn exit(code: u8) -> ExitCode {
    ExitCode::from(code)
}

/// Wait for a child process with a timeout.
///
/// Polls `try_wait` every 100ms. On timeout, sends SIGTERM, waits 1s,
/// then SIGKILL. Returns the exit status (which may reflect the signal).
pub fn wait_timeout(
    child: &mut Child, timeout: Duration,
) -> io::Result<ExitStatus> {
    let start = Instant::now();
    let poll = Duration::from_millis(100);

    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if start.elapsed() >= timeout {
            let pid = Pid::from_raw(child.id() as i32);
            let _ = kill(pid, Signal::SIGTERM);
            thread::sleep(Duration::from_secs(1));
            if child.try_wait()?.is_none() {
                let _ = kill(pid, Signal::SIGKILL);
            }
            return child.wait();
        }
        thread::sleep(poll);
    }
}

/// Spawn a watchdog thread that kills `pid` after `timeout`.
///
/// If the child exits before the timeout, the kill returns ESRCH
/// (harmlessly ignored). For use with `wait_with_output()` which
/// consumes the Child.
pub fn spawn_watchdog(pid: u32, timeout: Duration) {
    thread::spawn(move || {
        thread::sleep(timeout);
        let pid = Pid::from_raw(pid as i32);
        if kill(pid, Signal::SIGTERM).is_ok() {
            thread::sleep(Duration::from_secs(1));
            let _ = kill(pid, Signal::SIGKILL);
        }
    });
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before 1970")
        .as_secs()
}
