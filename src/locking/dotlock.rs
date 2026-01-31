use crate::util::LockError;
use nix::unistd::{Pid, getpid};
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const LOCK_PERM: u32 = 0o444;
const RETRY_UNIQUE: u32 = 8;

fn hostname() -> String {
    nix::unistd::gethostname()
        .ok()
        .and_then(|h: OsString| h.into_string().ok())
        .unwrap_or_default()
}

fn safe_hostname() -> String {
    let h = hostname();
    let mut out = String::with_capacity(h.len());
    for c in h.chars() {
        match c {
            '/' | ':' | '\\' => {
                out.push('\\');
                let b = c as u8;
                out.push(char::from(b'0' + (b >> 6)));
                out.push(char::from(b'0' + ((b >> 3) & 7)));
                out.push(char::from(b'0' + (b & 7)));
            }
            _ => out.push(c),
        }
    }
    out
}

fn unique_name(dir: &Path, pid: Pid) -> Option<PathBuf> {
    let chars = ['.', ',', '+', '%'];
    let host = safe_hostname();
    let mut serial = 0usize;
    let mut t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    for _ in 0..RETRY_UNIQUE {
        let suffix = if serial < chars.len() {
            chars[serial]
        } else {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            if now == t {
                sleep(Duration::from_secs(1));
                t = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
            } else {
                t = now;
            }
            serial = 0;
            chars[0]
        };

        let name = format!("_{}{}{}.{}", pid, suffix, t, host);
        let path = dir.join(&name);

        if !path.exists() {
            return Some(path);
        }
        serial += 1;
    }
    None
}

/// Create a lockfile using NFS-safe technique: create unique file, then rename.
pub fn create_lock(target: &Path) -> Result<(), LockError> {
    use std::io::Write;

    let dir = target.parent().unwrap_or(Path::new("."));
    let pid = getpid();

    let tmp = unique_name(dir, pid).ok_or(LockError::Unavailable)?;

    // Create with write permission, write content, then set final permissions
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(&tmp)
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::AlreadyExists => LockError::Exists,
            std::io::ErrorKind::NotFound => LockError::Unavailable,
            _ => LockError::Io(e),
        })?;

    // Write "0" to the file (pid 0 works across networks)
    file.write_all(b"0").map_err(LockError::Io)?;
    drop(file);

    // Set final read-only permissions
    fs::set_permissions(&tmp, fs::Permissions::from_mode(LOCK_PERM))
        .map_err(LockError::Io)?;

    // Hard link to target - this is atomic on POSIX
    match fs::hard_link(&tmp, target) {
        Ok(()) => {
            let _ = fs::remove_file(&tmp);
            Ok(())
        }
        Err(e) => {
            // Check if link succeeded despite error (NFS issue)
            if let Ok(meta) = fs::metadata(&tmp) {
                if meta.nlink() > 1 {
                    let _ = fs::remove_file(&tmp);
                    return Ok(());
                }
            }
            let _ = fs::remove_file(&tmp);
            if e.kind() == std::io::ErrorKind::AlreadyExists {
                Err(LockError::Exists)
            } else {
                Err(LockError::Io(e))
            }
        }
    }
}

/// Remove a lockfile.
pub fn remove_lock(target: &Path) -> Result<(), LockError> {
    fs::remove_file(target).map_err(LockError::Io)
}

/// Get the modification time of a lockfile.
pub fn lock_mtime(target: &Path) -> Option<u64> {
    fs::metadata(target)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_and_remove() {
        let dir = tempdir().unwrap();
        let lock = dir.path().join("test.lock");

        assert!(create_lock(&lock).is_ok());
        assert!(lock.exists());

        // Second create should fail
        assert!(matches!(create_lock(&lock), Err(LockError::Exists)));

        assert!(remove_lock(&lock).is_ok());
        assert!(!lock.exists());
    }

    #[test]
    fn test_lock_mtime() {
        let dir = tempdir().unwrap();
        let lock = dir.path().join("test.lock");

        create_lock(&lock).unwrap();
        let mtime = lock_mtime(&lock);
        assert!(mtime.is_some());

        remove_lock(&lock).unwrap();
    }
}
