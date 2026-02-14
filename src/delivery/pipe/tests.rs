use std::time::Instant;

use super::*;
use crate::delivery::tests::msg;
use crate::variables::VAR_TIMEOUT;

#[test]
fn pipe_to_cat() {
    let m = msg("Subject: Test\n\nBody content\n");
    let r = deliver_test("cat > /dev/null", &m, false).unwrap();
    assert_eq!(r.bytes, m.as_bytes().len());
}

#[test]
fn filter_mode() {
    let m = msg("Subject: Test\n\nHello\n");
    let r = deliver_test("cat", &m, true).unwrap();

    let output = r.output.unwrap();
    assert_eq!(output, m.as_bytes());
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

    match r {
        Err(DeliveryError::PipeExit(1)) => {}
        other => panic!("expected PipeExit(1), got {:?}", other),
    }
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
    env.set(VAR_TIMEOUT, "1");

    let start = Instant::now();
    let r = deliver("exec sleep 60", &m, false, true, false, &env);
    let elapsed = start.elapsed().as_secs();

    assert!(elapsed < 4);
    assert!(matches!(r, Err(DeliveryError::PipeSignal(_))));
}
