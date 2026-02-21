use std::process::{Command, Stdio};

use super::*;

#[test]
#[should_panic(expected = "umask conflict")]
fn umask_conflict() {
    set_umask(0o022);
}

#[test]
fn wait_timeout_normal_exit() {
    let mut child = Command::new("true").spawn().unwrap();
    let status =
        wait_timeout(&mut child, Duration::from_secs(5), "true").unwrap();
    assert!(status.success());
}

#[test]
fn wait_timeout_kills_slow() {
    let mut child = Command::new("sleep")
        .arg("60")
        .stdin(Stdio::null())
        .spawn()
        .unwrap();

    let start = Instant::now();
    let status =
        wait_timeout(&mut child, Duration::from_secs(1), "sleep").unwrap();
    let elapsed = start.elapsed().as_secs();

    assert!(!status.success());
    assert!(elapsed < 4);
}

#[test]
fn zero_timeout_waits() {
    let mut child = Command::new("true").spawn().unwrap();
    let status = wait_timeout(&mut child, Duration::MAX, "true").unwrap();
    assert!(status.success());
}

#[test]
fn sigkill_after_sigterm_ignored() {
    // Ignore SIGTERM, busy-wait in the shell itself (no child to leak).
    let mut child = Command::new("sh")
        .arg("-c")
        .arg("trap '' TERM; while true; do :; done")
        .stdin(Stdio::null())
        .spawn()
        .unwrap();

    let start = Instant::now();
    let status =
        wait_timeout(&mut child, Duration::from_secs(1), "sh").unwrap();
    let elapsed = start.elapsed().as_secs();

    assert!(!status.success());
    assert!(elapsed < 8);
}
