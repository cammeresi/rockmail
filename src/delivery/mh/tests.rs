use std::fs::{self, Permissions};
use std::os::unix::fs::PermissionsExt;

use tempfile::tempdir;

use super::*;
use crate::delivery::tests::msg;

#[test]
fn deliver_creates_folder() {
    let dir = tempdir().unwrap();
    let mh = dir.path().join("inbox");

    let m = msg("Subject: Test\n\nHello\n");
    deliver_test(&mh, &m).unwrap();

    assert!(mh.is_dir());
}

#[test]
fn sequential_numbers() {
    let dir = tempdir().unwrap();
    let mh = dir.path().join("inbox");

    for i in 1..=3 {
        let m = msg(&format!("Subject: Message {}\n\nBody {}\n", i, i));
        let r = deliver_test(&mh, &m).unwrap();
        assert!(r.path.ends_with(&format!("/{}", i)));
    }

    // Verify files
    assert!(mh.join("1").exists());
    assert!(mh.join("2").exists());
    assert!(mh.join("3").exists());
}

#[test]
fn finds_highest_existing() {
    let dir = tempdir().unwrap();
    let mh = dir.path().join("inbox");
    fs::create_dir_all(&mh).unwrap();

    // Create files 1, 3, 5 (simulating gaps)
    fs::write(mh.join("1"), b"msg1").unwrap();
    fs::write(mh.join("3"), b"msg3").unwrap();
    fs::write(mh.join("5"), b"msg5").unwrap();

    // Next should be 6
    let m = msg("Subject: Test\n\nBody\n");
    let r = deliver_test(&mh, &m).unwrap();
    assert!(r.path.ends_with("/6"));
}

#[test]
fn preserves_from_line() {
    let dir = tempdir().unwrap();
    let mh = dir.path().join("inbox");

    let m =
        msg("From sender Mon Jan  1 00:00:00 2024\nSubject: Test\n\nBody\n");
    let r = deliver_test(&mh, &m).unwrap();

    let content = fs::read_to_string(&r.path).unwrap();
    assert!(content.starts_with("From sender"));
}

#[test]
fn empty_body_no_extra_newline() {
    let dir = tempdir().unwrap();
    let mh = dir.path().join("inbox");
    let m = msg("Subject: Test\n\n");
    deliver_test(&mh, &m).unwrap();
    let content = fs::read(mh.join("1")).unwrap();
    assert_eq!(content, b"Subject: Test\n\n");
}

#[test]
fn readonly_dir_returns_error() {
    let dir = tempdir().unwrap();
    let parent = dir.path().join("readonly");
    fs::create_dir(&parent).unwrap();
    fs::set_permissions(&parent, Permissions::from_mode(0o444)).unwrap();

    let mh = parent.join("inbox");
    let m = msg("Subject: Test\n\nBody\n");
    let r = deliver_test(&mh, &m);
    let Err(crate::delivery::DeliveryError::Io { op, .. }) = r else {
        panic!("expected Io error, got {r:?}");
    };
    assert_eq!(op, "create");

    fs::set_permissions(&parent, Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn skips_existing_contiguous() {
    let dir = tempdir().unwrap();
    let mh = dir.path().join("inbox");
    fs::create_dir_all(&mh).unwrap();

    for i in 1..=5 {
        fs::write(mh.join(i.to_string()), format!("msg{i}")).unwrap();
    }

    let m = msg("Subject: Test\n\nBody\n");
    let r = deliver_test(&mh, &m).unwrap();
    assert!(r.path.ends_with("/6"));
}
