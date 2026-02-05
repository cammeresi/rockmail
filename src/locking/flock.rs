use crate::util::LockError;
use nix::fcntl::{Flock, FlockArg};
use std::fs::{File, OpenOptions};
use std::path::Path;

pub struct FileLock {
    #[allow(dead_code)]
    lock: Flock<File>,
}

impl FileLock {
    pub fn acquire(path: &Path) -> Result<Self, LockError> {
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
        Ok(Self { lock })
    }
}
