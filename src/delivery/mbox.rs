#[cfg(test)]
mod tests;

use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::Path;

use super::{DeliveryError, DeliveryResult};
use crate::mail::{Message, generate as from_line};

/// Deliver a message to an mbox file.
///
/// Appends the message with proper From_ escaping. A From_ line is
/// prepended if the message doesn't start with one.
///
/// The message is followed by a blank line separator.
pub fn deliver(
    path: &Path, msg: &Message, sender: &str,
) -> Result<DeliveryResult, DeliveryError> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;

    let mut w = BufWriter::new(file);
    let mut bytes = 0;

    // From_ line (either existing or generated)
    let header = msg.header();
    let header_to_escape = if let Some(fl) = msg.from_line() {
        // Write existing From_ line as-is (with newline)
        w.write_all(fl)?;
        w.write_all(b"\n")?;
        bytes += fl.len() + 1;
        // Escape remaining headers (after the From_ line)
        &header[fl.len() + 1..]
    } else {
        // Generate a From_ line
        let line = from_line(sender);
        w.write_all(&line)?;
        bytes += line.len();
        header
    };

    // Headers with From escaping
    bytes += write_escaped(&mut w, header_to_escape)?;

    // Blank line separator
    w.write_all(b"\n")?;
    bytes += 1;

    // Body with From escaping
    bytes += write_escaped(&mut w, msg.body())?;

    // Ensure trailing newline
    if !msg.body().ends_with(b"\n") {
        w.write_all(b"\n")?;
        bytes += 1;
    }

    // Extra blank line after message (mbox separator)
    w.write_all(b"\n")?;
    bytes += 1;

    w.flush()?;
    drop(w);

    // fsync for durability
    let file = OpenOptions::new().read(true).open(path)?;
    file.sync_all()?;

    Ok(DeliveryResult {
        bytes,
        path: path.display().to_string(),
    })
}

/// Write data with From_ escaping.
///
/// Lines starting with "From " are escaped by prepending ">".
fn write_escaped<W: Write>(w: &mut W, data: &[u8]) -> std::io::Result<usize> {
    let mut bytes = 0;
    let mut start = 0;
    let mut at_line_start = true;

    for (i, &b) in data.iter().enumerate() {
        if at_line_start && data[i..].starts_with(b"From ") {
            // Write any pending data
            if start < i {
                w.write_all(&data[start..i])?;
                bytes += i - start;
            }
            // Write escape
            w.write_all(b">")?;
            bytes += 1;
            start = i;
        }
        at_line_start = b == b'\n';
    }

    // Write remaining data
    if start < data.len() {
        w.write_all(&data[start..])?;
        bytes += data.len() - start;
    }

    Ok(bytes)
}
