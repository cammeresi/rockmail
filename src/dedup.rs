//! Duplicate detection via a circular null-terminated string cache.

use std::fs::OpenOptions;
use std::io::{self, Read, Seek, SeekFrom, Write};

use nix::fcntl::{Flock, FlockArg};

/// Check if `key` exists in the cache file at `path`.
///
/// Returns `true` if `key` was already present (duplicate).  If new,
/// the key is appended to the cache.  The cache is a circular buffer
/// of null-terminated strings; when it would exceed `maxlen` bytes the
/// write wraps to the start.
pub fn check_cache(key: &str, path: &str, maxlen: usize) -> io::Result<bool> {
    let key = key.trim();
    if key.is_empty() {
        return Ok(false);
    }

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;

    let mut file = Flock::lock(file, FlockArg::LockExclusive)
        .map_err(|(_, e)| io::Error::other(e))?;

    let mut buf = vec![0u8; maxlen];
    let n = file.read(&mut buf)?;
    buf.truncate(n);

    let mut dup = false;
    let mut insert: Option<usize> = None;

    let mut pos = 0;
    while pos < buf.len() {
        let start = pos;
        while pos < buf.len() && buf[pos] != 0 {
            pos += 1;
        }
        let entry = &buf[start..pos];

        if entry.is_empty() {
            if insert.is_none() {
                insert = Some(start);
            }
        } else if entry == key.as_bytes() {
            dup = true;
            break;
        }

        if pos < buf.len() {
            pos += 1;
        }
    }

    if !dup {
        let offset = if let Some(off) = insert {
            off
        } else if n >= maxlen {
            0
        } else {
            n
        };

        let needed = key.len() + 2;
        let offset = if offset + needed > maxlen { 0 } else { offset };

        file.seek(SeekFrom::Start(offset as u64))?;
        file.write_all(key.as_bytes())?;
        file.write_all(b"\0\0")?;
        file.set_len((offset + needed) as u64)?;
    }

    Ok(dup)
}
