use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

use nix::fcntl::{Flock, FlockArg};

use crate::util::LockError;

pub struct FileLock {
    #[allow(dead_code)]
    lock: Flock<File>,
    cleanup: Option<PathBuf>,
}

impl FileLock {
    pub fn acquire(path: &Path) -> Result<Self, LockError> {
        Self::open(path, None)
    }

    /// Acquire a lock and remove the file on drop.
    pub fn acquire_temp(path: &Path) -> Result<Self, LockError> {
        Self::open(path, Some(path.to_path_buf()))
    }

    fn open(path: &Path, cleanup: Option<PathBuf>) -> Result<Self, LockError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => LockError::Unavailable,
                _ => LockError::Io(e),
            })?;
        let lock = Flock::lock(file, FlockArg::LockExclusiveNonblock)
            .map_err(|_| LockError::Exists)?;
        Ok(Self { lock, cleanup })
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        if let Some(p) = self.cleanup.take() {
            let _ = fs::remove_file(p);
        }
    }
}
