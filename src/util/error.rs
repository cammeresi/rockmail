use std::io;
use thiserror::Error;

/// Error from a locking operation.
#[derive(Error, Debug)]
pub enum LockError {
    /// Lockfile already exists (non-blocking acquire failed).
    #[error("lockfile already exists")]
    Exists,
    /// Underlying I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    /// Lock path exceeds filesystem name limit.
    #[error("filename too long")]
    TooLong,
    /// Permission denied or parent directory missing.
    #[error("permission denied or missing directory")]
    Unavailable,
}
