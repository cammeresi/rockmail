//! Integration tests for the rockmail binary.

use std::fs;
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::process::{Command, Stdio};

use tempfile::TempDir;

#[allow(unused)]
mod common;

fn run(dir: &Path, args: &[&str], input: &[u8]) -> (Vec<u8>, i32) {
    let mut child = Command::new(common::rockmail())
        .args(args)
        .current_dir(dir)
        .env("RUST_LOG", "info")
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
    let out = child.wait_with_output().expect("wait");
    (out.stderr, out.status.code().unwrap_or(-1))
}

fn write_rc(dir: &Path, rc: &str) -> String {
    let path = dir.join("rcfile");
    let rc = rc.replace("$DIR", dir.to_str().unwrap());
    fs::write(&path, rc).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    path.to_str().unwrap().to_string()
}

#[test]
fn logfile_written() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let log = d.join("log");
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/default
LOGFILE=$DIR/log
VERBOSE=on

:0
inbox
",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    assert!(log.exists(), "logfile not created");
    let content = fs::read_to_string(&log).unwrap();
    assert!(
        content.contains("Delivered"),
        "logfile missing delivery entry: {content:?}"
    );
}

#[test]
fn log_variable_writes_to_logfile() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let log = d.join("log");
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/default
LOGFILE=$DIR/log

:0
{
    LOG=hello_from_LOG
}
",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    assert!(log.exists(), "logfile not created");
    let content = fs::read_to_string(&log).unwrap();
    assert!(
        content.contains("hello_from_LOG"),
        "LOG variable text not in logfile: {content:?}"
    );
}

// No gold test for Maildir secondaries: procmail has a bug where
// setlastfolder() clobbers buf2 before the secondary link loop
// (mailfold.c:282), so Maildir secondary linking always fails with
// ENOENT.  MH/Dir work because buf2 is restored at line 305.
#[test]
fn secondary_maildir() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/default/

:0
inbox/ copy/
",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);

    let inbox = d.join("inbox/new");
    let copy = d.join("copy/new");
    assert!(inbox.is_dir(), "inbox/new not created");
    assert!(copy.is_dir(), "copy/new not created");

    let inbox_files: Vec<_> = fs::read_dir(&inbox).unwrap().flatten().collect();
    let copy_files: Vec<_> = fs::read_dir(&copy).unwrap().flatten().collect();
    assert_eq!(inbox_files.len(), 1, "expected 1 file in inbox/new");
    assert_eq!(copy_files.len(), 1, "expected 1 file in copy/new");

    // Same content
    let a = fs::read(inbox_files[0].path()).unwrap();
    let b = fs::read(copy_files[0].path()).unwrap();
    assert_eq!(a, b, "inbox and copy content differ");

    // Hard-linked (same inode)
    let ia = fs::metadata(inbox_files[0].path()).unwrap().ino();
    let ib = fs::metadata(copy_files[0].path()).unwrap().ino();
    assert_eq!(ia, ib, "expected hard link (same inode)");
}
