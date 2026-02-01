//! Integration tests for formail binary.

use std::io::Write;
use std::process::{Command, Stdio};

fn formail() -> Command {
    Command::new(env!("CARGO_BIN_EXE_formail"))
}

fn run(args: &[&str], input: &str) -> (String, i32) {
    let mut child = formail()
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn formail");

    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();

    let output = child.wait_with_output().expect("failed to wait");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, code)
}

#[test]
fn basic_passthrough() {
    let input = "From: user@host\nSubject: Test\n\nBody\n";
    let (out, code) = run(&["-f"], input);
    assert_eq!(code, 0);
    assert!(out.contains("From: user@host"));
    assert!(out.contains("Subject: Test"));
    assert!(out.contains("Body"));
}

#[test]
fn adds_from_line() {
    let input = "From: user@host\nSubject: Test\n\nBody\n";
    let (out, code) = run(&[], input);
    assert_eq!(code, 0);
    assert!(out.starts_with("From user@host "));
}

#[test]
fn force_no_from_line() {
    let input = "From: user@host\nSubject: Test\n\nBody\n";
    let (out, code) = run(&["-f"], input);
    assert_eq!(code, 0);
    assert!(out.starts_with("From:"));
}

#[test]
fn extract_field() {
    let input = "From: user@host\nSubject: Hello World\n\nBody\n";
    let (out, code) = run(&["-x", "Subject"], input);
    assert_eq!(code, 0);
    assert!(out.trim().contains("Hello World"));
}

#[test]
fn extract_field_with_name() {
    let input = "From: user@host\nSubject: Hello\n\nBody\n";
    let (out, code) = run(&["-X", "Subject"], input);
    assert_eq!(code, 0);
    assert!(out.contains("Subject:"));
    assert!(out.contains("Hello"));
}

#[test]
fn delete_field() {
    let input = "From: user@host\nReceived: foo\nSubject: Test\n\nBody\n";
    let (out, code) = run(&["-f", "-I", "Received:"], input);
    assert_eq!(code, 0);
    assert!(!out.contains("Received:"));
    assert!(out.contains("Subject: Test"));
}

#[test]
fn add_field_if_not_present() {
    let input = "From: user@host\n\nBody\n";
    let (out, code) = run(&["-f", "-a", "X-Custom: added"], input);
    assert_eq!(code, 0);
    assert!(out.contains("X-Custom: added"));
}

#[test]
fn add_field_not_if_present() {
    let input = "From: user@host\nX-Custom: original\n\nBody\n";
    let (out, code) = run(&["-f", "-a", "X-Custom: added"], input);
    assert_eq!(code, 0);
    assert!(out.contains("X-Custom: original"));
    assert!(!out.contains("X-Custom: added"));
}

#[test]
fn add_field_always() {
    let input = "From: user@host\nX-Custom: original\n\nBody\n";
    let (out, code) = run(&["-f", "-A", "X-Custom: added"], input);
    assert_eq!(code, 0);
    assert!(out.contains("X-Custom: original"));
    assert!(out.contains("X-Custom: added"));
}

#[test]
fn keep_first_unique() {
    let input = "Received: first\nReceived: second\nSubject: Test\n\nBody\n";
    let (out, code) = run(&["-f", "-u", "Received"], input);
    assert_eq!(code, 0);
    assert!(out.contains("Received: first"));
    assert!(!out.contains("Received: second"));
}

#[test]
fn keep_last_unique() {
    let input = "Received: first\nReceived: second\nSubject: Test\n\nBody\n";
    let (out, code) = run(&["-f", "-U", "Received"], input);
    assert_eq!(code, 0);
    assert!(!out.contains("Received: first"));
    assert!(out.contains("Received: second"));
}

#[test]
fn reply_mode() {
    let input = "From: sender@example.com\nSubject: Hello\nMessage-ID: <123@host>\n\nBody\n";
    let (out, code) = run(&["-r", "-t"], input);
    assert_eq!(code, 0);
    assert!(out.contains("To: sender@example.com"));
    assert!(out.contains("Subject: Re: Hello"));
    assert!(out.contains("In-Reply-To: <123@host>"));
}

#[test]
fn reply_no_double_re() {
    let input = "From: sender@example.com\nSubject: Re: Hello\n\nBody\n";
    let (out, code) = run(&["-r", "-t"], input);
    assert_eq!(code, 0);
    assert!(out.contains("Subject: Re: Hello"));
    assert!(!out.contains("Re: Re:"));
}

#[test]
fn split_messages() {
    let input = "From a@a Mon Jan 1 00:00:00 2024\n\
                 From: a@a\nSubject: First\n\nBody1\n\n\
                 From b@b Mon Jan 1 00:00:00 2024\n\
                 From: b@b\nSubject: Second\n\nBody2\n";
    let (out, code) = run(&["-s"], input);
    assert_eq!(code, 0);
    assert!(out.contains("Subject: First"));
    assert!(out.contains("Subject: Second"));
}

#[test]
fn split_skip() {
    let input = "From a@a Mon Jan 1 00:00:00 2024\n\
                 From: a@a\nSubject: First\n\nB1\n\n\
                 From b@b Mon Jan 1 00:00:00 2024\n\
                 From: b@b\nSubject: Second\n\nB2\n";
    let (out, code) = run(&["+1", "-s"], input);
    assert_eq!(code, 0);
    assert!(!out.contains("Subject: First"));
    assert!(out.contains("Subject: Second"));
}

#[test]
fn split_total() {
    let input = "From a@a Mon Jan 1 00:00:00 2024\n\
                 From: a@a\nSubject: First\n\nB1\n\n\
                 From b@b Mon Jan 1 00:00:00 2024\n\
                 From: b@b\nSubject: Second\n\nB2\n\n\
                 From c@c Mon Jan 1 00:00:00 2024\n\
                 From: c@c\nSubject: Third\n\nB3\n";
    let (out, code) = run(&["-1", "-s"], input);
    assert_eq!(code, 0);
    assert!(out.contains("Subject: First"));
    assert!(!out.contains("Subject: Second"));
    assert!(!out.contains("Subject: Third"));
}

#[test]
fn duplicate_detection_new() {
    let cache = tempfile::NamedTempFile::new().unwrap();
    let path = cache.path().to_str().unwrap();

    let input = "From: a@a\nMessage-ID: <unique-test-1@host>\n\nBody\n";
    let (_, code) = run(&["-D", "1000", path], input);
    assert_eq!(code, 1); // not duplicate
}

#[test]
fn duplicate_detection_dup() {
    let cache = tempfile::NamedTempFile::new().unwrap();
    let path = cache.path().to_str().unwrap();

    let input = "From: a@a\nMessage-ID: <unique-test-2@host>\n\nBody\n";
    let (_, code1) = run(&["-D", "1000", path], input);
    assert_eq!(code1, 1); // not duplicate

    let (_, code2) = run(&["-D", "1000", path], input);
    assert_eq!(code2, 0); // duplicate
}

#[test]
fn concatenate_headers() {
    let input =
        "From: user@host\nSubject: This is\n a long\n subject\n\nBody\n";
    let (out, code) = run(&["-f", "-c", "-X", "Subject"], input);
    assert_eq!(code, 0);
    // After concatenation, newlines in value become spaces
    assert!(out.contains("Subject:"));
    assert!(!out.contains("\n a long"));
}

#[test]
fn zap_adds_space() {
    let input = "From: user@host\nX-NoSpace:value\n\nBody\n";
    let (out, code) = run(&["-f", "-z"], input);
    assert_eq!(code, 0);
    assert!(out.contains("X-NoSpace: value"));
}

#[test]
fn zap_removes_empty() {
    let input = "From: user@host\nX-Empty:   \nSubject: Test\n\nBody\n";
    let (out, code) = run(&["-f", "-z"], input);
    assert_eq!(code, 0);
    assert!(!out.contains("X-Empty:"));
    assert!(out.contains("Subject: Test"));
}

#[test]
fn log_summary() {
    let input = "From sender@host Mon Jan 1 00:00:00 2024\nFrom: sender@host\nSubject: Test Subject\n\nBody here\n";
    let (out, code) = run(&["-l", "inbox"], input);
    assert_eq!(code, 0);
    // Should output From_ line, subject, and folder summary
    assert!(out.contains("From sender@host"));
    assert!(out.contains("Test Subject"));
    assert!(out.contains("Folder: inbox"));
}

#[test]
fn duplicate_cache_circular() {
    // Test that the cache wraps around when full
    let cache = tempfile::NamedTempFile::new().unwrap();
    let path = cache.path().to_str().unwrap();

    // Use a small cache (50 bytes)
    // First message ID is ~20 bytes
    let input1 = "From: a@a\nMessage-ID: <id-circular-1@h>\n\nBody\n";
    let (_, code1) = run(&["-D", "50", path], input1);
    assert_eq!(code1, 1); // not duplicate

    // Second message ID is also ~20 bytes, should fit
    let input2 = "From: a@a\nMessage-ID: <id-circular-2@h>\n\nBody\n";
    let (_, code2) = run(&["-D", "50", path], input2);
    assert_eq!(code2, 1); // not duplicate

    // Third message should wrap, overwriting first
    let input3 = "From: a@a\nMessage-ID: <id-circular-3@h>\n\nBody\n";
    let (_, code3) = run(&["-D", "50", path], input3);
    assert_eq!(code3, 1); // not duplicate

    // First message should no longer be detected as duplicate
    let (_, code4) = run(&["-D", "50", path], input1);
    assert_eq!(code4, 1); // not duplicate (was overwritten)
}
