#[cfg(test)]
mod tests;

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{DeliveryError, DeliveryResult};
use crate::mail::{Message, skip_from_lines};

/// State for unique filename generation (preserves serial across calls).
#[derive(Debug)]
pub struct Namer {
    last_time: u64,
    serial: u32,
    host: String,
}

impl Default for Namer {
    fn default() -> Self {
        Self {
            last_time: 0,
            serial: 0,
            host: hostname(),
        }
    }
}

impl Namer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate unique filename in Maildir format: time.pid_serial.hostname
    pub fn next(&mut self) -> Result<String, DeliveryError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| DeliveryError::UniqueFile)?;

        let t = now.as_secs();
        let pid = process::id();

        if t != self.last_time {
            self.last_time = t;
            self.serial = 0;
        }
        let serial = self.serial;
        self.serial = self
            .serial
            .checked_add(1)
            .ok_or(DeliveryError::UniqueFile)?;

        Ok(format!("{}.{}_{}.{}", t, pid, serial, self.host))
    }
}

/// Deliver a message to a Maildir folder.
///
/// Creates the Maildir structure (tmp, new, cur) if needed.
/// Writes to tmp/ then hard-links to new/ for atomic delivery.
pub fn deliver(
    path: &Path, msg: &Message,
) -> Result<DeliveryResult, DeliveryError> {
    deliver_with(&mut Namer::new(), path, msg)
}

/// Deliver a message directly to a directory (procmail // mode).
///
/// Unlike Maildir, writes directly without tmp/new/cur structure.
pub fn deliver_dir(
    path: &Path, msg: &Message,
) -> Result<DeliveryResult, DeliveryError> {
    fs::create_dir_all(path)?;

    let name = Namer::new().next()?;
    let dest = path.join(format!("msg.{}", name));

    let bytes = write_msg(&dest, msg)?;

    Ok(DeliveryResult {
        bytes,
        path: dest.display().to_string(),
    })
}

/// Deliver with explicit namer (for preserving serial across deliveries).
pub fn deliver_with(
    namer: &mut Namer, path: &Path, msg: &Message,
) -> Result<DeliveryResult, DeliveryError> {
    ensure_dirs(path)?;

    let name = namer.next()?;
    let tmp = path.join("tmp").join(&name);
    let new = path.join("new").join(&name);

    let bytes = write_msg(&tmp, msg)?;

    if fs::hard_link(&tmp, &new).is_err() {
        fs::rename(&tmp, &new)?;
    } else {
        let _ = fs::remove_file(&tmp);
    }

    Ok(DeliveryResult {
        bytes,
        path: new.display().to_string(),
    })
}

fn ensure_dirs(path: &Path) -> Result<(), DeliveryError> {
    for sub in ["tmp", "new", "cur"] {
        fs::create_dir_all(path.join(sub))?;
    }
    Ok(())
}

fn hostname() -> String {
    nix::unistd::gethostname()
        .ok()
        .and_then(|s| s.into_string().ok())
        .unwrap_or_else(|| "localhost".to_string())
}

fn write_msg(path: &Path, msg: &Message) -> Result<usize, DeliveryError> {
    let file = File::create(path)?;
    let mut w = BufWriter::new(file);

    // Maildir doesn't use From_ lines
    let data = msg.as_bytes();
    let data = if msg.from_line().is_some() {
        skip_from_lines(data)
    } else {
        data
    };

    w.write_all(data)?;
    let bytes = data.len();

    // Ensure trailing newline
    let extra = if !data.ends_with(b"\n") {
        w.write_all(b"\n")?;
        1
    } else {
        0
    };

    w.flush()?;
    let file = w.into_inner().map_err(|e| e.into_error())?;
    file.sync_all()?;

    Ok(bytes + extra)
}
