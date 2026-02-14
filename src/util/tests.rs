use std::process::{Command, Stdio};

use super::*;

#[test]
fn wait_timeout_normal_exit() {
    let mut child = Command::new("true").spawn().unwrap();
    let status = wait_timeout(&mut child, Duration::from_secs(5)).unwrap();
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
    let status = wait_timeout(&mut child, Duration::from_secs(1)).unwrap();
    let elapsed = start.elapsed().as_secs();

    assert!(!status.success());
    assert!(elapsed < 5);
}
