use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{DeliveryError, DeliveryOpts, DeliveryResult};
use crate::mail::{Message, skip_from_lines};

/// Matches procmail's MAILDIRretries (config.h:243).
const RETRIES: u32 = 5;

#[cfg(test)]
mod tests;

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
    /// Create a new namer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate unique filename in Maildir format: time.pid_serial.hostname
    pub fn filename(&mut self) -> Result<String, DeliveryError> {
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

fn hostname() -> String {
    nix::unistd::gethostname()
        .ok()
        .and_then(|s| s.into_string().ok())
        .unwrap_or_else(|| "localhost".to_string())
}

pub(super) fn ensure_dirs(path: &Path) -> Result<(), DeliveryError> {
    for sub in ["tmp", "new", "cur"] {
        fs::create_dir_all(path.join(sub))?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct WriteOpts {
    raw: bool,
    strip_from: bool,
    /// Ensure trailing blank line (like procmail ft_forceblank).
    force_blank: bool,
}

fn write_msg(
    path: &Path, msg: &Message, opts: WriteOpts,
) -> Result<usize, DeliveryError> {
    let file = File::create(path)?;
    let mut w = BufWriter::new(file);

    let data = msg.as_bytes();
    let data = if opts.strip_from && msg.from_line().is_some() {
        skip_from_lines(data)
    } else {
        data
    };

    w.write_all(data)?;
    let bytes = data.len();

    let tail = if opts.force_blank {
        b"\n\n".as_slice()
    } else {
        b"\n"
    };
    let extra = if !opts.raw && !data.ends_with(tail) {
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

/// Deliver a message to an existing directory.
///
/// Unlike Maildir, writes directly without tmp/new/cur structure.
/// Unlike Maildir, preserves From_ lines (matching procmail ft_DIR).
pub fn deliver_dir(
    path: &Path, msg: &Message, opts: DeliveryOpts,
) -> Result<DeliveryResult, DeliveryError> {
    fs::create_dir_all(path)?;

    let name = Namer::new().filename()?;
    let dest = path.join(format!("msg.{}", name));

    let wo = WriteOpts {
        raw: opts.raw,
        strip_from: false,
        force_blank: true,
    };
    let bytes = write_msg(&dest, msg, wo)?;

    Ok(DeliveryResult {
        bytes,
        path: dest.display().to_string(),
    })
}

/// Deliver a message to a Maildir folder.
///
/// Creates the Maildir structure (tmp, new, cur) if needed.
/// Writes to tmp/ then hard-links to new/ for atomic delivery.
/// Retries with a fresh filename on EEXIST (matching procmail).
pub fn deliver(
    namer: &mut Namer, path: &Path, msg: &Message, opts: DeliveryOpts,
) -> Result<DeliveryResult, DeliveryError> {
    ensure_dirs(path)?;

    let wo = WriteOpts {
        raw: opts.raw,
        strip_from: true,
        force_blank: false,
    };

    for _ in 0..RETRIES {
        let name = namer.filename()?;
        let tmp = path.join("tmp").join(&name);
        let new = path.join("new").join(&name);

        let bytes = write_msg(&tmp, msg, wo)?;

        match fs::hard_link(&tmp, &new) {
            Ok(()) => {
                if let Err(e) = fs::remove_file(&tmp) {
                    eprintln!("error unlinking {}: {e}", tmp.display());
                }
                return Ok(DeliveryResult {
                    bytes,
                    path: new.display().to_string(),
                });
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                fs::remove_file(&tmp)?;
                continue;
            }
            Err(_) => {
                // Fall back to rename for non-collision link failures like
                // EXDEV, matching procmail mailfold.c:274-276.
                if !new.exists()
                    && let Ok(()) = fs::rename(&tmp, &new)
                {
                    return Ok(DeliveryResult {
                        bytes,
                        path: new.display().to_string(),
                    });
                }
                fs::remove_file(&tmp)?;
                continue;
            }
        }
    }

    Err(DeliveryError::UniqueFile)
}

/// Hard-link `src` into the `new/` subdirectory of a Maildir.
pub(super) fn link_unique(
    namer: &mut Namer, path: &Path, src: &Path,
) -> Result<String, DeliveryError> {
    ensure_dirs(path)?;
    let name = namer.filename()?;
    let dest = path.join("new").join(&name);
    fs::hard_link(src, &dest)?;
    Ok(dest.display().to_string())
}

#[cfg(test)]
pub fn deliver_test(
    path: &Path, msg: &Message,
) -> Result<DeliveryResult, DeliveryError> {
    deliver(&mut Namer::new(), path, msg, DeliveryOpts::default())
}
