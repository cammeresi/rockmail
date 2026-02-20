use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use filetime::{FileTime, set_file_mtime};
use tempfile::tempdir;

use super::*;

fn kill(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn hold_lock_briefly(p: &Path, secs: u64) -> Child {
    let sig = p.with_extension("ready");
    let mut child = Command::new("bash")
        .arg("-c")
        .arg(format!(
            // Open fd 9 on the lock file, flock it, signal readiness,
            // sleep, then exit (releasing the lock).
            "exec 9>>{p} && flock -x 9 && touch {sig} && sleep {secs}",
            p = p.display(),
            sig = sig.display(),
        ))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn lock holder");

    for _ in 0..500 {
        if sig.exists() {
            return child;
        }
        thread::sleep(Duration::from_millis(10));
    }
    kill(&mut child);
    panic!("child never acquired lock");
}

fn hold_lock(p: &Path) -> Child {
    hold_lock_briefly(p, 3600)
}

#[test]
fn acquire_temp_cleanup() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("test.lock");

    {
        let _lock = FileLock::acquire_temp(&p).unwrap();
        assert!(p.exists());
    }
    assert!(!p.exists());
}

#[test]
fn acquire_temp_contention() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("held.lock");

    let mut child = hold_lock(&p);

    let r = FileLock::acquire_temp(&p);
    assert_eq!(r.unwrap_err(), LockError::Exists);

    kill(&mut child);
}

#[test]
fn acquire_temp_retry_succeeds() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("retry.lock");

    // Child holds lock for 1 second then exits.
    let mut child = hold_lock_briefly(&p, 1);

    let r = FileLock::acquire_temp_retry(&p, 5, 1);
    assert!(r.is_ok());

    let _ = child.wait();
}

#[test]
fn acquire_temp_retry_timeout() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("timeout.lock");

    let mut child = hold_lock(&p);

    // Set mtime far in the future so age is negative (Duration::since
    // returns Err), preventing stale removal from firing.
    set_file_mtime(&p, FileTime::from_unix_time(i64::MAX / 2, 0)).unwrap();
    let r = FileLock::acquire_temp_retry(&p, 2, 1);
    assert_eq!(r.unwrap_err(), LockError::Exists);

    kill(&mut child);
}

#[test]
fn stale_lock_removed() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("stale.lock");

    // Create a small file and backdate it.
    File::create(&p).unwrap().write_all(b"0").unwrap();
    set_file_mtime(&p, FileTime::from_unix_time(0, 0)).unwrap();

    // No process holds the flock, so after detecting staleness the
    // retry loop removes the file and acquires the lock.
    let lock = FileLock::acquire_temp_retry(&p, 2, 1).unwrap();
    assert!(p.exists());
    drop(lock);
    assert!(!p.exists());
}

#[test]
fn stale_large_file_kept() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("big.lock");

    // Pre-create a large file, then have a child flock it.
    let mut f = File::create(&p).unwrap();
    f.write_all(&[b'x'; 32]).unwrap();
    drop(f);

    let mut child = hold_lock(&p);

    // Backdate so it looks stale, but size > MAX_LOCK_SIZE
    // prevents forced removal.
    set_file_mtime(&p, FileTime::from_unix_time(0, 0)).unwrap();
    let r = FileLock::acquire_temp_retry(&p, 2, 1);
    assert_eq!(r.unwrap_err(), LockError::Exists);
    assert!(p.exists());

    kill(&mut child);
}

#[test]
fn stale_dir_kept() {
    let dir = tempdir().unwrap();
    let p = dir.path().join("dir.lock");
    fs::create_dir(&p).unwrap();

    let r = FileLock::acquire_temp_retry(&p, 2, 1);
    assert_eq!(
        r.unwrap_err(),
        LockError::Io(io::ErrorKind::IsADirectory.into()),
    );
    assert!(p.exists());
}

#[test]
fn missing_directory() {
    let p = Path::new("/nonexistent/dir/test.lock");
    let r = FileLock::acquire_temp(p);
    assert_eq!(r.unwrap_err(), LockError::Unavailable);
}

#[test]
#[should_panic(expected = "child never acquired lock")]
fn hold_lock_briefly_panics_on_failure() {
    let _ =
        hold_lock_briefly(Path::new("/nonexistent/dir/test.lock"), 1).wait();
}
