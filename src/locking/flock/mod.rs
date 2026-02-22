use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use nix::fcntl::{Flock, FlockArg};

use super::{MAX_LOCK_SIZE, map_io_err, truncate_lock_path};
use crate::util::{LockError, signals};

#[cfg(test)]
mod tests;

/// An exclusive file lock backed by `flock(2)`, with optional cleanup on drop.
#[derive(Debug)]
pub struct FileLock {
    _lock: Flock<File>,
    cleanup: Option<PathBuf>,
}

impl FileLock {
    /// Acquire a blocking lock (waits until available).
    pub fn acquire_blocking(path: &Path) -> Result<Self, LockError> {
        Self::open(path, None, FlockArg::LockExclusive)
    }

    /// Acquire a lock and remove the file on drop.
    pub fn acquire_temp(path: &Path) -> Result<Self, LockError> {
        Self::open(
            path,
            Some(path.to_path_buf()),
            FlockArg::LockExclusiveNonblock,
        )
    }

    /// Acquire a temp lock with retry and stale-lock removal.
    ///
    /// Retries every `sleep` seconds up to `timeout` seconds total.
    /// If an existing lockfile is older than `timeout`, removes it
    /// and retries once (matching procmail's forced-unlock behavior).
    pub fn acquire_temp_retry(
        path: &Path, timeout: u64, sleep: u64,
    ) -> Result<Self, LockError> {
        let start = Instant::now();
        let timeout = Duration::from_secs(timeout);
        let sleep = Duration::from_secs(sleep);
        let mut forced = false;
        let mut owned = String::new();

        loop {
            let p = if owned.is_empty() {
                path
            } else {
                Path::new(&owned)
            };
            match Self::acquire_temp(p) {
                Ok(lock) => return Ok(lock),
                Err(LockError::Exists) => {}
                Err(LockError::TooLong) => {
                    if owned.is_empty() {
                        owned = path.to_string_lossy().into_owned();
                    }
                    if !truncate_lock_path(&mut owned) {
                        return Err(LockError::TooLong);
                    }
                    continue;
                }
                Err(e) => return Err(e),
            }

            if !forced
                && let Ok(meta) = fs::metadata(p)
                && !meta.is_dir()
                && meta.len() <= MAX_LOCK_SIZE
                && let Ok(mtime) = meta.modified()
                && let Ok(age) = SystemTime::now().duration_since(mtime)
                && age > timeout
            {
                let _ = fs::remove_file(p);
                forced = true;
                continue;
            }

            if start.elapsed() >= timeout || signals::should_exit() {
                return Err(LockError::Exists);
            }

            thread::sleep(sleep);
        }
    }

    fn open(
        path: &Path, cleanup: Option<PathBuf>, arg: FlockArg,
    ) -> Result<Self, LockError> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|e| match e.kind() {
                io::ErrorKind::NotFound => LockError::Unavailable,
                _ => map_io_err(e),
            })?;
        let _lock = Flock::lock(file, arg).map_err(|_| LockError::Exists)?;
        Ok(Self { _lock, cleanup })
    }
}

impl Drop for FileLock {
    fn drop(&mut self) {
        if let Some(p) = self.cleanup.take() {
            let _ = fs::remove_file(p);
        }
    }
}
