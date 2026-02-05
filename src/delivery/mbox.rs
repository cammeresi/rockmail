#[cfg(test)]
mod tests;

use std::fs::OpenOptions;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use super::{DeliveryError, DeliveryOpts, DeliveryResult};
use crate::locking::FileLock;
use crate::mail::{Message, generate as from_line};

/// Deliver a message to an mbox file.
///
/// Appends the message with proper From_ escaping. A From_ line is
/// prepended if the message doesn't start with one.
/// Acquires flock before writing for concurrent safety.
pub fn deliver(
    path: &Path, msg: &Message, sender: &str, opts: DeliveryOpts,
) -> Result<DeliveryResult, DeliveryError> {
    let _guard = FileLock::acquire(path)?;
    deliver_inner(path, msg, sender, opts)
}

#[cfg(test)]
pub fn deliver_test(
    path: &Path, msg: &Message, sender: &str,
) -> Result<DeliveryResult, DeliveryError> {
    deliver(path, msg, sender, DeliveryOpts::default())
}

fn deliver_inner(
    path: &Path, msg: &Message, sender: &str, opts: DeliveryOpts,
) -> Result<DeliveryResult, DeliveryError> {
    let file = OpenOptions::new().create(true).append(true).open(path)?;

    let mut w = BufWriter::new(file);
    let mut bytes = 0;

    // From_ line (either existing or generated)
    let header = msg.header();
    let hdr = if let Some(fl) = msg.from_line() {
        w.write_all(fl)?;
        w.write_all(b"\n")?;
        bytes += fl.len() + 1;
        header.get(fl.len() + 1..).unwrap_or(&[])
    } else {
        // Generate a From_ line
        let line = from_line(sender);
        w.write_all(&line)?;
        bytes += line.len();
        header
    };

    // Headers with From escaping
    bytes += write_escaped(&mut w, hdr)?;

    // Blank line separator
    w.write_all(b"\n")?;
    bytes += 1;

    // Body with From escaping
    bytes += write_escaped(&mut w, msg.body())?;

    // Ensure trailing newline (unless raw mode)
    if !opts.raw && !msg.body().ends_with(b"\n") {
        w.write_all(b"\n")?;
        bytes += 1;
    }

    // Extra blank line after message (mbox separator)
    w.write_all(b"\n")?;
    bytes += 1;

    w.flush()?;
    let file = w.into_inner().map_err(|e| e.into_error())?;
    file.sync_all()?;

    Ok(DeliveryResult {
        bytes,
        path: path.display().to_string(),
    })
}

/// Write data with From_ escaping.
///
/// Lines starting with "From " are escaped by prepending ">".
fn write_escaped<W>(w: &mut W, data: &[u8]) -> io::Result<usize>
where
    W: Write,
{
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
