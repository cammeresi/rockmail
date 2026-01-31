#[cfg(test)]
mod tests;

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::Path;

use super::{DeliveryError, DeliveryResult};
use crate::mail::Message;

/// Deliver a message to an MH folder.
///
/// MH folders use numbered files (1, 2, 3, ...).
/// Creates the folder directory if needed.
pub fn deliver(
    path: &Path, msg: &Message,
) -> Result<DeliveryResult, DeliveryError> {
    fs::create_dir_all(path)?;

    let num = next_number(path)?;
    let dest = path.join(num.to_string());

    let bytes = write_msg(&dest, msg)?;

    Ok(DeliveryResult {
        bytes,
        path: dest.display().to_string(),
    })
}

fn next_number(path: &Path) -> Result<u64, DeliveryError> {
    let mut max = 0u64;

    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(s) = name.to_str() else { continue };
            let Ok(n) = s.parse::<u64>() else { continue };
            max = max.max(n);
        }
    }

    Ok(max + 1)
}

fn write_msg(path: &Path, msg: &Message) -> Result<usize, DeliveryError> {
    // MH format doesn't use From_ lines
    let data = if msg.from_line().is_some() {
        crate::mail::skip_from_lines(msg.as_bytes())
    } else {
        msg.as_bytes()
    };

    let file = File::create(path)?;
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
    drop(w);

    let file = File::open(path)?;
    file.sync_all()?;

    Ok(bytes + extra)
}
