use std::fs::File;
use std::io::{ErrorKind, Read, Write};
use std::os::fd::{AsFd, AsRawFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use nix::fcntl::{FcntlArg::F_SETFL, OFlag, fcntl};
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};

use super::{DeliveryError, DeliveryResult, io_err};
use crate::mail::Message;
use crate::variables::{Environment, SHELL, SHELLFLAGS};

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

/// Write `data` to child stdin and read child stdout concurrently
/// using poll(2), avoiding deadlock when pipe buffers fill.
fn pump(child: &mut Child, data: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut stdin = Some(child.stdin.take().unwrap());
    let mut stdout = child.stdout.take().unwrap();

    fcntl(
        stdin.as_ref().unwrap().as_raw_fd(),
        F_SETFL(OFlag::O_NONBLOCK),
    )?;
    fcntl(stdout.as_raw_fd(), F_SETFL(OFlag::O_NONBLOCK))?;

    let mut written = 0;
    let mut out = Vec::new();
    let mut chunk = [0u8; 8192];

    loop {
        // Poll stdin (if still open) and stdout.  The fds vec is
        // scoped so its borrows end before we might drop stdin.
        let (can_write, can_read, rd_hup) = {
            let mut fds = vec![PollFd::new(stdout.as_fd(), PollFlags::POLLIN)];
            if let Some(ref w) = stdin {
                fds.push(PollFd::new(w.as_fd(), PollFlags::POLLOUT));
            }
            poll(&mut fds, PollTimeout::NONE)?;
            let re = fds[0].revents().unwrap_or(PollFlags::empty());
            let we = fds
                .get(1)
                .and_then(|f| f.revents())
                .unwrap_or(PollFlags::empty());
            (
                we.intersects(PollFlags::POLLOUT),
                re.intersects(PollFlags::POLLIN),
                re.intersects(PollFlags::POLLHUP)
                    && !re.intersects(PollFlags::POLLIN),
            )
        };

        if can_write {
            let w = stdin.as_mut().unwrap();
            match w.write(&data[written..]) {
                Ok(n) => {
                    written += n;
                    if written >= data.len() {
                        stdin = None;
                    }
                }
                Err(e) if e.kind() == ErrorKind::BrokenPipe => {
                    stdin = None;
                }
                Err(e) if e.kind() == ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            }
        }

        if can_read {
            match stdout.read(&mut chunk) {
                Ok(0) => return Ok(out),
                Ok(n) => out.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == ErrorKind::WouldBlock => {}
                Err(e) => return Err(e),
            }
        } else if rd_hup {
            return Ok(out);
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
    env: &Environment, stderr: &File,
) -> Result<PipeResult, DeliveryError> {
    let grab = filter || capture;
    let p = Path::new(cmd);
    let me = |e, op| io_err(e, p, op);
    let shell = env.get_or_default(&SHELL);
    let flags = env.get_or_default(&SHELLFLAGS);
    let child_stderr = stderr
        .try_clone()
        .map(Stdio::from)
        .unwrap_or_else(|_| Stdio::null());
    let mut child = Command::new(shell)
        .arg(flags)
        .arg(cmd)
        .env_clear()
        .envs(env.iter())
        .stdin(Stdio::piped())
        .stdout(if grab { Stdio::piped() } else { Stdio::null() })
        .stderr(child_stderr)
        .process_group(0)
        .spawn()
        .map_err(|e| me(e, "spawn"))?;

    let (bytes, captured) = if grab {
        let mut data = Vec::new();
        // ft_forceblank: force trailing \n (mailfold.c:115-118)
        msg.write_to_forceblank(&mut data).expect("Vec write");
        let out = pump(&mut child, &data).map_err(|e| me(e, "pipe"))?;
        (data.len(), Some(out))
    } else {
        let mut n = 0;
        if let Some(mut w) = child.stdin.take() {
            // ft_forceblank: force trailing \n (mailfold.c:115-118)
            let r = msg.write_to_forceblank(&mut w);
            drop(w);
            match r {
                Ok(written) => n = written,
                Err(e) if e.kind() != ErrorKind::BrokenPipe => {
                    return Err(me(e, "write"));
                }
                _ => {}
            }
        }
        (n, None)
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
        bytes,
        output: captured,
    })
}

#[cfg(test)]
pub fn deliver_test(
    cmd: &str, msg: &Message, filter: bool,
) -> Result<PipeResult, DeliveryError> {
    deliver(
        cmd,
        msg,
        filter,
        false,
        false,
        &Environment::from_process(),
        &crate::engine::dup_stderr(),
    )
}
