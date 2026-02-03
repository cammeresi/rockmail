//! Signal handling utilities.

use std::sync::atomic::{AtomicBool, Ordering};

use nix::sys::signal::{SigHandler, Signal, signal};

static EXIT_FLAG: AtomicBool = AtomicBool::new(false);

/// Install signal handlers for graceful termination.
///
/// Handles SIGHUP, SIGINT, SIGQUIT, SIGTERM by setting a flag.
/// Ignores SIGPIPE.
///
/// # Panics
/// Panics if signal registration fails.
pub fn setup() {
    let handler = SigHandler::Handler(handle);
    // SAFETY: Signal handlers are async-signal-safe (only set atomic flag).
    unsafe {
        for sig in [
            Signal::SIGHUP,
            Signal::SIGINT,
            Signal::SIGQUIT,
            Signal::SIGTERM,
        ] {
            signal(sig, handler).expect("failed to install signal handler");
        }
        signal(Signal::SIGPIPE, SigHandler::SigIgn)
            .expect("failed to ignore SIGPIPE");
    }
}

extern "C" fn handle(_: i32) {
    EXIT_FLAG.store(true, Ordering::Release);
}

/// Check if termination was requested.
pub fn should_exit() -> bool {
    EXIT_FLAG.load(Ordering::Acquire)
}
