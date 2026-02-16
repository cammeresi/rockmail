use std::fs::{self, Permissions};
use std::os::unix::fs::PermissionsExt;

use tempfile::tempdir;

use super::*;
use crate::delivery::{DeliveryError, tests::msg};

// --- dir delivery tests ---

#[test]
fn dir_creates_dir() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("plain");

    let m = msg("Subject: Test\n\nHello\n");
    deliver_dir_test(&target, &m).unwrap();

    assert!(target.is_dir());
    let files: Vec<_> = fs::read_dir(&target).unwrap().collect();
    assert_eq!(files.len(), 1);
    let name = files[0].as_ref().unwrap().file_name();
    assert!(name.to_str().unwrap().starts_with("msg."));
}

#[test]
fn dir_preserves_from_line() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("plain");

    let m =
        msg("From sender Mon Jan  1 00:00:00 2024\nSubject: Test\n\nBody\n");
    let r = deliver_dir_test(&target, &m).unwrap();

    let content = fs::read_to_string(&r.path).unwrap();
    assert!(content.starts_with("From "));
}

#[test]
fn dir_forces_trailing_blank() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("plain");

    // Body ends with a single newline; dir delivery adds one more.
    let m = msg("Subject: Test\n\nBody\n");
    let r = deliver_dir_test(&target, &m).unwrap();

    let content = fs::read_to_string(&r.path).unwrap();
    assert!(content.ends_with("\n\n"));
    assert!(!content.ends_with("\n\n\n"));
}

#[test]
fn dir_no_subdirs() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("plain");

    let m = msg("Subject: Test\n\nHello\n");
    deliver_dir_test(&target, &m).unwrap();

    assert!(!target.join("tmp").exists());
    assert!(!target.join("new").exists());
    assert!(!target.join("cur").exists());
}

#[test]
fn deliver_creates_dirs() {
    let dir = tempdir().unwrap();
    let maildir = dir.path().join("Maildir");

    let m = msg("Subject: Test\n\nHello\n");
    deliver_test(&maildir, &m).unwrap();

    assert!(maildir.join("tmp").is_dir());
    assert!(maildir.join("new").is_dir());
    assert!(maildir.join("cur").is_dir());
}

#[test]
fn deliver_to_new() {
    let dir = tempdir().unwrap();
    let maildir = dir.path().join("Maildir");

    let m = msg("Subject: Test\n\nBody\n");
    let r = deliver_test(&maildir, &m).unwrap();

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
    let r = deliver_test(&maildir, &m).unwrap();

    let content = fs::read_to_string(&r.path).unwrap();
    assert!(!content.starts_with("From "));
    assert!(content.starts_with("Subject:"));
}

#[test]
fn unique_names() {
    let mut namer = Namer::new();
    let n1 = namer.filename().unwrap();
    let n2 = namer.filename().unwrap();
    assert_ne!(n1, n2);
}

#[test]
fn serial_increments_same_second() {
    let mut namer = Namer::new();
    let n1 = namer.filename_at(1_000_000);
    let n2 = namer.filename_at(1_000_000);
    assert!(n1.contains("_0."));
    assert!(n2.contains("_1."));
}

#[test]
fn retry_exhaustion() {
    let dir = tempdir().unwrap();
    let maildir = dir.path().join("Maildir");
    ensure_dirs(&maildir).unwrap();

    // Make new/ read-only so hard_link and rename both fail
    fs::set_permissions(maildir.join("new"), Permissions::from_mode(0o444))
        .unwrap();

    let m = msg("Subject: Test\n\nBody\n");
    let r = deliver(&mut Namer::new(), &maildir, &m, DeliveryOpts::default());
    assert!(matches!(r, Err(DeliveryError::UniqueFile)));

    // Restore so tempdir cleanup works
    fs::set_permissions(maildir.join("new"), Permissions::from_mode(0o755))
        .unwrap();
}
