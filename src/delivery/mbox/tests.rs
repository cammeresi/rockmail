use std::fs;

use tempfile::tempdir;

use super::*;
use crate::delivery::tests::msg;

#[test]
fn deliver_simple() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("mbox");

    let m = msg("Subject: Test\n\nHello world\n");
    deliver(&path, &m, "user@host").unwrap();

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
    deliver(&path, &m, "other@host").unwrap();

    let content = fs::read_to_string(&path).unwrap();
    // Should use the existing From_ line, not add a new one
    assert!(content.starts_with("From sender Mon"));
}

#[test]
fn from_escaping() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("mbox");

    let m = msg("Subject: Test\n\nFrom here we go\nFrom there too\n");
    deliver(&path, &m, "user@host").unwrap();

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

    deliver(&path, &m1, "user@host").unwrap();
    deliver(&path, &m2, "user@host").unwrap();

    let content = fs::read_to_string(&path).unwrap();
    let from_count = content.matches("\nFrom ").count();
    assert_eq!(from_count, 1); // 2 total but 1 starts the file
    assert!(content.contains("Body 1"));
    assert!(content.contains("Body 2"));
}

#[test]
fn escaping_in_headers() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("mbox");

    // Headers shouldn't normally have "From " at line start, but test anyway
    let m = msg("Subject: Test\nFrom scratch\n\nBody\n");
    deliver(&path, &m, "user@host").unwrap();

    let content = fs::read_to_string(&path).unwrap();
    assert!(content.contains(">From scratch"));
}

#[test]
fn body_without_trailing_newline() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("mbox");

    let m = msg("Subject: Test\n\nNo newline at end");
    deliver(&path, &m, "user@host").unwrap();

    let content = fs::read_to_string(&path).unwrap();
    assert!(content.ends_with("No newline at end\n\n"));
}
