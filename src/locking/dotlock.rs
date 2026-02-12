use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use nix::unistd::getpid;

use crate::util::{LockError, now_secs};

#[cfg(test)]
mod tests;

const LOCK_PERM: u32 = 0o444;

fn compute_safe_hostname() -> String {
    let h = nix::unistd::gethostname().expect("gethostname");
    let h = h.into_string().expect("hostname not utf-8");
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

fn safe_hostname() -> &'static str {
    static CACHE: OnceLock<String> = OnceLock::new();
    CACHE.get_or_init(compute_safe_hostname)
}

fn random_u64() -> u64 {
    let mut buf = [0u8; 8];
    File::open("/dev/urandom")
        .expect("failed to open /dev/urandom")
        .read_exact(&mut buf)
        .expect("failed to read /dev/urandom");
    u64::from_ne_bytes(buf)
}

fn unique_name(dir: &Path) -> PathBuf {
    let host = safe_hostname();
    let pid = getpid();
    let t = now_secs();
    let r = random_u64();
    let name = format!("_{}.{}.{:x}.{}", pid, t, r, host);
    dir.join(&name)
}

struct TmpGuard<'a> {
    path: &'a Path,
    disarm: bool,
}

impl<'a> TmpGuard<'a> {
    fn new(path: &'a Path) -> Self {
        Self {
            path,
            disarm: false,
        }
    }

    fn disarm(&mut self) {
        self.disarm = true;
    }
}

impl Drop for TmpGuard<'_> {
    fn drop(&mut self) {
        if !self.disarm {
            let _ = fs::remove_file(self.path);
        }
    }
}

/// Get the modification time of a lockfile.
pub fn lock_mtime(target: &Path) -> Option<u64> {
    fs::metadata(target)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

/// Remove a lockfile.
pub fn remove_lock(target: &Path) -> Result<(), LockError> {
    fs::remove_file(target).map_err(LockError::Io)
}

/// Create a lockfile using NFS-safe technique: create unique file, then rename.
pub fn create_lock(target: &Path) -> Result<(), LockError> {
    let dir = target.parent().unwrap_or(Path::new("."));
    let tmp = unique_name(dir);

    // Create with write permission, write content, then set final permissions
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o644)
        .open(&tmp)
        .map_err(|e| match e.kind() {
            io::ErrorKind::AlreadyExists => LockError::Exists,
            io::ErrorKind::NotFound => LockError::Unavailable,
            _ => LockError::Io(e),
        })?;

    let mut guard = TmpGuard::new(&tmp);

    // Write "0" to the file (pid 0 works across networks)
    file.write_all(b"0").map_err(LockError::Io)?;
    drop(file);

    // Set final read-only permissions
    fs::set_permissions(&tmp, fs::Permissions::from_mode(LOCK_PERM))
        .map_err(LockError::Io)?;

    // Hard link to target - this is atomic on POSIX
    match fs::hard_link(&tmp, target) {
        Ok(()) => {
            guard.disarm();
            let _ = fs::remove_file(&tmp);
            Ok(())
        }
        Err(e) => {
            // Check if link succeeded despite error (NFS issue)
            if let Ok(meta) = fs::metadata(&tmp)
                && meta.nlink() > 1
            {
                guard.disarm();
                let _ = fs::remove_file(&tmp);
                return Ok(());
            }
            if e.kind() == io::ErrorKind::AlreadyExists {
                Err(LockError::Exists)
            } else {
                Err(LockError::Io(e))
            }
        }
    }
}
