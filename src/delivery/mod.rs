#[cfg(test)]
mod tests;

mod maildir;
mod mbox;
mod mh;
mod pipe;

use std::io;
use std::path::Path;

use crate::mail::Message;

pub use maildir::{deliver as maildir, deliver_dir as dir};
pub use mbox::deliver as mbox;
pub use mh::deliver as mh;
pub use pipe::deliver as pipe;

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
            // foo/. → MH
            let stripped = &path[..len - 2];
            let stripped = stripped.trim_end_matches('/');
            (FolderType::Mh, stripped)
        } else if len >= 2 && bytes[len - 1] == b'/' && bytes[len - 2] == b'/' {
            // foo// → Dir
            let stripped = path.trim_end_matches('/');
            (FolderType::Dir, stripped)
        } else if len >= 1 && bytes[len - 1] == b'/' {
            // foo/ → Maildir
            let stripped = path.trim_end_matches('/');
            (FolderType::Maildir, stripped)
        } else {
            // foo → File (or Dir based on fs check, handled elsewhere)
            (FolderType::File, path)
        }
    }

    pub fn deliver(
        self, path: &Path, msg: &Message, sender: &str, opts: DeliveryOpts,
    ) -> Result<DeliveryResult, DeliveryError> {
        match self {
            FolderType::File => mbox::deliver(path, msg, sender, opts),
            FolderType::Maildir => maildir::deliver(path, msg, opts),
            FolderType::Mh => mh::deliver(path, msg, opts),
            FolderType::Dir => maildir::deliver_dir(path, msg, opts),
        }
    }
}

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
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("failed to create unique filename")]
    UniqueFile,

    #[error("failed to link to final location")]
    Link,

    #[error("failed to acquire lock: {0}")]
    Lock(#[from] crate::util::LockError),

    #[error("pipe command failed with exit code {0}")]
    PipeExit(i32),

    #[error("pipe command killed by signal {0}")]
    PipeSignal(i32),
}
