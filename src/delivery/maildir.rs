#[cfg(test)]
mod tests;

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{DeliveryError, DeliveryResult};
use crate::mail::{Message, skip_from_lines};

/// Deliver a message to a Maildir folder.
///
/// Creates the Maildir structure (tmp, new, cur) if needed.
/// Writes to tmp/ then hard-links to new/ for atomic delivery.
pub fn deliver(
    path: &Path, msg: &Message,
) -> Result<DeliveryResult, DeliveryError> {
    ensure_dirs(path)?;

    let name = unique_name()?;
    let tmp = path.join("tmp").join(&name);
    let new = path.join("new").join(&name);

    // Write to tmp
    let bytes = write_msg(&tmp, msg)?;

    // Link to new (atomic)
    if fs::hard_link(&tmp, &new).is_err() {
        // Fall back to rename if hard link fails (e.g., cross-filesystem)
        fs::rename(&tmp, &new).map_err(|_| DeliveryError::Link)?;
    } else {
        // Remove tmp file after successful link
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

fn unique_name() -> Result<String, DeliveryError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| DeliveryError::UniqueFile)?;

    let pid = std::process::id();
    let host = hostname();

    // Format: time.pid.hostname
    // Add microseconds for uniqueness
    Ok(format!(
        "{}.M{}P{}.{}",
        now.as_secs(),
        now.subsec_micros(),
        pid,
        host
    ))
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
    drop(w);

    // fsync for durability
    let file = File::open(path)?;
    file.sync_all()?;

    Ok(bytes + extra)
}
