use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum LockError {
    #[error("lockfile already exists")]
    Exists,

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("filename too long")]
    TooLong,

    #[error("permission denied or missing directory")]
    Unavailable,
}
