use std::io::{ErrorKind, Write};
use std::process::{Command, Stdio};

use super::{DeliveryError, DeliveryResult};
use crate::mail::Message;
use crate::variables::{DEF_SHELL, DEF_SHELLFLAGS, Environment};

#[cfg(test)]
mod tests;

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

/// Deliver a message by piping to a command.
///
/// The command is executed via /bin/sh -c.
/// The message is written to the command's stdin.
///
/// If `filter` is true, the command's stdout is captured and returned
/// as bytes (for filter mode recipes).
///
/// If `wait` is true, returns error on non-zero exit (caller handles messaging).
/// If `wait` is false, ignores exit status (original behavior for non-w recipes).
pub fn deliver(
    cmd: &str, msg: &Message, filter: bool, wait: bool, capture: bool,
    env: &Environment,
) -> Result<PipeResult, DeliveryError> {
    let grab = filter || capture;
    let mut child = Command::new(DEF_SHELL)
        .arg(DEF_SHELLFLAGS)
        .arg(cmd)
        .env_clear()
        .envs(env.iter())
        .stdin(Stdio::piped())
        .stdout(if grab { Stdio::piped() } else { Stdio::null() })
        .stderr(Stdio::inherit())
        .spawn()?;

    crate::util::spawn_watchdog(child.id(), env.timeout());

    let data = msg.as_bytes();

    if let Some(mut stdin) = child.stdin.take()
        && let Err(e) = stdin.write_all(data)
        && e.kind() != ErrorKind::BrokenPipe
    {
        return Err(e.into());
    }

    let output = child.wait_with_output()?;

    if wait && !output.status.success() {
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
        output: if grab { Some(output.stdout) } else { None },
    })
}

#[cfg(test)]
pub fn deliver_test(
    cmd: &str, msg: &Message, filter: bool,
) -> Result<PipeResult, DeliveryError> {
    deliver(cmd, msg, filter, false, false, &Environment::from_process())
}
