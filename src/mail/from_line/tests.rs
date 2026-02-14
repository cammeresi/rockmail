use super::*;
use chrono::Utc;

#[test]
fn generate_from_line() {
    let line = generate("user@example.com");
    assert!(line.starts_with(b"From user@example.com  "));
    assert!(line.ends_with(b"\n"));
}

#[test]
fn skip_from() {
    let data = b"From user Mon Jan 1 00:00:00 2024\nSubject: Test\n\nBody";
    let rest = skip_from_lines(data);
    assert!(rest.starts_with(b"Subject:"));
}

#[test]
fn skip_multiple_from() {
    let data =
        b"From user Mon Jan 1 00:00:00 2024\n>From forwarded\nSubject: Test\n";
    let rest = skip_from_lines(data);
    assert!(rest.starts_with(b"Subject:"));
}

#[test]
fn ctime_epoch() {
    let epoch = Utc.timestamp_opt(0, 0).unwrap();
    let line = generate_with_time("test", epoch);
    let s = String::from_utf8_lossy(&line);
    assert_eq!(s, "From test  Thu Jan  1 00:00:00 1970\n");
}

#[test]
#[should_panic(expected = "sender must not be empty")]
fn generate_empty_sender_panics() {
    generate("");
}

#[test]
#[should_panic(expected = "sender must not contain whitespace")]
fn generate_sender_with_space_panics() {
    generate("user name@host");
}

#[test]
#[should_panic(expected = "sender must not contain whitespace")]
fn generate_sender_with_newline_panics() {
    generate("user\n@host");
}

#[test]
fn extract_timestamp_standard() {
    let line = b"From user@host  Mon Jan  1 00:00:00 2024";
    assert_eq!(extract_timestamp(line), Some("Mon Jan  1 00:00:00 2024"));
}

#[test]
fn extract_timestamp_single_space() {
    let line = b"From user@host Mon Jan  1 00:00:00 2024";
    assert_eq!(extract_timestamp(line), Some("Mon Jan  1 00:00:00 2024"));
}

#[test]
fn extract_timestamp_no_prefix() {
    assert_eq!(extract_timestamp(b"Subject: Test"), None);
}

#[test]
fn extract_timestamp_sender_only() {
    assert_eq!(extract_timestamp(b"From user@host"), None);
}

#[test]
fn extract_timestamp_roundtrip() {
    let epoch = Utc.timestamp_opt(0, 0).unwrap();
    let line = generate_with_time("test", epoch);
    // strip trailing newline to match from_line() output
    let ts = extract_timestamp(&line[..line.len() - 1]).unwrap();
    assert_eq!(ts, "Thu Jan  1 00:00:00 1970");
}
