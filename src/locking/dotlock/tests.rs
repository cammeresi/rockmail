use super::*;
use std::thread;
use tempfile::tempdir;

#[test]
fn create_and_remove() {
    let dir = tempdir().unwrap();
    let lock = dir.path().join("test.lock");

    assert!(create_lock(&lock).is_ok());
    assert!(lock.exists());
    assert_eq!(create_lock(&lock).unwrap_err(), LockError::Exists);
    assert!(remove_lock(&lock).is_ok());
    assert!(!lock.exists());
}

#[test]
fn mtime() {
    let dir = tempdir().unwrap();
    let lock = dir.path().join("test.lock");

    create_lock(&lock).unwrap();
    let mtime = lock_mtime(&lock);
    assert!(mtime.is_some());
    remove_lock(&lock).unwrap();
}

#[test]
fn mtime_nonexistent() {
    let dir = tempdir().unwrap();
    let lock = dir.path().join("nonexistent.lock");
    assert!(lock_mtime(&lock).is_none());
}

#[test]
fn remove_nonexistent() {
    let dir = tempdir().unwrap();
    let lock = dir.path().join("nonexistent.lock");
    assert_eq!(
        remove_lock(&lock).unwrap_err(),
        LockError::Io(std::io::ErrorKind::NotFound.into()),
    );
}

#[test]
fn missing_directory() {
    let lock = Path::new("/nonexistent/dir/test.lock");
    assert_eq!(create_lock(lock).unwrap_err(), LockError::Unavailable);
}

#[test]
fn concurrent() {
    let dir = tempdir().unwrap();
    let lock = dir.path().join("race.lock");
    let n = 10;

    let handles: Vec<_> = (0..n)
        .map(|_| {
            let p = lock.clone();
            thread::spawn(move || create_lock(&p).is_ok())
        })
        .collect();

    let wins: usize = handles
        .into_iter()
        .map(|h| h.join().unwrap() as usize)
        .sum();
    assert_eq!(wins, 1);
}

#[test]
fn lock_permissions() {
    let dir = tempdir().unwrap();
    let lock = dir.path().join("perm.lock");

    create_lock(&lock).unwrap();
    let meta = fs::metadata(&lock).unwrap();
    assert_eq!(meta.permissions().mode() & 0o777, LOCK_PERM);
    remove_lock(&lock).unwrap();
}

#[test]
fn special_chars_in_path() {
    let dir = tempdir().unwrap();
    let lock = dir.path().join("test with spaces.lock");

    assert!(create_lock(&lock).is_ok());
    assert!(lock.exists());
    assert!(remove_lock(&lock).is_ok());
}

#[test]
fn directory_as_target() {
    let dir = tempdir().unwrap();
    let lock = dir.path().join("dir.lock");
    fs::create_dir(&lock).unwrap();
    // hard_link to a directory fails
    assert!(create_lock(&lock).is_err());
}
