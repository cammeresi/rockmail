use std::io;

use nix::libc;

use crate::util::{LockError, warning};

#[cfg(feature = "nfs")]
mod dotlock;
mod flock;
#[cfg(test)]
mod tests;

/// Lockfiles should not be larger than this (procmail's MAX_locksize).
pub const MAX_LOCK_SIZE: u64 = 16;

#[cfg(feature = "nfs")]
pub use dotlock::{create_lock, lock_mtime, remove_lock};
pub use flock::FileLock;

/// Map an I/O error to a `LockError`, turning ENAMETOOLONG into `TooLong`.
fn map_io_err(e: io::Error) -> LockError {
    if e.raw_os_error() == Some(libc::ENAMETOOLONG) {
        LockError::TooLong
    } else {
        LockError::Io(e)
    }
}

/// Truncate a lock path by one byte for ENAMETOOLONG recovery.
///
/// Returns false if the path is too short or the second-to-last byte
/// is a directory separator (matching procmail's truncation rule).
pub fn truncate_lock_path(p: &mut String) -> bool {
    let b = p.as_bytes();
    if b.len() > 1 && b[b.len() - 2] != b'/' {
        warning!("Truncating \"{p}\" and retrying lock");
        p.pop();
        true
    } else {
        false
    }
}
