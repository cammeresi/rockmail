use super::*;

fn to_bytes(msg: &Message) -> Vec<u8> {
    let mut buf = Vec::new();
    msg.write_to(&mut buf, false).expect("Vec write");
    buf
}

#[test]
fn null_bytes_in_header() {
    let msg = Message::parse(b"Subject: Test\x00null\n\nBody");
    let s = msg.get_header("Subject").unwrap();
    // Null is valid UTF-8; passes through unchanged
    assert!(s.contains('\0'));
}

#[test]
fn long_header_line() {
    let v = "x".repeat(2000);
    let raw = format!("Subject: {v}\n\nBody");
    let msg = Message::parse(raw.as_bytes());
    assert_eq!(msg.get_header("Subject").unwrap().len(), 2000);
}

#[test]
fn header_empty_value() {
    let msg = Message::parse(b"Subject:\n\nBody");
    assert_eq!(msg.get_header("Subject").unwrap().as_ref(), "");
}

#[test]
fn utf8_invalid_sequences() {
    let msg = Message::parse(b"Subject: \xFF\xFE invalid\n\nBody");
    let s = msg.get_header("Subject").unwrap();
    assert!(s.contains('\u{FFFD}'));
}

#[test]
fn crlf_in_continuation() {
    let msg = Message::parse(b"Subject: Line1\r\n continuing\n\nBody");
    let s = msg.get_header("Subject").unwrap();
    assert_eq!(s.as_ref(), "Line1 continuing");
}

#[test]
fn content_length_mismatch() {
    let msg = Message::parse(b"Content-Length: 5\n\nThis is longer");
    assert_eq!(msg.content_length(), Some(5));
    assert_eq!(msg.body().len(), 14);
}

#[test]
fn empty_from_line() {
    let msg = Message::parse(b"From \nSubject: Test\n\nBody");
    assert_eq!(msg.envelope_sender(), Some(""));
}

#[test]
fn from_sender_no_timestamp() {
    let msg = Message::parse(b"From sender\nSubject: Test\n\nBody");
    assert_eq!(msg.envelope_sender(), Some("sender"));
    assert_eq!(msg.envelope_timestamp(), None);
}

#[test]
fn skip_escaped_from_line() {
    let msg = Message::parse(
        b"From user@host Mon Jan 1 00:00:00 2024\n>From forwarded\nSubject: Test\n\nBody",
    );
    let hdrs: Vec<_> = msg.headers().collect();
    assert_eq!(hdrs.len(), 1);
    assert_eq!(hdrs[0].0.as_ref(), "Subject");
}

#[test]
fn parse_simple_message() {
    let msg =
        Message::parse(b"From: test@example.com\nSubject: Hello\n\nBody text");
    assert_eq!(msg.header(), b"From: test@example.com\nSubject: Hello\n");
    assert_eq!(msg.body(), b"Body text");
}

#[test]
fn parse_no_body() {
    let msg = Message::parse(b"From: test@example.com\nSubject: Hello\n");
    assert_eq!(msg.header(), b"From: test@example.com\nSubject: Hello\n");
    assert_eq!(msg.body(), b"");
}

#[test]
fn parse_empty_body() {
    let msg = Message::parse(b"Subject: Test\n\n");
    assert_eq!(msg.header(), b"Subject: Test\n");
    assert_eq!(msg.body(), b"");
}

#[test]
fn skip_leading_newlines() {
    let msg = Message::parse(b"\n\n\nFrom: test\n\nBody");
    assert_eq!(msg.header(), b"From: test\n");
    assert_eq!(msg.body(), b"Body");
}

#[test]
fn header_continuation() {
    let msg = Message::parse(
        b"Subject: This is a\n very long\n\tsubject line\n\nBody",
    );
    let subj = msg.get_header("Subject").unwrap();
    assert_eq!(subj.as_ref(), "This is a very long subject line");
}

#[test]
fn header_value_trimmed() {
    let msg = Message::parse(b"Subject: Hello\n\n");
    let subj = msg.get_header("Subject").unwrap();
    assert_eq!(subj.as_ref(), "Hello"); // no leading space
}

#[test]
fn case_insensitive_header_lookup() {
    let msg = Message::parse(b"Content-Type: text/plain\n\n");
    assert!(msg.get_header("content-type").is_some());
    assert!(msg.get_header("CONTENT-TYPE").is_some());
    assert!(msg.get_header("Content-type").is_some());
}

#[test]
fn content_length_header() {
    let msg = Message::parse(b"Content-Length: 42\n\nBody here");
    assert_eq!(msg.content_length(), Some(42));
}

#[test]
fn content_length_missing() {
    let msg = Message::parse(b"Subject: Test\n\nBody");
    assert_eq!(msg.content_length(), None);
}

#[test]
fn from_parts() {
    let msg = Message::from_parts(b"Subject: Test\n", b"Body text");
    assert_eq!(msg.header(), b"Subject: Test\n");
    assert_eq!(msg.body(), b"Body text");
}

#[test]
fn headers_iterator() {
    let msg = Message::parse(b"From: a@b\nTo: c@d\nSubject: Hi\n\nBody");
    let headers: Vec<_> = msg.headers().collect();
    assert_eq!(headers.len(), 3);
    assert_eq!(headers[0].0.as_ref(), "From");
    assert_eq!(headers[1].0.as_ref(), "To");
    assert_eq!(headers[2].0.as_ref(), "Subject");
}

#[test]
fn skip_from_line() {
    let msg = Message::parse(
        b"From user@host Mon Jan 1 00:00:00 2024\nSubject: Test\n\nBody",
    );
    let headers: Vec<_> = msg.headers().collect();
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].0.as_ref(), "Subject");
}

#[test]
fn empty_message() {
    let msg = Message::parse(b"");
    assert!(msg.is_empty());
    assert!(msg.header().is_empty());
    assert!(msg.body().is_empty());
}

#[test]
fn only_body_no_headers() {
    let msg = Message::parse(b"\nJust body text");
    // Leading newline skipped, no blank line found — raw data has no
    // valid fields so header is empty, body is empty (no \n\n boundary).
    assert!(msg.header().is_empty());
    assert_eq!(msg.body(), b"");
}

#[test]
fn multiple_blank_lines() {
    let msg = Message::parse(b"Subject: Test\n\n\nBody with extra blank");
    assert_eq!(msg.header(), b"Subject: Test\n");
    // Body includes the extra blank line
    assert_eq!(msg.body(), b"\nBody with extra blank");
}

#[test]
fn from_line_extraction() {
    let msg = Message::parse(
        b"From user@host Mon Jan 1 00:00:00 2024\nSubject: Test\n\nBody",
    );
    let line = msg.from_line().unwrap();
    assert_eq!(line, b"From user@host Mon Jan 1 00:00:00 2024");
}

#[test]
fn from_line_missing() {
    let msg = Message::parse(b"Subject: Test\n\nBody");
    assert!(msg.from_line().is_none());
}

#[test]
fn envelope_sender_extraction() {
    let msg = Message::parse(
        b"From user@host Mon Jan 1 00:00:00 2024\nSubject: Test\n\nBody",
    );
    assert_eq!(msg.envelope_sender(), Some("user@host"));
}

#[test]
fn crlf_normalized() {
    let msg = Message::parse(b"Subject: Test\r\n\r\nBody");
    assert_eq!(msg.header(), b"Subject: Test\n");
    assert_eq!(msg.body(), b"Body");
}

#[test]
fn crlf_headers() {
    let msg = Message::parse(b"From: a@b\r\nTo: c@d\r\n\r\nBody");
    let headers: Vec<_> = msg.headers().collect();
    assert_eq!(headers.len(), 2);
    assert_eq!(headers[0].0.as_ref(), "From");
    assert_eq!(headers[1].0.as_ref(), "To");
}

#[test]
fn from_parts_has_separator() {
    let msg = Message::from_parts(b"Subject: Test\n", b"Body");
    assert_eq!(to_bytes(&msg), b"Subject: Test\n\nBody");
}

#[test]
fn many_from_lines_no_stack_overflow() {
    let mut data = Vec::new();
    for i in 0..1000 {
        data.extend_from_slice(format!("From user{}\n", i).as_bytes());
    }
    data.extend_from_slice(b"Subject: Test\n\nBody");
    let msg = Message::parse(&data);
    let headers: Vec<_> = msg.headers().collect();
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].0.as_ref(), "Subject");
}

#[test]
fn malformed_header_skipped() {
    let msg = Message::parse(b"Not-A-Header\nSubject: Test\n\nBody");
    let headers: Vec<_> = msg.headers().collect();
    assert_eq!(headers.len(), 1);
    assert_eq!(headers[0].0.as_ref(), "Subject");
}

#[test]
fn set_envelope_sender_no_existing() {
    let mut msg = Message::parse(b"Subject: Test\n\nBody");
    msg.set_envelope_sender("alice@host");
    assert_eq!(msg.envelope_sender(), Some("alice@host"));
    assert!(msg.get_header("Subject").is_some());
    assert_eq!(msg.body(), b"Body");
}

#[test]
fn set_envelope_sender_replaces() {
    let mut msg = Message::parse(
        b"From old@host  Thu Jan  1 00:00:00 1970\nSubject: Test\n\nBody",
    );
    msg.set_envelope_sender("new@host");
    assert_eq!(msg.envelope_sender(), Some("new@host"));
    let bytes = to_bytes(&msg);
    assert!(!bytes.windows(8).any(|w| w == b"old@host"));
    assert_eq!(msg.body(), b"Body");
}

#[test]
fn set_envelope_sender_long() {
    let sender = "a".repeat(500) + "@host";
    let mut msg = Message::parse(b"Subject: Test\n\nBody");
    msg.set_envelope_sender(&sender);
    assert_eq!(msg.envelope_sender(), Some(sender.as_str()));
}

#[test]
fn strip_from_line_removes() {
    let mut msg = Message::parse(
        b"From user@host  Thu Jan  1 00:00:00 1970\nSubject: Test\n\nBody",
    );
    msg.strip_from_line();
    assert!(msg.from_line().is_none());
    assert_eq!(msg.get_header("Subject").unwrap().as_ref(), "Test");
    assert_eq!(msg.body(), b"Body");
}

#[test]
fn strip_from_line_noop() {
    let msg_a = Message::parse(b"Subject: Test\n\nBody");
    let mut msg_b = msg_a.clone();
    msg_b.strip_from_line();
    assert_eq!(to_bytes(&msg_a), to_bytes(&msg_b));
}

#[test]
fn set_then_strip_roundtrip() {
    let orig = Message::parse(b"Subject: Test\n\nBody");
    let mut msg = orig.clone();
    msg.set_envelope_sender("user@host");
    msg.strip_from_line();
    assert_eq!(to_bytes(&orig), to_bytes(&msg));
}

#[test]
fn envelope_timestamp_extraction() {
    let msg = Message::parse(
        b"From user@host  Mon Jan  1 00:00:00 2024\nSubject: Test\n\nBody",
    );
    assert_eq!(msg.envelope_timestamp(), Some("Mon Jan  1 00:00:00 2024"));
}

#[test]
fn envelope_timestamp_missing() {
    let msg = Message::parse(b"Subject: Test\n\nBody");
    assert_eq!(msg.envelope_timestamp(), None);
}

#[test]
fn refresh_envelope_sender_preserves_timestamp() {
    let mut msg = Message::parse(
        b"From old@host  Mon Jan  1 00:00:00 2024\nSubject: Test\n\nBody",
    );
    msg.refresh_envelope_sender("new@host");
    assert_eq!(msg.envelope_sender(), Some("new@host"));
    assert_eq!(msg.envelope_timestamp(), Some("Mon Jan  1 00:00:00 2024"));
    assert_eq!(msg.body(), b"Body");
}

#[test]
fn refresh_envelope_sender_no_existing() {
    let mut msg = Message::parse(b"Subject: Test\n\nBody");
    msg.refresh_envelope_sender("user@host");
    assert_eq!(msg.envelope_sender(), Some("user@host"));
    assert!(msg.envelope_timestamp().is_some());
    assert_eq!(msg.body(), b"Body");
}

#[test]
fn fields_direct_access() {
    let msg = Message::parse(b"Subject: Test\nFrom: a@b\n\nBody");
    assert_eq!(msg.fields().len(), 2);
    assert_eq!(msg.fields().byte_len(), b"Subject: Test\nFrom: a@b\n".len());
}

#[test]
fn fields_mut_modify() {
    let mut msg = Message::parse(b"Subject: old\nFrom: a@b\n\nBody");
    msg.fields_mut().remove_all(b"Subject");
    assert!(msg.get_header("Subject").is_none());
    assert_eq!(msg.header(), b"From: a@b\n");
}

fn to_bytes_stripped(msg: &Message) -> Vec<u8> {
    let mut buf = Vec::new();
    msg.write_to(&mut buf, true).expect("Vec write");
    buf
}

#[test]
fn write_to_roundtrip() {
    let msg = Message::parse(b"Subject: Test\n\nBody text");
    assert_eq!(to_bytes(&msg), b"Subject: Test\n\nBody text");
}

#[test]
fn write_to_strip_from() {
    let msg = Message::parse(
        b"From user@host  Thu Jan  1 00:00:00 1970\nSubject: Test\n\nBody",
    );
    let mut expected = msg.clone();
    expected.strip_from_line();
    assert_eq!(to_bytes_stripped(&msg), to_bytes(&expected));
}

#[test]
fn write_to_strip_from_no_from_line() {
    let msg = Message::parse(b"Subject: Test\n\nBody");
    assert_eq!(to_bytes_stripped(&msg), to_bytes(&msg));
}

#[test]
fn write_to_strip_from_only_from_line() {
    let msg = Message::parse(b"From user@host  Thu Jan  1 00:00:00 1970\n\nBody");
    let out = to_bytes_stripped(&msg);
    assert_eq!(out, b"Body");
}

#[test]
fn write_to_strip_from_return_value() {
    let msg = Message::parse(
        b"From user@host  Thu Jan  1 00:00:00 1970\nSubject: Test\n\nBody",
    );
    let mut buf = Vec::new();
    let n = msg.write_to(&mut buf, true).expect("Vec write");
    assert_eq!(n, buf.len());
    let full = to_bytes(&msg).len();
    assert!(n < full);
}

#[test]
fn ends_with_empty_body_with_headers() {
    let msg = Message::parse(b"Subject: Test\n\n");
    assert!(msg.ends_with_newline());
    assert!(msg.ends_with_blank_line());
}

#[test]
fn ends_with_body_single_newline_with_headers() {
    let msg = Message::parse(b"Subject: Test\n\n\n");
    assert!(msg.ends_with_newline());
    assert!(msg.ends_with_blank_line());
}

#[test]
fn ends_with_completely_empty() {
    let msg = Message::parse(b"");
    assert!(!msg.ends_with_newline());
    assert!(!msg.ends_with_blank_line());
}
