use std::time::Instant;

use super::*;
use crate::delivery::DeliveryError;
use crate::delivery::tests::msg;
use crate::variables::{SHELL, TIMEOUT};

fn to_bytes(msg: &Message) -> Vec<u8> {
    let mut buf = Vec::new();
    msg.write_to_forceblank(&mut buf).expect("Vec write");
    buf
}

fn pipe_exit(r: Result<PipeResult, DeliveryError>) -> i32 {
    let Err(DeliveryError::PipeExit(code)) = r else {
        panic!("expected PipeExit, got {r:?}");
    };
    code
}

#[test]
fn pipe_to_cat() {
    let m = msg("Subject: Test\n\nBody content\n");
    let r = deliver_test("cat > /dev/null", &m, false).unwrap();
    // +1 for ft_forceblank trailing \n (mailfold.c:115-118)
    assert_eq!(r.bytes, m.len() + 1);
}

#[test]
fn filter_mode() {
    let m = msg("Subject: Test\n\nHello\n");
    let r = deliver_test("cat", &m, true).unwrap();

    let output = r.output.unwrap();
    assert_eq!(output, to_bytes(&m));
}

#[test]
fn filter_transforms() {
    let m = msg("Subject: Test\n\nHello\n");
    let r = deliver_test("tr a-z A-Z", &m, true).unwrap();

    let output = r.output.unwrap();
    let s = String::from_utf8_lossy(&output);
    assert!(s.contains("SUBJECT: TEST"));
    assert!(s.contains("HELLO"));
}

#[test]
fn exit_code_ignored_without_wait() {
    let m = msg("Subject: Test\n\nBody\n");
    // Without wait flag, non-zero exit is ignored
    let r = deliver_test("exit 1", &m, false);
    assert!(r.is_ok());
}

#[test]
fn exit_code_error_with_wait() {
    let m = msg("Subject: Test\n\nBody\n");
    // With wait flag, non-zero exit returns error
    let r = deliver(
        "exit 1",
        &m,
        false,
        true,
        false,
        &Environment::from_process(),
    );

    assert_eq!(pipe_exit(r), 1);
}

#[test]
#[should_panic(expected = "expected PipeExit")]
fn pipe_exit_helper_panics_on_ok() {
    let m = msg("Subject: Test\n\nBody\n");
    pipe_exit(deliver_test("true", &m, false));
}

#[test]
fn early_exit() {
    // Command that reads nothing and exits
    let m = msg(&"Subject: Test\n\n".repeat(1000));
    let r = deliver_test("exit 0", &m, false);
    // Should succeed even though we couldn't write everything
    assert!(r.is_ok());
}

#[test]
fn timeout_kills_hung_command() {
    let m = msg("Subject: Test\n\nBody\n");
    let mut env = Environment::from_process();
    env.set(TIMEOUT.name, "1");

    let start = Instant::now();
    let r = deliver("exec sleep 60", &m, false, true, false, &env);
    let elapsed = start.elapsed().as_secs();

    assert!(elapsed < 4);
    assert_eq!(
        r.unwrap_err(),
        DeliveryError::PipeSignal(nix::libc::SIGTERM)
    );
}

#[test]
fn broken_pipe_tolerance() {
    // `true` closes stdin immediately; large message triggers EPIPE
    let body = "x".repeat(1024 * 1024);
    let m = msg(&format!("Subject: Test\n\n{body}\n"));
    let r = deliver_test("true", &m, false);
    assert!(r.is_ok());
}

#[test]
fn filter_large_message() {
    // Message larger than pipe buffer; deadlocks without poll-based pump.
    let body = "x".repeat(256 * 1024);
    let m = msg(&format!("Subject: Test\n\n{body}\n"));
    let r = deliver_test("cat", &m, true).unwrap();
    assert_eq!(r.output.unwrap(), to_bytes(&m));
}

#[test]
fn spawn_failure() {
    let m = msg("Subject: Test\n\nBody\n");
    let mut env = Environment::from_process();
    env.set(SHELL.name, "/nonexistent");

    let r = deliver("true", &m, false, false, false, &env);
    let Err(DeliveryError::Io { op, .. }) = r else {
        panic!("expected Io error, got {r:?}");
    };
    assert_eq!(op, "spawn");
}
