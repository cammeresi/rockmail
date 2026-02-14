use std::io::Read;

use tempfile::NamedTempFile;

use rockmail::field::read_headers;

use super::*;

#[test]
fn parse_skip_total_skip_only() {
    let args = vec!["formail".into(), "+5".into(), "-a".into(), "X:".into()];
    let (skip, total, rest) = parse_skip_total(&args);
    assert_eq!(skip, Some(5));
    assert_eq!(total, None);
    assert_eq!(rest, vec!["formail", "-a", "X:"]);
}

#[test]
fn parse_skip_total_total_only() {
    let args = vec!["formail".into(), "-10".into(), "-f".into()];
    let (skip, total, rest) = parse_skip_total(&args);
    assert_eq!(skip, None);
    assert_eq!(total, Some(10));
    assert_eq!(rest, vec!["formail", "-f"]);
}

#[test]
fn parse_skip_total_both() {
    let args = vec!["formail".into(), "+1".into(), "-5".into(), "-s".into()];
    let (skip, total, rest) = parse_skip_total(&args);
    assert_eq!(skip, Some(1));
    assert_eq!(total, Some(5));
    assert_eq!(rest, vec!["formail", "-s"]);
}

#[test]
fn parse_header_arg_with_value() {
    let (name, value) = parse_header_arg("Subject: Test");
    assert_eq!(name, "Subject");
    assert_eq!(value, "Test");
}

#[test]
fn parse_header_arg_name_only() {
    let (name, value) = parse_header_arg("Received:");
    assert_eq!(name, "Received");
    assert_eq!(value, "");
}

#[test]
fn extract_address_angle() {
    assert_eq!(
        extract_address("John <john@example.com>"),
        "john@example.com"
    );
}

#[test]
fn extract_address_bare() {
    assert_eq!(extract_address("john@example.com"), "john@example.com");
}

#[test]
fn extract_address_rfc822() {
    // No closing bracket → falls back to first word
    assert_eq!(extract_address("John <broken"), "John");
    // Comments are skipped
    assert_eq!(extract_address("(comment) user@host.com"), "user@host.com");
    // Angle brackets preferred
    assert_eq!(
        extract_address("John > Smith <user@host.com>"),
        "user@host.com"
    );
}

#[test]
fn generate_message_id_format() {
    let id = generate_message_id();
    assert!(id.starts_with('<'));
    assert!(id.ends_with('>'));
    assert!(id.contains('@'));
}

#[test]
fn is_header_field_valid() {
    assert!(is_header_field(b"Subject: Test\n"));
    assert!(is_header_field(b"From foo@bar Mon Jan 1\n"));
    assert!(is_header_field(b"X-Custom: value\n"));
}

#[test]
fn is_header_field_invalid() {
    assert!(!is_header_field(b"Not a header\n"));
    assert!(!is_header_field(b" continuation\n"));
    assert!(!is_header_field(b"\n"));
}

#[test]
fn reply_to_field() {
    let mut fields = FieldList::new();
    fields.push(Field::from_parts(b"From:", b"sender@example.com"));
    fields.push(Field::from_parts(b"Subject:", b"Test"));
    fields.push(Field::from_parts(b"Message-ID:", b"<123@example.com>"));

    let args = Args {
        trust: true,
        reply: true,
        ..Args::default()
    };
    let reply = generate_reply(&args, &fields);

    assert!(reply.find(b"To").is_some());
    assert!(reply.find(b"Subject").is_some());
    assert!(reply.find(b"In-Reply-To").is_some());
}

#[test]
fn reply_subject_adds_re() {
    let mut fields = FieldList::new();
    fields.push(Field::from_parts(b"From:", b"sender@example.com"));
    fields.push(Field::from_parts(b"Subject:", b"Hello"));

    let args = Args {
        trust: true,
        reply: true,
        ..Args::default()
    };
    let reply = generate_reply(&args, &fields);

    let subj = reply.find(b"Subject").unwrap();
    assert!(subj.value().starts_with(b" Re:"));
}

#[test]
fn reply_subject_always_adds_re() {
    // procmail always adds Re:, even if already present
    let mut fields = FieldList::new();
    fields.push(Field::from_parts(b"From:", b"sender@example.com"));
    fields.push(Field::from_parts(b"Subject:", b"Re: Hello"));

    let args = Args {
        trust: true,
        reply: true,
        ..Args::default()
    };
    let reply = generate_reply(&args, &fields);

    let subj = reply.find(b"Subject").unwrap();
    let val = std::str::from_utf8(subj.value()).unwrap();
    assert!(val.contains("Re: Re: Hello"));
}

#[test]
fn sender_reply_mode() {
    let mut fields = FieldList::new();
    fields.push(Field::from_parts(b"Reply-To:", b"reply@example.com"));
    fields.push(Field::from_parts(b"From:", b"from@example.com"));

    let addr = get_sender(&fields, SenderMode::Reply).unwrap();
    assert_eq!(addr, "reply@example.com");
}

#[test]
fn sender_envelope_mode() {
    let mut fields = FieldList::new();
    fields.push(Field::from_parts(b"Return-Path:", b"<bounce@example.com>"));
    fields.push(Field::from_parts(b"Reply-To:", b"reply@example.com"));

    let addr = get_sender(&fields, SenderMode::Envelope).unwrap();
    assert_eq!(addr, "bounce@example.com");
}

#[test]
fn extract_address_nested_comment() {
    assert_eq!(
        extract_address("(outer (inner)) user@host.com"),
        "user@host.com"
    );
}

#[test]
fn extract_address_escaped_in_comment() {
    // Backslash-escaped paren inside comment
    assert_eq!(
        extract_address("(comment \\) still comment) addr@x.com"),
        "addr@x.com"
    );
}

#[test]
fn extract_address_quoted_string() {
    assert_eq!(
        extract_address("\"John Doe\"@example.com rest"),
        "\"John Doe\"@example.com"
    );
}

#[test]
fn extract_address_only_comments() {
    assert_eq!(extract_address("(nothing here)"), "");
}

#[test]
fn extract_address_empty() {
    assert_eq!(extract_address(""), "");
    assert_eq!(extract_address("   "), "");
}

#[test]
fn penalty_good_address() {
    assert_eq!(addr_penalty("user@host.example.com"), 0);
}

#[test]
fn penalty_bare_user() {
    // No @, !, or /
    assert_eq!(addr_penalty("justuser"), 12 * 4);
}

#[test]
fn penalty_uucp() {
    assert_eq!(addr_penalty("host.UUCP!user"), 12 * 3);
}

#[test]
fn penalty_no_dot() {
    // Has @ but no dot
    assert_eq!(addr_penalty("user@host"), 12 * 2);
}

#[test]
fn penalty_bangpath() {
    assert_eq!(addr_penalty("host!user"), 12);
}

#[test]
fn from_line_simple_at() {
    let (addr, pen) = from_line_addr("user@host.com Mon Jan 1 00:00:00 2000");
    assert_eq!(addr, "user@host.com");
    assert_eq!(pen, 0);
}

#[test]
fn from_line_uucp_bangpath() {
    let raw = "user Mon Jan 1 00:00:00 2000 remote from gateway";
    let (addr, pen) = from_line_addr(raw);
    assert_eq!(addr, "gateway!user");
    assert_eq!(pen, 0);
}

#[test]
fn from_line_multi_hop() {
    let raw = "localuser Mon Jan  1 00:00:00 2000 remote from hop1\n\
               >From localuser Mon Jan  1 00:00:00 2000 remote from hop2";
    let (addr, pen) = from_line_addr(raw);
    // Multi-line → penalty 2; hops reconstructed
    assert_eq!(pen, 2);
    assert!(addr.contains("hop1!"));
    assert!(addr.contains("hop2!"));
    assert!(addr.ends_with("localuser"));
}

#[test]
fn from_line_forwarded_by() {
    let raw = "user Mon Jan  1 00:00:00 2000 forwarded by relay";
    let (addr, _) = from_line_addr(raw);
    assert_eq!(addr, "relay!user");
}

#[test]
fn from_line_empty() {
    let (addr, pen) = from_line_addr("");
    assert_eq!(addr, "");
    assert_eq!(pen, 0);
}

#[test]
fn sender_prefers_higher_weight() {
    // Return-Path (idx=9) beats Path (idx=6) in envelope mode
    let mut fields = FieldList::new();
    fields.push(Field::from_parts(b"Path:", b"news!host"));
    fields.push(Field::from_parts(b"Return-Path:", b"<bounce@example.com>"));

    let addr = get_sender(&fields, SenderMode::Envelope).unwrap();
    assert_eq!(addr, "bounce@example.com");
}

#[test]
fn sender_quality_can_override_position() {
    // From: (idx=1) with good address should beat Reply-To: (idx=0) with bare
    // user, because the penalty for bare user (48) exceeds the index difference
    let mut fields = FieldList::new();
    fields.push(Field::from_parts(b"Reply-To:", b"bareuser"));
    fields.push(Field::from_parts(b"From:", b"good@host.example.com"));

    let addr = get_sender(&fields, SenderMode::Envelope).unwrap();
    assert_eq!(addr, "good@host.example.com");
}

#[test]
fn sender_empty_return_path() {
    // Empty Return-Path → "<>" with override weight
    let mut fields = FieldList::new();
    fields.push(Field::from_parts(b"Return-Path:", b"<>"));
    fields.push(Field::from_parts(b"From:", b"user@example.com"));

    let addr = get_sender(&fields, SenderMode::Envelope).unwrap();
    assert_eq!(addr, "<>");
}

#[test]
fn sender_from_line_only_if_first() {
    // From_ not first → ignored
    let mut fields = FieldList::new();
    fields.push(Field::from_parts(b"From:", b"header@example.com"));
    fields.push(
        Field::new(
            b"From envelope@example.com Mon Jan  1 00:00:00 2000\n".to_vec(),
        )
        .unwrap(),
    );

    let addr = get_sender(&fields, SenderMode::Envelope).unwrap();
    assert_eq!(addr, "header@example.com");
}

#[test]
fn sender_from_line_when_first() {
    let mut fields = FieldList::new();
    fields.push(
        Field::new(
            b"From envelope@example.com Mon Jan  1 00:00:00 2000\n".to_vec(),
        )
        .unwrap(),
    );
    fields.push(Field::from_parts(b"Sender:", b"other@example.com"));

    // From_ (idx=10) beats Sender (idx=2) in envelope mode
    let addr = get_sender(&fields, SenderMode::Envelope).unwrap();
    assert_eq!(addr, "envelope@example.com");
}

#[test]
fn sender_ignores_unrelated_headers() {
    let mut fields = FieldList::new();
    fields.push(Field::from_parts(b"Subject:", b"hello"));
    fields.push(Field::from_parts(b"Date:", b"Mon, 1 Jan 2000"));
    assert!(get_sender(&fields, SenderMode::Envelope).is_none());
}

#[test]
fn sender_reply_weights() {
    // In reply mode, Reply-To (wrepl=8) beats Return-Path (wrepl=5)
    let mut fields = FieldList::new();
    fields.push(Field::from_parts(b"Return-Path:", b"<bounce@example.com>"));
    fields.push(Field::from_parts(b"Reply-To:", b"reply@example.com"));

    let addr = get_sender(&fields, SenderMode::Reply).unwrap();
    assert_eq!(addr, "reply@example.com");
}

#[test]
fn duplicate_cache_circular_buffer() {
    let cache = NamedTempFile::new().unwrap();
    let path = cache.path().to_str().unwrap().to_string();

    // Helper to run check_duplicate
    let check = |msgid: &str, maxlen: usize| -> bool {
        let mut fields = FieldList::new();
        fields.push(Field::from_parts(b"Message-ID:", msgid.as_bytes()));
        let args = Args::default();
        check_duplicate(&args, &fields, &path, maxlen).unwrap()
    };

    // Helper to read cache contents
    let read_cache = || -> Vec<u8> {
        let mut f = std::fs::File::open(&path).unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        buf
    };

    // First entry: <id1> (5 bytes) + null + end marker = 7 bytes
    assert!(!check("<id1>", 20));
    assert_eq!(&read_cache(), b"<id1>\0\0");

    // Second entry: adds 6 bytes -> total 13
    assert!(!check("<id2>", 20));
    assert_eq!(&read_cache(), b"<id1>\0<id2>\0\0");

    // Third entry: would be 19 bytes, still fits
    assert!(!check("<id3>", 20));
    assert_eq!(&read_cache(), b"<id1>\0<id2>\0<id3>\0\0");

    // Fourth entry: would exceed 20, wraps to start
    assert!(!check("<id4>", 20));
    let contents = read_cache();
    assert!(contents.starts_with(b"<id4>\0\0"));

    // id1 should no longer be detected (was overwritten)
    assert!(!check("<id1>", 20));

    // id4 should still be detected as duplicate
    assert!(check("<id4>", 20));
}

#[test]
fn non_utf8_header_log_summary() {
    // ISO-8859-1 subject with accented chars
    let mut fields = FieldList::new();
    fields.push(
        Field::new(b"From test@example.com Mon Jan 1 00:00:00 2000\n".to_vec())
            .unwrap(),
    );
    fields.push(Field::from_parts(b"Subject:", b"Caf\xe9 \xe0 Paris"));

    let mut out = Vec::new();
    output_log_summary("/inbox", &fields, 100, &mut out).unwrap();
    let s = String::from_utf8_lossy(&out);
    assert!(s.contains("Caf"));
    assert!(s.contains("Folder: /inbox"));
}

#[test]
fn empty_input() {
    let (fields, body) = read_headers(&b""[..]).unwrap();
    assert!(fields.is_empty());
    assert!(body.is_empty());
}

#[test]
fn binary_body() {
    let header = b"Subject: Test\n\n";
    let binary: Vec<u8> = (0u8..=255).collect();
    let mut input = header.to_vec();
    input.extend(&binary);

    let (fields, body) = read_headers(&input[..]).unwrap();
    assert_eq!(fields.len(), 1);
    assert_eq!(body.len(), 256);
    assert_eq!(body, binary);
}

#[test]
fn duplicate_maxlen_zero() {
    let cache = NamedTempFile::new().unwrap();
    let path = cache.path().to_str().unwrap().to_string();

    let mut fields = FieldList::new();
    fields.push(Field::from_parts(b"Message-ID:", b"<test@example>"));
    let args = Args::default();

    // With maxlen=0, cache is effectively disabled
    let result = check_duplicate(&args, &fields, &path, 0);
    assert!(result.is_ok());
}

#[test]
fn output_body_custom_prefix() {
    let body = b"Hello\nFrom me\nFrom the start\nGoodbye\n";
    let mut out = Vec::new();
    output_body(&body[..], &mut &[][..], &mut out, Quote::From, "|").unwrap();
    assert_eq!(out, b"Hello\n|From me\n|From the start\nGoodbye\n");
}

#[test]
fn duplicate_cache_many_messages() {
    let cache = NamedTempFile::new().unwrap();
    let path = cache.path().to_str().unwrap().to_string();

    let check = |msgid: &str, maxlen| -> bool {
        let mut fields = FieldList::new();
        fields.push(Field::from_parts(b"Message-ID:", msgid.as_bytes()));
        let args = Args::default();
        check_duplicate(&args, &fields, &path, maxlen).unwrap()
    };

    // Each ID is 12 chars + null + end marker = 14 bytes
    // Use cache large enough for ~8 IDs
    let maxlen = 120;

    // Insert several unique messages
    for i in 0..8 {
        let id = format!("<msg{:03}@x>", i);
        let dup = check(&id, maxlen);
        assert!(!dup, "msg {} should not be duplicate on first insert", i);
    }

    // All inserted messages should be detected as duplicates
    for i in 0..8 {
        let id = format!("<msg{:03}@x>", i);
        let dup = check(&id, maxlen);
        assert!(dup, "msg {} should be duplicate", i);
    }

    // Overflow the cache - this should wrap and evict earlier entries
    for i in 8..16 {
        let id = format!("<msg{:03}@x>", i);
        let dup = check(&id, maxlen);
        assert!(!dup, "msg {} should not be duplicate on first insert", i);
    }

    // Early messages should have been evicted by wrap-around
    let id = format!("<msg{:03}@x>", 0);
    let dup = check(&id, maxlen);
    assert!(!dup, "msg 0 should have been evicted");

    // Recent messages should still be present
    let id = format!("<msg{:03}@x>", 15);
    let dup = check(&id, maxlen);
    assert!(dup, "msg 15 should still be in cache");
}
