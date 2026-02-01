#[cfg(test)]
mod tests;

use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use super::{DeliveryError, DeliveryResult};
use crate::mail::Message;

/// State for MH message numbering (caches next number across deliveries).
#[derive(Debug, Default)]
pub struct Namer {
    next: u64,
}

impl Namer {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Deliver a message to an MH folder.
///
/// MH folders use numbered files (1, 2, 3, ...).
/// Creates the folder directory if needed.
pub fn deliver(
    path: &Path, msg: &Message,
) -> Result<DeliveryResult, DeliveryError> {
    deliver_with(&mut Namer::new(), path, msg)
}

/// Deliver with explicit namer (for batch deliveries).
pub fn deliver_with(
    namer: &mut Namer, path: &Path, msg: &Message,
) -> Result<DeliveryResult, DeliveryError> {
    fs::create_dir_all(path)?;

    let (file, dest) = create_unique(namer, path)?;
    let bytes = write_msg(file, msg)?;

    Ok(DeliveryResult {
        bytes,
        path: dest.display().to_string(),
    })
}

fn create_unique(
    namer: &mut Namer, path: &Path,
) -> Result<(File, PathBuf), DeliveryError> {
    if namer.next == 0 {
        namer.next = scan_max(path)? + 1;
    }
    loop {
        let dest = path.join(namer.next.to_string());
        namer.next += 1;
        match OpenOptions::new().write(true).create_new(true).open(&dest) {
            Ok(f) => return Ok((f, dest)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
}

fn scan_max(path: &Path) -> Result<u64, DeliveryError> {
    let mut max = 0u64;
    let entries = fs::read_dir(path)?;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let Some(s) = name.to_str() else { continue };
        let Ok(n) = s.parse::<u64>() else { continue };
        max = max.max(n);
    }
    Ok(max)
}

fn write_msg(file: File, msg: &Message) -> Result<usize, DeliveryError> {
    let data = if msg.from_line().is_some() {
        crate::mail::skip_from_lines(msg.as_bytes())
    } else {
        msg.as_bytes()
    };

    let mut w = BufWriter::new(file);

    w.write_all(data)?;
    let bytes = data.len();

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
