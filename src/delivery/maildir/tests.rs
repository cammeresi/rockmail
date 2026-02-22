use std::fs::{self, Permissions};
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use tempfile::tempdir;

use super::*;
use crate::delivery::{DeliveryError, tests::msg};
use crate::engine;

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
fn dir_empty_body_no_extra_newline() {
    let dir = tempdir().unwrap();
    let target = dir.path().join("plain");
    let m = msg("Subject: Test\n\n");
    let r = deliver_dir_test(&target, &m).unwrap();
    let content = fs::read(&r.path).unwrap();
    assert_eq!(content, b"Subject: Test\n\n");
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
    let stderr = engine::dup_stderr();
    let r = deliver(
        &mut Namer::new(),
        &maildir,
        &m,
        DeliveryOpts::default(),
        &stderr,
    );
    assert_eq!(r.unwrap_err(), DeliveryError::UniqueFile);

    // Restore so tempdir cleanup works
    fs::set_permissions(maildir.join("new"), Permissions::from_mode(0o755))
        .unwrap();
}

fn unwrap_io(e: DeliveryError) -> (io::ErrorKind, PathBuf, &'static str) {
    let DeliveryError::Io { source, path, op } = e else {
        panic!("expected Io, got {e:?}");
    };
    (source.kind(), path, op)
}

#[test]
#[should_panic(expected = "expected Io")]
fn unwrap_io_panics_on_non_io() {
    unwrap_io(DeliveryError::UniqueFile);
}

#[test]
fn collision_retries_with_new_name() {
    let dir = tempdir().unwrap();
    let maildir = dir.path().join("Maildir");
    ensure_dirs(&maildir).unwrap();

    // Get the filename that a fresh Namer would produce right now.
    let name = Namer::new().filename().unwrap();

    // Pre-create that file in new/ so the first hard_link hits EEXIST.
    fs::write(maildir.join("new").join(&name), b"").unwrap();

    // A fresh Namer in the same second produces the same first name,
    // hits EEXIST, then retries with serial=1 and succeeds.
    let m = msg("Subject: Test\n\nBody\n");
    let stderr = engine::dup_stderr();
    let r = deliver(
        &mut Namer::new(),
        &maildir,
        &m,
        DeliveryOpts::default(),
        &stderr,
    )
    .unwrap();

    assert!(!r.path.ends_with(&name));
    assert!(r.path.contains("/new/"));
    assert_eq!(fs::read_dir(maildir.join("new")).unwrap().count(), 2);
    assert_eq!(fs::read_dir(maildir.join("tmp")).unwrap().count(), 0);
}

#[test]
fn link_unique_creates_file() {
    let dir = tempdir().unwrap();
    let maildir = dir.path().join("Maildir");
    let src = dir.path().join("src.txt");
    fs::write(&src, b"hello").unwrap();

    let mut namer = Namer::new();
    let dest = link_unique(&mut namer, &maildir, &src).unwrap();

    assert!(dest.contains("/new/"));
    assert_eq!(fs::read_to_string(&dest).unwrap(), "hello");
    assert!(src.exists());
}

#[test]
fn link_unique_missing_source() {
    let dir = tempdir().unwrap();
    let maildir = dir.path().join("Maildir");
    let missing = dir.path().join("no_such_file");

    let mut namer = Namer::new();
    let (kind, _, op) =
        unwrap_io(link_unique(&mut namer, &maildir, &missing).unwrap_err());

    assert_eq!(kind, io::ErrorKind::NotFound);
    assert_eq!(op, "link");
}
