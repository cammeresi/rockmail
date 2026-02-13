//! Gold standard tests comparing Rust formail against procmail's formail.
//!
//! Run with:
//!     cargo test --features gold --test formail_gold
//!
//! The original formail binary is found automatically.  To override,
//! set PROCMAIL_FORMAIL to the path of the original formail.
//!
//! Many of these tests require running procmail with -f lest the two
//! procmails generate new "From " lines with differing timestamps.
//! Otherwise timestamps need to be ignored when comparing output.

#![cfg(feature = "gold")]

use std::iter;

use common::{Gold, GoldResult, normalize_from_line, normalize_message_id};

#[allow(unused)]
mod common;

fn procmail_formail() -> String {
    common::find_gold("PROCMAIL_FORMAIL", "formail")
}

fn rust_formail() -> &'static str {
    env!("CARGO_BIN_EXE_formail")
}

fn run_once(args: &[&str], input: &[u8]) -> GoldResult {
    Gold::run_once(rust_formail(), &procmail_formail(), args, input)
}

fn gold() -> Gold {
    Gold::new()
}

fn run(g: &Gold, args: &[&str], input: &[u8]) -> GoldResult {
    g.run(rust_formail(), &procmail_formail(), args, input)
}

macro_rules! gold {
    ($name:ident, $args:expr, $input:expr) => {
        #[test]
        fn $name() {
            run_once($args, $input).assert_eq();
        }
    };
    ($name:ident, $args:expr, $input:expr, $norm:expr) => {
        #[test]
        fn $name() {
            run_once($args, $input).assert_eq_with($norm);
        }
    };
}

// Tier 1: Exact match (deterministic operations)

gold!(
    passthrough,
    &["-f"],
    b"From: user@host\nSubject: Test\n\nBody\n"
);

gold!(
    extract_value,
    &["-x", "Subject"],
    b"From: user@host\nSubject: Hello World\n\nBody\n"
);

gold!(
    extract_value_multiple,
    &["-x", "Received"],
    b"Received: first\nReceived: second\nSubject: Test\n\nBody\n"
);

gold!(
    extract_value_continuation,
    &["-x", "Subject"],
    b"From: user@host\nSubject: This is\n a continued\n header\n\nBody\n"
);

gold!(
    extract_with_name,
    &["-X", "Subject"],
    b"From: user@host\nSubject: Hello\n\nBody\n"
);

gold!(
    extract_with_name_multiple,
    &["-X", "Received"],
    b"Received: first\nReceived: second\nSubject: Test\n\nBody\n"
);

gold!(
    extract_with_name_continuation,
    &["-X", "Subject"],
    b"From: user@host\nSubject: This is\n a continued\n header\n\nBody\n"
);

gold!(
    extract_missing,
    &["-x", "X-Missing"],
    b"From: user@host\n\nBody\n"
);

gold!(
    extract_concatenate,
    &["-c", "-x", "Subject"],
    b"From: user@host\nSubject: This is\n a continued\n header\n\nBody\n"
);

gold!(
    extract_concatenate_with_name,
    &["-c", "-X", "Subject"],
    b"From: user@host\nSubject: This is\n a continued\n header\n\nBody\n"
);

gold!(
    delete_field,
    &["-f", "-I", "Received:"],
    b"From: user@host\nReceived: foo\nSubject: Test\n\nBody\n"
);

gold!(
    keep_first,
    &["-f", "-u", "Received"],
    b"Received: first\nReceived: second\nSubject: Test\n\nBody\n"
);

gold!(
    keep_first_single,
    &["-f", "-u", "Subject"],
    b"From: user@host\nSubject: Test\n\nBody\n"
);

gold!(
    keep_first_missing,
    &["-f", "-u", "X-Missing"],
    b"From: user@host\nSubject: Test\n\nBody\n"
);

gold!(
    keep_last,
    &["-f", "-U", "Received"],
    b"Received: first\nReceived: second\nSubject: Test\n\nBody\n"
);

gold!(
    keep_last_single,
    &["-f", "-U", "Subject"],
    b"From: user@host\nSubject: Test\n\nBody\n"
);

gold!(
    keep_last_missing,
    &["-f", "-U", "X-Missing"],
    b"From: user@host\nSubject: Test\n\nBody\n"
);

gold!(
    rename_multiple,
    &["-f", "-R", "Received:", "X-Received:"],
    b"Received: first\nReceived: second\nSubject: Test\n\nBody\n"
);

gold!(
    rename_missing,
    &["-f", "-R", "X-Missing:", "X-New:"],
    b"From: user@host\nSubject: Test\n\nBody\n"
);

gold!(
    add_if_missing,
    &["-f", "-a", "X-Foo: bar"],
    b"From: user@host\n\nBody\n"
);

gold!(
    add_if_present,
    &["-f", "-a", "X-Foo: bar"],
    b"From: user@host\nX-Foo: original\n\nBody\n"
);

gold!(
    add_if_missing_multiple,
    &["-f", "-a", "X-Foo: bar", "-a", "X-Bar: baz"],
    b"From: user@host\n\nBody\n"
);

gold!(
    add_if_missing_msgid,
    &["-f", "-a", "Message-ID:"],
    b"From: user@host\n\nBody\n",
    normalize_message_id
);

gold!(
    add_if_present_msgid,
    &["-f", "-a", "Message-ID:"],
    b"From: user@host\nMessage-ID: <existing@host>\n\nBody\n"
);

gold!(
    add_always,
    &["-f", "-A", "X-Foo: bar"],
    b"From: user@host\nX-Foo: original\n\nBody\n"
);

gold!(
    add_always_no_existing,
    &["-f", "-A", "X-Foo: bar"],
    b"From: user@host\n\nBody\n"
);

gold!(
    add_always_multiple,
    &["-f", "-A", "X-Foo: bar", "-A", "X-Foo: baz"],
    b"From: user@host\n\nBody\n"
);

gold!(
    rename_field,
    &["-f", "-R", "Subject:", "X-Subject:"],
    b"From: user\nSubject: Test\n\nBody\n"
);

gold!(
    insert_rename,
    &["-f", "-i", "Subject: New Subject"],
    b"From: user\nSubject: Old Subject\n\nBody\n"
);

gold!(
    insert_rename_no_existing,
    &["-f", "-i", "X-New: value"],
    b"From: user\nSubject: Test\n\nBody\n"
);

gold!(
    insert_rename_multiple,
    &["-f", "-i", "Received: new"],
    b"Received: first\nReceived: second\nSubject: Test\n\nBody\n"
);

gold!(
    insert_rename_name_only,
    &["-f", "-i", "Subject:"],
    b"From: user\nSubject: Old Subject\n\nBody\n"
);

gold!(
    delete_insert,
    &["-f", "-I", "Subject: New Subject"],
    b"From: user\nSubject: Old Subject\n\nBody\n"
);

gold!(
    delete_insert_no_existing,
    &["-f", "-I", "X-New: value"],
    b"From: user\nSubject: Test\n\nBody\n"
);

gold!(
    delete_insert_multiple,
    &["-f", "-I", "Received: new"],
    b"Received: first\nReceived: second\nSubject: Test\n\nBody\n"
);

gold!(
    delete_insert_name_only,
    &["-f", "-I", "Subject:"],
    b"From: user\nSubject: Old Subject\n\nBody\n"
);

gold!(
    delete_insert_name_only_no_existing,
    &["-f", "-I", "X-Missing:"],
    b"From: user\nSubject: Test\n\nBody\n"
);

gold!(
    zap_space,
    &["-f", "-z"],
    b"From: user@host\nX-NoSpace:value\n\nBody\n"
);

gold!(
    zap_empty,
    &["-f", "-z"],
    b"From: user@host\nX-Empty:   \nSubject: Test\n\nBody\n"
);

gold!(
    concatenate,
    &["-f", "-c", "-X", "Subject"],
    b"From: user@host\nSubject: This is\n a long\n subject\n\nBody\n"
);

gold!(
    no_escape,
    &["-f", "-b"],
    b"From: user@host\n\nFrom the beginning\nBody\n"
);

gold!(
    custom_prefix,
    &["-p", "|"],
    b"From: user@host\n\nFrom the start\nBody\n",
    normalize_from_line
);

// Tier 2: Normalized match (From_ line timestamps)

gold!(
    add_from_line,
    &[],
    b"From: user@host\nSubject: Test\n\nBody\n",
    normalize_from_line
);

gold!(
    preserve_from_line,
    &[],
    b"From user@host Mon Jan  1 00:00:00 2024\nFrom: user@host\n\nBody\n",
    normalize_from_line
);

gold!(
    from_escape,
    &[],
    b"From: user@host\n\nFrom the start\nFrom me to you\n",
    normalize_from_line
);

// Tier 3: Reply mode

gold!(
    reply_basic,
    &["-r", "-t"],
    b"From: sender@example.com\nSubject: Hello\nMessage-ID: <123@host>\n\nBody\n",
    normalize_message_id
);

gold!(
    reply_no_double_re,
    &["-r", "-t"],
    b"From: sender@example.com\nSubject: Re: Hello\n\nBody\n",
    normalize_message_id
);

gold!(
    reply_with_body,
    &["-r", "-t", "-k"],
    b"From: sender@example.com\nSubject: Hello\n\nBody here\n",
    normalize_message_id
);

// Tier 4: Split mode

gold!(
    split_mbox,
    &["-s"],
    b"From a@a Mon Jan  1 00:00:00 2024\n\
      From: a@a\nSubject: First\n\nB1\n\n\
      From b@b Mon Jan  1 00:00:00 2024\n\
      From: b@b\nSubject: Second\n\nB2\n"
);

gold!(
    split_skip,
    &["+1", "-s"],
    b"From a@a Mon Jan  1 00:00:00 2024\n\
      From: a@a\nSubject: First\n\nB1\n\n\
      From b@b Mon Jan  1 00:00:00 2024\n\
      From: b@b\nSubject: Second\n\nB2\n"
);

gold!(
    split_total,
    &["-1", "-s"],
    b"From a@a Mon Jan  1 00:00:00 2024\n\
      From: a@a\nSubject: First\n\nB1\n\n\
      From b@b Mon Jan  1 00:00:00 2024\n\
      From: b@b\nSubject: Second\n\nB2\n\n\
      From c@c Mon Jan  1 00:00:00 2024\n\
      From: c@c\nSubject: Third\n\nB3\n"
);

gold!(
    split_content_length,
    &["-s"],
    b"From a@a Mon Jan  1 00:00:00 2024\n\
      From: a@a\nSubject: First\nContent-Length: 30\n\n\
      From the desk of nobody...\nok\n\n\
      From b@b Mon Jan  1 00:00:00 2024\n\
      From: b@b\nSubject: Second\n\nBody2\n"
);

gold!(
    split_digest,
    &["-f", "-d", "-s"],
    b"From: digest@host\nSubject: Digest\n\n---\n\
      From: a@a\nSubject: First\n\nB1\n\n\
      From: b@b\nSubject: Second\n\nB2\n"
);

// Tier 5: Edge cases

gold!(empty_input, &["-f"], b"");

gold!(no_body, &["-f"], b"From: user@host\nSubject: Test\n\n");

gold!(no_headers, &["-f"], b"Just body text\n");

gold!(
    long_continuation,
    &["-f", "-X", "Subject"],
    b"Subject: This\n is\n a\n very\n long\n continued\n header\n\nBody\n"
);

gold!(
    multiple_received,
    &["-f"],
    b"Received: first\nReceived: second\nReceived: third\nSubject: Test\n\nBody\n"
);

#[test]
fn binary_body() {
    let mut input = b"Subject: Test\n\n".to_vec();
    for b in 1u8..=255 {
        // skip 0 to avoid null-termination issues
        input.push(b);
    }
    input.push(b'\n');

    run_once(&["-f"], &input).assert_eq();
}

#[test]
fn control_char_in_field_name() {
    let input = b"From: a@a\nSub\x01ject: Test\n\nBody\n";
    run_once(&["-f"], input).assert_eq();
}

#[test]
fn very_long_continuation() {
    let mut input = b"Subject: start\n ".to_vec();
    input.extend(iter::repeat_n(b'x', 2000));
    input.extend_from_slice(b"\n\nBody\n");
    run_once(&["-f", "-X", "Subject"], &input).assert_eq();
}

#[test]
fn binary_header_value() {
    let mut input = b"X-Bin: ".to_vec();
    for b in 1u8..=255 {
        if b != b'\n' {
            input.push(b);
        }
    }
    input.extend_from_slice(b"\n\nBody\n");
    run_once(&["-f"], &input).assert_eq();
}

#[test]
fn duplicate_new() {
    let input = b"From: a@a\nMessage-ID: <gold-new@host>\n\nBody\n";
    run_once(&["-D", "1000", "cache"], input).assert_eq();
}

#[test]
fn duplicate_found() {
    let g = gold();
    let input = b"From: a@a\nMessage-ID: <gold-found@host>\n\nBody\n";
    // First run populates cache
    run(&g, &["-D", "1000", "cache"], input).assert_eq();
    // Second finds duplicate
    run(&g, &["-D", "1000", "cache"], input).assert_eq();
}

gold!(
    duplicate_no_msgid,
    &["-D", "1000", "cache"],
    b"From: a@a\nSubject: Test\n\nBody\n"
);

gold!(
    duplicate_empty_msgid,
    &["-D", "1000", "cache"],
    b"From: a@a\nMessage-ID:\n\nBody\n"
);

gold!(
    duplicate_whitespace_msgid,
    &["-D", "1000", "cache"],
    b"From: a@a\nMessage-ID:   \n\nBody\n"
);

gold!(
    duplicate_msgid_special,
    &["-D", "1000", "cache"],
    b"From: a@a\nMessage-ID: <special+chars_123.test@host.domain>\n\nBody\n"
);

#[test]
fn duplicate_sequence() {
    let g = gold();
    let msg1 = b"From: a@a\nMessage-ID: <seq-1@test>\n\nBody1\n";
    let msg2 = b"From: a@a\nMessage-ID: <seq-2@test>\n\nBody2\n";

    // First occurrence of msg1 - unique
    run(&g, &["-D", "1000", "cache"], msg1).assert_eq();
    // First occurrence of msg2 - unique
    run(&g, &["-D", "1000", "cache"], msg2).assert_eq();
    // Second occurrence of msg1 - should be duplicate
    run(&g, &["-D", "1000", "cache"], msg1).assert_eq();
}

#[test]
fn duplicate_wraparound() {
    let g = gold();
    // <wrap-1@h> = 10 chars. Entry = 10 + 1 (null) + 1 (end marker) = 12 bytes.
    // With maxlen=11, scan stops before seeing end marker, so second entry
    // wraps to start, evicting the first.
    let msg1 = b"From: a@a\nMessage-ID: <wrap-1@h>\n\nBody\n";
    let msg2 = b"From: a@a\nMessage-ID: <wrap-2@h>\n\nBody\n";

    run(&g, &["-D", "11", "cache"], msg1).assert_eq();
    run(&g, &["-D", "11", "cache"], msg2).assert_eq();
    // msg2 overwrote msg1, so msg1 is now unique again
    run(&g, &["-D", "11", "cache"], msg1).assert_eq();
    // msg2 should still be cached
    run(&g, &["-D", "11", "cache"], msg2).assert_eq();
}

#[test]
fn duplicate_reply_mode() {
    let g = gold();
    // Two messages with same From address but different Message-IDs
    let msg1 = b"From: same@sender.com\nMessage-ID: <reply-1@host>\n\nBody1\n";
    let msg2 = b"From: same@sender.com\nMessage-ID: <reply-2@host>\n\nBody2\n";

    // First message with this sender
    run(&g, &["-D", "1000", "cache", "-r"], msg1).assert_eq();
    // Second message from same sender - should be duplicate in -r mode
    run(&g, &["-D", "1000", "cache", "-r"], msg2).assert_eq();
}

#[test]
fn duplicate_leading_whitespace_match() {
    let g = gold();
    // Procmail strips leading spaces only (not trailing)
    let msg1 = b"From: a@a\nMessage-ID: <ws-test@host>\n\nBody\n";
    let msg2 = b"From: a@a\nMessage-ID:   <ws-test@host>\n\nBody\n";

    // First occurrence
    run(&g, &["-D", "1000", "cache"], msg1).assert_eq();
    // Same ID with extra leading whitespace - should match as duplicate
    run(&g, &["-D", "1000", "cache"], msg2).assert_eq();
}

gold!(
    log_summary,
    &["-l", "/inbox"],
    b"From user@host Mon Jan  1 00:00:00 2024\nFrom: user@host\nSubject: Test message\n\nBody here\n"
);

// Sender scoring: generate From_ lines and reply To: headers with various
// header combinations, comparing against procmail as oracle.

fn norm_from_and_msgid(data: &[u8]) -> Vec<u8> {
    normalize_message_id(&normalize_from_line(data))
}

fn make_msg(headers: &[(&str, &str)]) -> Vec<u8> {
    let mut msg = Vec::new();
    for &(name, val) in headers {
        if name == "From " {
            msg.extend_from_slice(
                format!("From {} Mon Jan  1 00:00:00 2024\n", val).as_bytes(),
            );
        } else {
            msg.extend_from_slice(format!("{}: {}\n", name, val).as_bytes());
        }
    }
    msg.extend_from_slice(b"\nBody\n");
    msg
}

// From_ line generation with various sender headers
#[test]
fn sender_from_only() {
    let msg = make_msg(&[("From", "user@example.com")]);
    run_once(&[], &msg).assert_eq_with(normalize_from_line);
}

#[test]
fn sender_return_path_and_from() {
    let msg = make_msg(&[
        ("Return-Path", "<bounce@example.com>"),
        ("From", "user@example.com"),
    ]);
    run_once(&[], &msg).assert_eq_with(normalize_from_line);
}

#[test]
fn sender_reply_to_from_sender() {
    let msg = make_msg(&[
        ("Reply-To", "reply@example.com"),
        ("From", "from@example.com"),
        ("Sender", "sender@example.com"),
    ]);
    run_once(&[], &msg).assert_eq_with(normalize_from_line);
}

#[test]
fn sender_resent_headers() {
    let msg = make_msg(&[
        ("Resent-From", "resent@example.com"),
        ("From", "from@example.com"),
    ]);
    run_once(&[], &msg).assert_eq_with(normalize_from_line);
}

#[test]
fn sender_path_only() {
    let msg = make_msg(&[("Path", "news!gateway!user")]);
    run_once(&[], &msg).assert_eq_with(normalize_from_line);
}

#[test]
fn sender_errors_to_only() {
    let msg = make_msg(&[("Errors-To", "errors@example.com")]);
    run_once(&[], &msg).assert_eq_with(normalize_from_line);
}

#[test]
fn sender_empty_return_path() {
    let msg = make_msg(&[("Return-Path", "<>"), ("From", "user@example.com")]);
    run_once(&[], &msg).assert_eq_with(normalize_from_line);
}

#[test]
fn sender_bare_user_penalty() {
    // bare user in Reply-To should lose to good address in From
    let msg = make_msg(&[
        ("Reply-To", "bareuser"),
        ("From", "good@host.example.com"),
    ]);
    run_once(&[], &msg).assert_eq_with(normalize_from_line);
}

#[test]
fn sender_uucp_penalty() {
    let msg = make_msg(&[
        ("Reply-To", "host.UUCP!user"),
        ("From", "good@host.example.com"),
    ]);
    run_once(&[], &msg).assert_eq_with(normalize_from_line);
}

#[test]
fn sender_no_dot_penalty() {
    let msg = make_msg(&[
        ("Reply-To", "user@localhost"),
        ("From", "good@host.example.com"),
    ]);
    run_once(&[], &msg).assert_eq_with(normalize_from_line);
}

#[test]
fn sender_from_line_first() {
    let msg = make_msg(&[
        ("From ", "envelope@example.com"),
        ("From", "header@example.com"),
    ]);
    run_once(&[], &msg).assert_eq_with(normalize_from_line);
}

#[test]
fn sender_from_line_not_first() {
    // From_ not first field → should be ignored for sender scoring
    let msg = make_msg(&[
        ("From", "header@example.com"),
        ("From ", "envelope@example.com"),
    ]);
    run_once(&["-f"], &msg).assert_eq();
}

// Reply mode (-r -t) sender selection
#[test]
fn reply_sender_trusted_reply_to() {
    let msg = make_msg(&[
        ("Reply-To", "reply@example.com"),
        ("From", "from@example.com"),
        ("Subject", "Test"),
    ]);
    run_once(&["-r", "-t"], &msg).assert_eq_with(normalize_message_id);
}

#[test]
fn reply_sender_trusted_from() {
    let msg = make_msg(&[
        ("From", "from@example.com"),
        ("Sender", "sender@example.com"),
        ("Subject", "Test"),
    ]);
    run_once(&["-r", "-t"], &msg).assert_eq_with(normalize_message_id);
}

#[test]
fn reply_sender_untrusted() {
    let msg = make_msg(&[
        ("Return-Path", "<bounce@example.com>"),
        ("Reply-To", "reply@example.com"),
        ("From", "from@example.com"),
        ("Subject", "Test"),
    ]);
    run_once(&["-r"], &msg).assert_eq_with(normalize_message_id);
}

#[test]
fn reply_sender_untrusted_from_line() {
    let msg = make_msg(&[
        ("From ", "envelope@example.com"),
        ("Reply-To", "reply@example.com"),
        ("From", "from@example.com"),
        ("Subject", "Test"),
    ]);
    run_once(&["-r"], &msg).assert_eq_with(norm_from_and_msgid);
}

// Resent-reply mode: -r -t -a "Resent-" prefers Resent-* headers
#[test]
fn reply_resent_reply_to() {
    let msg = make_msg(&[
        ("Reply-To", "regular@example.com"),
        ("From", "from@example.com"),
        ("Resent-Reply-To", "resent-reply@example.com"),
        ("Resent-From", "resent-from@example.com"),
        ("Subject", "Test"),
    ]);
    run_once(&["-r", "-t", "-a", "Resent-"], &msg)
        .assert_eq_with(normalize_message_id);
}

#[test]
fn reply_resent_from_only() {
    let msg = make_msg(&[
        ("Reply-To", "regular@example.com"),
        ("Resent-From", "resent-from@example.com"),
        ("Subject", "Test"),
    ]);
    run_once(&["-r", "-t", "-a", "Resent-"], &msg)
        .assert_eq_with(normalize_message_id);
}

#[test]
fn reply_resent_no_resent_headers() {
    let msg = make_msg(&[
        ("Reply-To", "regular@example.com"),
        ("From", "from@example.com"),
        ("Subject", "Test"),
    ]);
    run_once(&["-r", "-t", "-a", "Resent-"], &msg)
        .assert_eq_with(normalize_message_id);
}

// Angle bracket and RFC 822 address extraction
#[test]
fn sender_angle_brackets() {
    let msg = make_msg(&[("From", "\"John Doe\" <john@example.com>")]);
    run_once(&[], &msg).assert_eq_with(normalize_from_line);
}

#[test]
fn sender_comment_in_from() {
    let msg = make_msg(&[("From", "(John Doe) john@example.com")]);
    run_once(&[], &msg).assert_eq_with(normalize_from_line);
}

// Parametric: try many header combos as From_ line generation
#[test]
fn sender_matrix() {
    let combos: &[&[(&str, &str)]] = &[
        &[("From", "a@b.com")],
        &[("Sender", "a@b.com")],
        &[("Return-Path", "<a@b.com>")],
        &[("From", "a@b.com"), ("Sender", "b@c.com")],
        &[("Return-Path", "<a@b.com>"), ("From", "b@c.com")],
        &[
            ("Reply-To", "r@b.com"),
            ("From", "f@b.com"),
            ("Sender", "s@b.com"),
        ],
        &[("Errors-To", "e@b.com"), ("From", "f@b.com")],
        &[("Return-Receipt-To", "rr@b.com"), ("From", "f@b.com")],
        &[("Return-Path", "<>"), ("From", "f@b.com")],
        &[("Path", "a!b!c"), ("From", "f@b.com")],
        &[("Reply-To", "bare"), ("From", "good@host.example.com")],
    ];
    for (i, headers) in combos.iter().enumerate() {
        let msg = make_msg(headers);
        let r = run_once(&[], &msg);
        let rust = normalize_from_line(&r.rust_out);
        let proc = normalize_from_line(&r.proc_out);
        assert_eq!(
            rust,
            proc,
            "combo {i} ({headers:?}):\n  rust: {}\n  proc: {}",
            String::from_utf8_lossy(&rust),
            String::from_utf8_lossy(&proc),
        );
    }
}
