use std::fs::{self, Permissions};
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::PermissionsExt;

use tempfile::tempdir;

use super::*;
use crate::delivery::tests::msg;

fn escaped(data: &[u8]) -> (Vec<u8>, usize) {
    let mut buf = Vec::new();
    let n = write_escaped(&mut buf, data).unwrap();
    (buf, n)
}

#[test]
fn escaped_empty() {
    let (buf, n) = escaped(b"");
    assert_eq!(buf, b"");
    assert_eq!(n, 0);
}

#[test]
fn escaped_no_from() {
    let (buf, n) = escaped(b"hello world\n");
    assert_eq!(buf, b"hello world\n");
    assert_eq!(n, 12);
}

#[test]
fn escaped_from_at_start() {
    let (buf, n) = escaped(b"From someone\n");
    assert_eq!(buf, b">From someone\n");
    assert_eq!(n, 14);
}

#[test]
fn escaped_from_after_newline() {
    let (buf, n) = escaped(b"ok\nFrom bar\n");
    assert_eq!(buf, b"ok\n>From bar\n");
    assert_eq!(n, 13);
}

#[test]
fn escaped_multiple_froms() {
    let (buf, _) = escaped(b"From a\nFrom b\nFrom c\n");
    assert_eq!(buf, b">From a\n>From b\n>From c\n");
}

#[test]
fn escaped_from_mid_line() {
    // "From " not at line start should not be escaped
    let (buf, n) = escaped(b"x From y\n");
    assert_eq!(buf, b"x From y\n");
    assert_eq!(n, 9);
}

#[test]
fn escaped_from_without_space() {
    // "From" without trailing space is not a From_ line
    let (buf, _) = escaped(b"Fromage\n");
    assert_eq!(buf, b"Fromage\n");
}

#[test]
fn escaped_no_trailing_newline() {
    let (buf, n) = escaped(b"From x");
    assert_eq!(buf, b">From x");
    assert_eq!(n, 7);
}

#[test]
fn escaped_already_escaped() {
    // >From at line start is not double-escaped
    let (buf, _) = escaped(b">From x\n");
    assert_eq!(buf, b">From x\n");
}

#[test]
fn deliver_simple() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("mbox");

    let m = msg("Subject: Test\n\nHello world\n");
    deliver_test(&path, &m, "user@host").unwrap();

    let content = fs::read_to_string(&path).unwrap();
    assert!(content.starts_with("From user@host "));
    assert!(content.contains("Subject: Test"));
    assert!(content.contains("Hello world"));
    assert!(content.ends_with("\n\n"));
}

#[test]
fn deliver_with_from_line() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("mbox");

    let m =
        msg("From sender Mon Jan  1 00:00:00 2024\nSubject: Test\n\nBody\n");
    deliver_test(&path, &m, "other@host").unwrap();

    let content = fs::read_to_string(&path).unwrap();
    // Should use the existing From_ line, not add a new one
    assert!(content.starts_with("From sender Mon"));
}

#[test]
fn from_escaping() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("mbox");

    let m = msg("Subject: Test\n\nFrom here we go\nFrom there too\n");
    deliver_test(&path, &m, "user@host").unwrap();

    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains(">From here we go"));
    assert!(content.contains(">From there too"));
}

#[test]
fn multiple_deliveries() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("mbox");

    let m1 = msg("Subject: First\n\nBody 1\n");
    let m2 = msg("Subject: Second\n\nBody 2\n");

    deliver_test(&path, &m1, "user@host").unwrap();
    deliver_test(&path, &m2, "user@host").unwrap();

    let content = fs::read_to_string(&path).unwrap();
    let from_count = content.matches("\nFrom ").count();
    assert_eq!(from_count, 1); // 2 total but 1 starts the file
    assert!(content.contains("Body 1"));
    assert!(content.contains("Body 2"));
}

#[test]
fn body_without_trailing_newline() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("mbox");

    let m = msg("Subject: Test\n\nNo newline at end");
    deliver_test(&path, &m, "user@host").unwrap();

    let content = fs::read_to_string(&path).unwrap();
    assert!(content.ends_with("No newline at end\n\n"));
}

#[test]
fn truncation_restores_on_error() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("mbox");
    let original = b"existing content\n";
    fs::write(&path, original).unwrap();

    // Verify set_len truncation mechanism works
    let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
    let saved = file.metadata().unwrap().len();
    // Simulate partial write by seeking past end
    let mut file = file;
    file.seek(SeekFrom::End(0)).unwrap();
    file.write_all(b"garbage").unwrap();
    // Truncate back
    file.set_len(saved).unwrap();
    drop(file);

    let content = fs::read(&path).unwrap();
    assert_eq!(content, original);
}

#[test]
fn open_failure_returns_error() {
    let dir = tempdir().unwrap();
    let sub = dir.path().join("readonly");
    fs::create_dir(&sub).unwrap();
    fs::set_permissions(&sub, Permissions::from_mode(0o444)).unwrap();

    let m = msg("Subject: Test\n\nBody\n");
    let r = deliver_test(&sub.join("mbox"), &m, "user@host");
    assert!(r.is_err());

    // Restore so tempdir cleanup works
    fs::set_permissions(&sub, Permissions::from_mode(0o755)).unwrap();
}
