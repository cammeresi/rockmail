use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LockError {
    #[error("lockfile already exists")]
    Exists,

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("out of memory")]
    OutOfMemory,

    #[error("signal received")]
    Signal,

    #[error("retries exhausted")]
    RetriesExhausted,

    #[error("filename too long")]
    NameTooLong,

    #[error("permission denied or missing directory")]
    Unavailable,
}
