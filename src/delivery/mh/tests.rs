use std::fs;

use tempfile::tempdir;

use super::*;

fn msg(s: &str) -> Message {
    Message::parse(s.as_bytes())
}

#[test]
fn deliver_creates_folder() {
    let dir = tempdir().unwrap();
    let mh = dir.path().join("inbox");

    let m = msg("Subject: Test\n\nHello\n");
    deliver(&mh, &m).unwrap();

    assert!(mh.is_dir());
}

#[test]
fn sequential_numbers() {
    let dir = tempdir().unwrap();
    let mh = dir.path().join("inbox");

    for i in 1..=3 {
        let m = msg(&format!("Subject: Message {}\n\nBody {}\n", i, i));
        let r = deliver(&mh, &m).unwrap();
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
    let r = deliver(&mh, &m).unwrap();
    assert!(r.path.ends_with("/6"));
}

#[test]
fn strips_from_line() {
    let dir = tempdir().unwrap();
    let mh = dir.path().join("inbox");

    let m =
        msg("From sender Mon Jan  1 00:00:00 2024\nSubject: Test\n\nBody\n");
    let r = deliver(&mh, &m).unwrap();

    let content = fs::read_to_string(&r.path).unwrap();
    assert!(!content.starts_with("From "));
    assert!(content.starts_with("Subject:"));
}
