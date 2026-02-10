//! Mail delivery to folders and pipes.

use std::io;
use std::path::Path;

use crate::mail::Message;

mod maildir;
mod mbox;
mod mh;
mod pipe;
#[cfg(test)]
mod tests;

pub use maildir::Namer;
pub use pipe::deliver as pipe;

/// Options for delivery operations.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeliveryOpts {
    /// Raw mode: don't ensure trailing newline.
    pub raw: bool,
}

/// Result of a delivery operation.
#[derive(Debug)]
pub struct DeliveryResult {
    /// Bytes written.
    pub bytes: usize,
    /// Path where message was delivered (for logging).
    pub path: String,
}

/// Common delivery error type.
#[derive(Debug, thiserror::Error)]
pub enum DeliveryError {
    /// I/O error during delivery.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Failed to create a unique filename (maildir/MH).
    #[error("failed to create unique filename")]
    UniqueFile,

    /// Failed to link temp file to final location.
    #[error("failed to link to final location")]
    Link,

    /// Failed to acquire lockfile.
    #[error("failed to acquire lock: {0}")]
    Lock(#[from] crate::util::LockError),

    /// Pipe command exited with non-zero status.
    #[error("pipe command failed with exit code {0}")]
    PipeExit(i32),

    /// Pipe command killed by signal.
    #[error("pipe command killed by signal {0}")]
    PipeSignal(i32),
}

/// Folder type as determined by path suffix or filesystem state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderType {
    /// Regular file (mbox format).
    File,
    /// Maildir (path ends with /).
    Maildir,
    /// MH folder (path ends with /.).
    Mh,
    /// Directory with msgprefix (path ends with //).
    Dir,
}

impl FolderType {
    /// Whether this folder type needs a recipe-level lockfile.
    /// Maildir and MH use atomic delivery and need no locking.
    pub fn suffix(self) -> &'static str {
        match self {
            Self::File => "",
            Self::Maildir => "/",
            Self::Mh => "/.",
            Self::Dir => "//",
        }
    }

    /// Whether this folder type needs a recipe-level lockfile.
    pub fn needs_lock(self) -> bool {
        matches!(self, Self::File | Self::Dir)
    }

    /// Parse folder type from path suffix, stripping type specifier.
    ///
    /// - `foo/` → Maildir
    /// - `foo/.` → MH
    /// - `foo//` → Dir (directory with msgprefix)
    /// - `foo` → File (or Dir if path is an existing directory)
    ///
    /// Returns the type and the path with specifier stripped.
    pub fn parse(path: &str) -> (FolderType, &str) {
        let bytes = path.as_bytes();
        let len = bytes.len();

        if len >= 2 && bytes[len - 1] == b'.' && bytes[len - 2] == b'/' {
            let stripped = &path[..len - 2];
            let stripped = stripped.trim_end_matches('/');
            (FolderType::Mh, stripped)
        } else if len >= 2 && bytes[len - 1] == b'/' && bytes[len - 2] == b'/' {
            let stripped = path.trim_end_matches('/');
            (FolderType::Dir, stripped)
        } else if len >= 1 && bytes[len - 1] == b'/' {
            let stripped = path.trim_end_matches('/');
            (FolderType::Maildir, stripped)
        } else {
            (FolderType::File, path)
        }
    }

    /// Deliver a message to this folder type.
    pub fn deliver(
        self, path: &Path, msg: &Message, sender: &str, opts: DeliveryOpts,
        namer: &mut Namer,
    ) -> Result<DeliveryResult, DeliveryError> {
        match self {
            FolderType::File => mbox::deliver(path, msg, sender, opts),
            FolderType::Maildir => maildir::deliver(namer, path, msg, opts),
            FolderType::Mh => mh::deliver(path, msg, opts),
            FolderType::Dir => maildir::deliver_dir(path, msg, opts),
        }
    }
}
