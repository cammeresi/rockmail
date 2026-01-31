#[cfg(test)]
mod tests;

use std::io::Write;
use std::process::{Command, Stdio};

use super::{DeliveryError, DeliveryResult};
use crate::mail::Message;

/// Deliver a message by piping to a command.
///
/// The command is executed via /bin/sh -c.
/// The message is written to the command's stdin.
///
/// If `filter` is true, the command's stdout is captured and returned
/// as bytes (for filter mode recipes).
pub fn deliver(
    cmd: &str, msg: &Message, filter: bool,
) -> Result<PipeResult, DeliveryError> {
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg(cmd)
        .stdin(Stdio::piped())
        .stdout(if filter {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stderr(Stdio::inherit())
        .spawn()?;

    let data = msg.as_bytes();

    // Write message to stdin
    if let Some(mut stdin) = child.stdin.take() {
        // Ignore broken pipe - command may exit early
        let _ = stdin.write_all(data);
    }

    let output = child.wait_with_output()?;

    if !output.status.success() {
        if let Some(code) = output.status.code() {
            return Err(DeliveryError::PipeExit(code));
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            if let Some(sig) = output.status.signal() {
                return Err(DeliveryError::PipeSignal(sig));
            }
        }
        return Err(DeliveryError::PipeExit(-1));
    }

    Ok(PipeResult {
        bytes: data.len(),
        output: if filter { Some(output.stdout) } else { None },
    })
}

/// Result of pipe delivery.
#[derive(Debug)]
pub struct PipeResult {
    /// Bytes written to command.
    pub bytes: usize,
    /// Captured stdout if filter mode.
    pub output: Option<Vec<u8>>,
}

impl From<PipeResult> for DeliveryResult {
    fn from(r: PipeResult) -> Self {
        DeliveryResult {
            bytes: r.bytes,
            path: "|command".to_string(),
        }
    }
}
