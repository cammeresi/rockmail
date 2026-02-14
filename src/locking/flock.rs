use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use nix::fcntl::{Flock, FlockArg};

use crate::util::{LockError, signals};

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

        loop {
            match Self::acquire_temp(path) {
                Ok(lock) => return Ok(lock),
                Err(LockError::Exists) => {}
                Err(e) => return Err(e),
            }

            if !forced {
                if let Ok(meta) = fs::metadata(path)
                    && let Ok(mtime) = meta.modified()
                    && let Ok(age) = SystemTime::now().duration_since(mtime)
                    && age > timeout
                {
                    let _ = fs::remove_file(path);
                    forced = true;
                    continue;
                }
            }

            if start.elapsed() >= timeout || signals::should_exit() {
                return Err(LockError::Exists);
            }

            thread::sleep(sleep);
        }
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
