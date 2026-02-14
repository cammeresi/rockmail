use std::io::{ErrorKind, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use super::{DeliveryError, DeliveryResult, io_err};
use crate::mail::Message;
use crate::variables::{
    DEF_SHELL, DEF_SHELLFLAGS, Environment, VAR_SHELL, VAR_SHELLFLAGS,
};

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
    let p = Path::new(cmd);
    let me = |e, op| io_err(e, p, op);
    let shell = env.get(VAR_SHELL).unwrap_or(DEF_SHELL);
    let flags = env.get(VAR_SHELLFLAGS).unwrap_or(DEF_SHELLFLAGS);
    let mut child = Command::new(shell)
        .arg(flags)
        .arg(cmd)
        .env_clear()
        .envs(env.iter())
        .stdin(Stdio::piped())
        .stdout(if grab { Stdio::piped() } else { Stdio::null() })
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| me(e, "spawn"))?;

    let data = msg.as_bytes();
    let stdout = child.stdout.take();

    if let Some(mut stdin) = child.stdin.take()
        && let Err(e) = stdin.write_all(data)
        && e.kind() != ErrorKind::BrokenPipe
    {
        return Err(me(e, "write"));
    }

    let captured = if let Some(mut r) = stdout {
        let mut buf = Vec::new();
        r.read_to_end(&mut buf).map_err(|e| me(e, "read"))?;
        Some(buf)
    } else {
        None
    };

    let status = crate::util::wait_timeout(&mut child, env.timeout(), cmd)
        .map_err(|e| me(e, "wait"))?;

    if wait && !status.success() {
        if let Some(code) = status.code() {
            return Err(DeliveryError::PipeExit(code));
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            if let Some(sig) = status.signal() {
                return Err(DeliveryError::PipeSignal(sig));
            }
        }
        return Err(DeliveryError::PipeExit(-1));
    }

    Ok(PipeResult {
        bytes: data.len(),
        output: captured,
    })
}

#[cfg(test)]
pub fn deliver_test(
    cmd: &str, msg: &Message, filter: bool,
) -> Result<PipeResult, DeliveryError> {
    deliver(cmd, msg, filter, false, false, &Environment::from_process())
}
