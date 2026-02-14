use std::fs::{self, File, OpenOptions};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use super::{DeliveryError, DeliveryOpts, DeliveryResult};
use crate::mail::Message;

#[cfg(test)]
mod tests;

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

fn write_msg(
    file: File, msg: &Message, opts: DeliveryOpts,
) -> Result<usize, DeliveryError> {
    let data = msg.as_bytes();

    let mut w = BufWriter::new(file);

    w.write_all(data)?;
    let bytes = data.len();

    let mut extra = 0;
    if !opts.raw && !data.ends_with(b"\n\n") {
        // weirdly, procmail checks for two then adds only one
        w.write_all(b"\n")?;
        extra += 1;
    }

    w.flush()?;
    let file = w.into_inner().map_err(|e| e.into_error())?;
    file.sync_all()?;

    Ok(bytes + extra)
}

/// Deliver a message to an MH folder.
///
/// MH folders use numbered files (1, 2, 3, ...).
/// Creates the folder directory if needed.
pub fn deliver(
    path: &Path, msg: &Message, opts: DeliveryOpts,
) -> Result<DeliveryResult, DeliveryError> {
    deliver_with(&mut Namer::new(), path, msg, opts)
}

/// Deliver with explicit namer (for batch deliveries).
pub fn deliver_with(
    namer: &mut Namer, path: &Path, msg: &Message, opts: DeliveryOpts,
) -> Result<DeliveryResult, DeliveryError> {
    fs::create_dir_all(path)?;

    let (file, dest) = create_unique(namer, path)?;
    let bytes = write_msg(file, msg, opts)?;

    Ok(DeliveryResult {
        bytes,
        path: dest.display().to_string(),
    })
}

/// Hard-link `src` into the next available MH slot.
pub(super) fn link_unique(
    path: &Path, src: &Path,
) -> Result<String, DeliveryError> {
    let mut n = scan_max(path)? + 1;
    loop {
        let dest = path.join(n.to_string());
        match fs::hard_link(src, &dest) {
            Ok(()) => return Ok(dest.display().to_string()),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                n += 1;
                continue;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

#[cfg(test)]
pub fn deliver_test(
    path: &Path, msg: &Message,
) -> Result<DeliveryResult, DeliveryError> {
    deliver(path, msg, DeliveryOpts::default())
}
