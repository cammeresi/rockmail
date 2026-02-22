use std::fs;
use std::process::Command;

use filetime::{FileTime, set_file_mtime};

use super::*;

#[test]
fn invert_ok() {
    assert_eq!(invert_code(EX_OK, true), EX_CANTCREAT);
}

#[test]
fn invert_cantcreat() {
    assert_eq!(invert_code(EX_CANTCREAT, true), EX_OK);
}

#[test]
fn invert_passthrough() {
    assert_eq!(invert_code(EX_TEMPFAIL, true), EX_TEMPFAIL);
}

#[test]
fn invert_noop() {
    assert_eq!(invert_code(EX_OK, false), EX_OK);
}

#[test]
fn cleanup_removes() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("a.lock");
    let b = tmp.path().join("b.lock");
    create_lock(&a).unwrap();
    create_lock(&b).unwrap();
    cleanup(&[a.clone(), b.clone()]);
    assert!(!a.exists());
    assert!(!b.exists());
}

#[test]
fn cleanup_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("gone.lock");
    cleanup(&[a]);
}

#[test]
fn force_unlock_disabled() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("f.lock");
    create_lock(&p).unwrap();
    assert!(!try_force_unlock(&p, 0, 0));
}

#[test]
fn force_unlock_stale() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("f.lock");
    create_lock(&p).unwrap();
    set_file_mtime(&p, FileTime::from_unix_time(0, 0)).unwrap();
    assert!(try_force_unlock(&p, 1, 0));
    assert!(!p.exists());
}

#[test]
fn force_unlock_fresh() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("f.lock");
    create_lock(&p).unwrap();
    assert!(!try_force_unlock(&p, 9999, 0));
    assert!(p.exists());
}

#[test]
fn force_unlock_missing() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("nope.lock");
    assert!(!try_force_unlock(&p, 1, 0));
}

#[test]
fn force_unlock_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path().join("dir.lock");
    fs::create_dir(&d).unwrap();
    assert!(!try_force_unlock(&d, 1, 0));
}

#[test]
fn force_unlock_large() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("big.lock");
    fs::write(&p, vec![0u8; (MAX_LOCK_SIZE + 1) as usize]).unwrap();
    assert!(!try_force_unlock(&p, 1, 0));
}

#[test]
fn exists_decrement() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("e.lock");
    create_lock(&p).unwrap();
    let mut retries = 3i64;
    let r = handle_exists(&p, 0, &mut retries, 0, 0);
    assert_eq!(r, Ok(false));
    assert_eq!(retries, 2);
}

#[test]
fn exists_zero() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("e.lock");
    create_lock(&p).unwrap();
    let mut retries = 0i64;
    assert_eq!(handle_exists(&p, 0, &mut retries, 0, 0), Err(EX_CANTCREAT));
}

#[test]
fn exists_infinite() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("e.lock");
    create_lock(&p).unwrap();
    let mut retries = -1i64;
    let r = handle_exists(&p, 0, &mut retries, 0, 0);
    assert_eq!(r, Ok(false));
    assert_eq!(retries, -1);
}

#[test]
fn exists_force() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("e.lock");
    create_lock(&p).unwrap();
    set_file_mtime(&p, FileTime::from_unix_time(0, 0)).unwrap();
    let mut retries = 0i64;
    let r = handle_exists(&p, 0, &mut retries, 1, 0);
    assert_eq!(r, Ok(true));
    assert!(!p.exists());
}

#[test]
fn nfs_retry_decrement() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("n.lock");
    let mut nfs = 5u32;
    assert_eq!(handle_nfs_error(&p, 0, &mut nfs), Ok(()));
    assert_eq!(nfs, 4);
}

#[test]
fn nfs_retry_exhausted() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("n.lock");
    let mut nfs = 1u32;
    assert_eq!(handle_nfs_error(&p, 0, &mut nfs), Err(EX_UNAVAILABLE));
}

#[test]
fn try_lock_success() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("t.lock");
    let mut retries = -1i64;
    let mut acquired = Vec::new();
    try_lock(&p, 0, &mut retries, 0, 0, &mut acquired).unwrap();
    assert_eq!(acquired.len(), 1);
    assert!(acquired[0].exists());
}

#[test]
fn try_lock_retries_exhausted() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("t.lock");
    create_lock(&p).unwrap();
    let mut retries = 0i64;
    let mut acquired = Vec::new();
    let r = try_lock(&p, 0, &mut retries, 0, 0, &mut acquired);
    assert_eq!(r, Err(EX_CANTCREAT));
}

#[test]
fn check_signal_ok() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("s.lock");
    assert_eq!(check_signal(&p), Ok(()));
}

fn lockfile_bin() -> PathBuf {
    let mut p = std::env::current_exe().unwrap();
    // test binary is in target/debug/deps/lockfile-HASH; go up to target/debug/
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    p.push("lockfile");
    p
}

#[test]
fn no_args() {
    let s = Command::new(lockfile_bin()).output().unwrap();
    assert_eq!(s.status.code(), Some(EX_USAGE as i32));
}

#[test]
fn create_single() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("one.lock");
    let s = Command::new(lockfile_bin()).arg(&p).output().unwrap();
    assert_eq!(s.status.code(), Some(0));
    assert!(p.exists());
}

#[test]
fn create_multiple() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("a.lock");
    let b = tmp.path().join("b.lock");
    let s = Command::new(lockfile_bin())
        .args([&a, &b])
        .output()
        .unwrap();
    assert_eq!(s.status.code(), Some(0));
    assert!(a.exists());
    assert!(b.exists());
}

#[test]
fn invert_exit() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("inv.lock");
    let s = Command::new(lockfile_bin())
        .args(["-!", p.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(s.status.code(), Some(EX_CANTCREAT as i32));
}

#[test]
fn double_invert() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("inv2.lock");
    let s = Command::new(lockfile_bin())
        .args(["-!", "-!", p.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(s.status.code(), Some(0));
}

#[test]
fn retries_exhausted() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("busy.lock");
    create_lock(&p).unwrap();
    let s = Command::new(lockfile_bin())
        .args(["-r", "0", "-S", "0", p.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(s.status.code(), Some(EX_CANTCREAT as i32));
}

#[test]
fn cleanup_on_failure() {
    let tmp = tempfile::tempdir().unwrap();
    let a = tmp.path().join("ok.lock");
    let b = tmp.path().join("busy.lock");
    create_lock(&b).unwrap();
    let s = Command::new(lockfile_bin())
        .args([
            "-r",
            "0",
            "-S",
            "0",
            a.to_str().unwrap(),
            b.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(s.status.code(), Some(EX_CANTCREAT as i32));
    // first lock should have been cleaned up on failure
    assert!(!a.exists());
}

#[test]
fn force_stale() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("stale.lock");
    create_lock(&p).unwrap();
    set_file_mtime(&p, FileTime::from_unix_time(0, 0)).unwrap();
    let s = Command::new(lockfile_bin())
        .args(["-l", "1", "-s", "0", "-S", "0", p.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(s.status.code(), Some(0));
}
