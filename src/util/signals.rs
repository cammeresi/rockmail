//! Signal handling utilities.

use std::sync::atomic::{AtomicBool, Ordering};

use nix::sys::signal::{
    SigHandler, SigSet, SigmaskHow, Signal, signal, sigprocmask,
};

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
        for sig in [
            Signal::SIGPIPE,
            Signal::SIGXCPU,
            Signal::SIGXFSZ,
        ] {
            signal(sig, SigHandler::SigIgn)
                .expect("failed to ignore signal");
        }
        signal(Signal::SIGCHLD, SigHandler::SigDfl)
            .expect("failed to reset SIGCHLD");
    }
}

extern "C" fn handle(_: i32) {
    EXIT_FLAG.store(true, Ordering::Release);
}

/// Check if termination was requested.
pub fn should_exit() -> bool {
    EXIT_FLAG.load(Ordering::Acquire)
}

fn term_set() -> SigSet {
    let mut set = SigSet::empty();
    for sig in [
        Signal::SIGHUP,
        Signal::SIGINT,
        Signal::SIGQUIT,
        Signal::SIGTERM,
    ] {
        set.add(sig);
    }
    set
}

/// Block termination signals for a critical section.
pub fn block_signals() {
    let _ = sigprocmask(SigmaskHow::SIG_BLOCK, Some(&term_set()), None);
}

/// Unblock termination signals.  If one was pending, the handler
/// will fire and set the exit flag.
pub fn unblock_signals() {
    let _ = sigprocmask(SigmaskHow::SIG_UNBLOCK, Some(&term_set()), None);
}
