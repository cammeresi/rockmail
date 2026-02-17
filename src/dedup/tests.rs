use std::fs;

use tempfile::tempdir;

use super::*;

fn cache(dir: &std::path::Path) -> String {
    dir.join("cache").to_str().unwrap().to_string()
}

fn raw(path: &str) -> Vec<u8> {
    fs::read(path).unwrap()
}

#[test]
fn new_key() {
    let d = tempdir().unwrap();
    let p = cache(d.path());
    assert!(!check_cache("abc", &p, 100).unwrap());
    assert_eq!(raw(&p), b"abc\0\0");
}

#[test]
fn dup() {
    let d = tempdir().unwrap();
    let p = cache(d.path());
    check_cache("abc", &p, 100).unwrap();
    assert!(check_cache("abc", &p, 100).unwrap());
}

#[test]
fn empty_key() {
    let d = tempdir().unwrap();
    let p = cache(d.path());
    assert!(!check_cache("", &p, 100).unwrap());
    assert!(!check_cache("  ", &p, 100).unwrap());
    assert!(!d.path().join("cache").exists());
}

/// Two entries — second reuses trailing null of first's double-null
/// terminator.
#[test]
fn two_entries() {
    let d = tempdir().unwrap();
    let p = cache(d.path());
    check_cache("a", &p, 100).unwrap();
    check_cache("b", &p, 100).unwrap();
    assert_eq!(raw(&p), b"a\0b\0\0");
}

/// Append at end when no gap exists.  Seed the file with a
/// single-null-terminated entry so the scan finds no empty slot.
#[test]
fn append_no_gap() {
    let d = tempdir().unwrap();
    let p = cache(d.path());
    fs::write(&p, b"ab\0").unwrap();
    assert!(!check_cache("cd", &p, 100).unwrap());
    assert_eq!(raw(&p), b"ab\0cd\0\0");
}

/// Overwrite a gap left by a shorter previous entry.  Seed the cache
/// with "longkey\0\0", then manually shorten the entry to leave a null
/// gap, and insert a new key that reuses the gap.
#[test]
fn reuse_gap() {
    let d = tempdir().unwrap();
    let p = cache(d.path());

    // Seed: "aa\0\0bb\0\0"
    check_cache("aa", &p, 100).unwrap();
    check_cache("bb", &p, 100).unwrap();

    // Punch a hole: replace first entry with nulls -> "\0\0\0\0bb\0\0"
    fs::write(&p, b"\0\0\0\0bb\0\0").unwrap();

    // "x" should land at offset 0 (the gap), exercising insert=Some(0)
    assert!(!check_cache("x", &p, 100).unwrap());
    let data = raw(&p);
    // Gap reused: "x\0\0" written at 0, rest truncated by set_len
    assert_eq!(data, b"x\0\0");
}

/// Cache full, no gap, offset falls through to.  Seed with dense content
/// (no nulls) at maxlen so insert=None and n>=maxlen.
#[test]
fn wrap_when_full() {
    let d = tempdir().unwrap();
    let p = cache(d.path());
    // 6 bytes of non-null content, maxlen=6 → no gap, n>=maxlen → offset=0
    fs::write(&p, b"xyzxyz").unwrap();
    assert!(!check_cache("a", &p, 6).unwrap());
    assert_eq!(raw(&p), b"a\0\0");
}
