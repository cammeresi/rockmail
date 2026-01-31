use std::fs;

use tempfile::tempdir;

use super::*;

fn msg(s: &str) -> Message {
    Message::parse(s.as_bytes())
}

#[test]
fn deliver_creates_dirs() {
    let dir = tempdir().unwrap();
    let maildir = dir.path().join("Maildir");

    let m = msg("Subject: Test\n\nHello\n");
    deliver(&maildir, &m).unwrap();

    assert!(maildir.join("tmp").is_dir());
    assert!(maildir.join("new").is_dir());
    assert!(maildir.join("cur").is_dir());
}

#[test]
fn deliver_to_new() {
    let dir = tempdir().unwrap();
    let maildir = dir.path().join("Maildir");

    let m = msg("Subject: Test\n\nBody\n");
    let r = deliver(&maildir, &m).unwrap();

    assert!(r.path.contains("/new/"));

    // tmp should be empty after delivery
    let tmp_count = fs::read_dir(maildir.join("tmp")).unwrap().count();
    assert_eq!(tmp_count, 0);

    // new should have one file
    let new_count = fs::read_dir(maildir.join("new")).unwrap().count();
    assert_eq!(new_count, 1);
}

#[test]
fn strips_from_line() {
    let dir = tempdir().unwrap();
    let maildir = dir.path().join("Maildir");

    let m =
        msg("From sender Mon Jan  1 00:00:00 2024\nSubject: Test\n\nBody\n");
    let r = deliver(&maildir, &m).unwrap();

    let content = fs::read_to_string(&r.path).unwrap();
    assert!(!content.starts_with("From "));
    assert!(content.starts_with("Subject:"));
}

#[test]
fn unique_names() {
    let n1 = unique_name().unwrap();
    std::thread::sleep(std::time::Duration::from_micros(10));
    let n2 = unique_name().unwrap();
    assert_ne!(n1, n2);
}
