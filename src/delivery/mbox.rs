use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Seek, SeekFrom, Write};
use std::path::Path;

use super::{DeliveryError, DeliveryOpts, DeliveryResult, io_err};
use crate::locking::FileLock;
use crate::mail::{Message, generate as from_line};
use crate::variables::DEV_NULL;

#[cfg(test)]
mod tests;

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

fn write_body(
    w: &mut BufWriter<&File>, msg: &Message, sender: &str, opts: DeliveryOpts,
) -> io::Result<usize> {
    let mut bytes = 0;

    let header = msg.header();
    let hdr = if let Some(fl) = msg.from_line() {
        w.write_all(fl)?;
        w.write_all(b"\n")?;
        bytes += fl.len() + 1;
        header.get(fl.len() + 1..).unwrap_or(&[])
    } else {
        let line = from_line(sender);
        w.write_all(&line)?;
        bytes += line.len();
        header
    };

    bytes += write_escaped(w, hdr)?;

    w.write_all(b"\n")?;
    bytes += 1;

    bytes += write_escaped(w, msg.body())?;

    if !opts.raw && !msg.body().ends_with(b"\n") {
        w.write_all(b"\n")?;
        bytes += 1;
    }

    w.write_all(b"\n")?;
    bytes += 1;

    w.flush()?;
    Ok(bytes)
}

fn deliver_inner(
    path: &Path, msg: &Message, sender: &str, opts: DeliveryOpts,
) -> Result<DeliveryResult, DeliveryError> {
    let me = |e, op| io_err(e, path, op);
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(|e| me(e, "open"))?;
    file.seek(SeekFrom::End(0)).map_err(|e| me(e, "seek"))?;
    let saved = file.metadata().map_err(|e| me(e, "stat"))?.len();

    let mut w = BufWriter::new(&file);
    match write_body(&mut w, msg, sender, opts) {
        Ok(bytes) => {
            drop(w);
            if path != Path::new(DEV_NULL) {
                file.sync_all().map_err(|e| me(e, "sync"))?;
            }
            Ok(DeliveryResult {
                bytes,
                path: path.display().to_string(),
            })
        }
        Err(e) => {
            drop(w);
            if let Err(te) = file.set_len(saved) {
                eprintln!("truncate {}: {te}", path.display());
            }
            Err(me(e, "write"))
        }
    }
}

/// Deliver a message to an mbox file.
///
/// Appends the message with proper From_ escaping. A From_ line is
/// prepended if the message doesn't start with one.
/// Acquires flock before writing for concurrent safety.
pub fn deliver(
    path: &Path, msg: &Message, sender: &str, opts: DeliveryOpts,
) -> Result<DeliveryResult, DeliveryError> {
    // Locking /dev/null would be silly (matches procmail behavior).
    let _guard = if path != Path::new(DEV_NULL) {
        Some(FileLock::acquire_blocking(path)?)
    } else {
        None
    };
    deliver_inner(path, msg, sender, opts)
}

#[cfg(test)]
pub fn deliver_test(
    path: &Path, msg: &Message, sender: &str,
) -> Result<DeliveryResult, DeliveryError> {
    deliver(path, msg, sender, DeliveryOpts::default())
}
