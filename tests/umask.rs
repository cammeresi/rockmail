//! Tests for UMASK variable and default umask behavior.

use std::fs::{self, Permissions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::process::{Command, Stdio};

use tempfile::TempDir;

#[allow(unused)]
mod common;

fn run(dir: &Path, args: &[&str], input: &[u8]) -> i32 {
    let mut child = Command::new(common::rockmail())
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn rockmail");
    if let Err(e) = child.stdin.take().unwrap().write_all(input)
        && e.kind() != ErrorKind::BrokenPipe
    {
        panic!("write stdin: {e}");
    }
    child
        .wait_with_output()
        .expect("wait")
        .status
        .code()
        .unwrap_or(-1)
}

fn write_rc(dir: &Path, rc: &str) -> String {
    let path = dir.join("rcfile");
    let rc = rc.replace("$DIR", dir.to_str().unwrap());
    fs::write(&path, rc).unwrap();
    fs::set_permissions(&path, Permissions::from_mode(0o644)).unwrap();
    path.to_str().unwrap().to_string()
}

const MSG: &[u8] = b"From: user@host\nSubject: Test\n\nBody\n";

/// Default umask is 077, so new mbox files get mode 0600.
#[test]
fn default_umask_0600() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let mbox = d.join("inbox");
    let rc = write_rc(d, "MAILDIR=$DIR\nDEFAULT=$DIR/inbox\n");
    let code = run(d, &["-f", "sender@test", &rc], MSG);
    assert_eq!(code, 0);
    let mode = fs::metadata(&mbox).unwrap().mode() & 0o777;
    assert_eq!(mode, 0o600, "expected 0600, got {mode:03o}");
}

/// UMASK=022 in rcfile widens permissions to 0644.
#[test]
fn umask_override() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let mbox = d.join("inbox");
    let rc = write_rc(d, "MAILDIR=$DIR\nDEFAULT=$DIR/inbox\nUMASK=022\n");
    let code = run(d, &["-f", "sender@test", &rc], MSG);
    assert_eq!(code, 0);
    let mode = fs::metadata(&mbox).unwrap().mode() & 0o777;
    assert_eq!(mode, 0o644, "expected 0644, got {mode:03o}");
}

/// UMASK=022 gives maildir subdirectories mode 0755 (execute bit set).
/// After clearing UPDATE_MASK on the maildir, a second delivery restores it.
#[test]
fn maildir_execute_bit() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let inbox = d.join("inbox");
    let rc = write_rc(
        d,
        "MAILDIR=$DIR\nDEFAULT=$DIR/fallback\nUMASK=022\n\n:0\ninbox/\n",
    );
    let code = run(d, &["-f", "sender@test", &rc], MSG);
    assert_eq!(code, 0);
    for sub in ["tmp", "new", "cur"] {
        let p = inbox.join(sub);
        let mode = fs::metadata(&p).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o755, "inbox/{sub}: expected 0755, got {mode:03o}");
    }

    // Clear UPDATE_MASK (o+x) on the maildir itself.
    let mode = fs::metadata(&inbox).unwrap().mode() & 0o777;
    fs::set_permissions(&inbox, Permissions::from_mode(mode & !0o001)).unwrap();

    // Deliver again; update_perms restores UPDATE_MASK.
    let code = run(d, &["-f", "sender@test", &rc], MSG);
    assert_eq!(code, 0);
    let mode = fs::metadata(&inbox).unwrap().mode() & 0o777;
    assert_eq!(
        mode & 0o001,
        0o001,
        "inbox after re-delivery: expected o+x set, got {mode:03o}"
    );
}
