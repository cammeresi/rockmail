use super::*;

fn msg(s: &str) -> Message {
    Message::parse(s.as_bytes())
}

#[test]
fn pipe_to_cat() {
    let m = msg("Subject: Test\n\nBody content\n");
    let r = deliver("cat > /dev/null", &m, false).unwrap();
    assert_eq!(r.bytes, m.as_bytes().len());
}

#[test]
fn filter_mode() {
    let m = msg("Subject: Test\n\nHello\n");
    let r = deliver("cat", &m, true).unwrap();

    let output = r.output.unwrap();
    assert_eq!(output, m.as_bytes());
}

#[test]
fn filter_transforms() {
    let m = msg("Subject: Test\n\nHello\n");
    let r = deliver("tr a-z A-Z", &m, true).unwrap();

    let output = r.output.unwrap();
    let s = String::from_utf8_lossy(&output);
    assert!(s.contains("SUBJECT: TEST"));
    assert!(s.contains("HELLO"));
}

#[test]
fn exit_code_error() {
    let m = msg("Subject: Test\n\nBody\n");
    let r = deliver("exit 1", &m, false);

    match r {
        Err(DeliveryError::PipeExit(1)) => {}
        other => panic!("expected PipeExit(1), got {:?}", other),
    }
}

#[test]
fn early_exit() {
    // Command that reads nothing and exits
    let m = msg(&"Subject: Test\n\n".repeat(1000));
    let r = deliver("exit 0", &m, false);
    // Should succeed even though we couldn't write everything
    assert!(r.is_ok());
}
