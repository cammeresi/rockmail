//! Gold standard tests comparing Rust formail against procmail's formail.
//!
//! Run with:
//!     PROCMAIL_FORMAIL=/bin/formail \
//!         cargo test --features gold --test formail_gold
//!
//! Many of these tests require running procmail with -f lest the two
//! procmails generate new "From " lines with differing timestamps.
//! Otherwise timestamps need to be ignored when comparing output.

#![cfg(feature = "gold")]

use std::io::{ErrorKind, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn procmail_formail() -> String {
    std::env::var("PROCMAIL_FORMAIL")
        .expect("PROCMAIL_FORMAIL env var required")
}

fn rust_formail() -> &'static str {
    env!("CARGO_BIN_EXE_formail")
}

#[must_use]
struct GoldResult {
    rust_out: Vec<u8>,
    rust_code: i32,
    proc_out: Vec<u8>,
    proc_code: i32,
}

impl GoldResult {
    fn assert_codes_eq(rust: i32, proc: i32) {
        assert_eq!(
            rust, proc,
            "exit codes differ: rust={}, proc={}",
            rust, proc
        );
    }

    fn assert_output_eq(rust: &[u8], proc: &[u8]) {
        if rust != proc {
            panic!(
                "stdout differs:\n--- rust ---\n{}\n--- proc ---\n{}",
                String::from_utf8_lossy(rust),
                String::from_utf8_lossy(proc)
            );
        }
    }

    fn assert_eq(self) {
        Self::assert_codes_eq(self.rust_code, self.proc_code);
        Self::assert_output_eq(&self.rust_out, &self.proc_out);
    }

    fn assert_eq_with<F: Fn(&[u8]) -> Vec<u8>>(self, norm: F) {
        Self::assert_codes_eq(self.rust_code, self.proc_code);
        Self::assert_output_eq(&norm(&self.rust_out), &norm(&self.proc_out));
    }
}

/// Runs both Rust and procmail formail in temp directories for comparison.
struct Gold {
    rust_dir: TempDir,
    proc_dir: TempDir,
}

impl Gold {
    fn new() -> Self {
        Self {
            rust_dir: TempDir::new().unwrap(),
            proc_dir: TempDir::new().unwrap(),
        }
    }

    fn run_impl(
        dir: &Path, bin: &str, args: &[&str], input: &[u8],
    ) -> (Vec<u8>, i32) {
        let mut child = Command::new(bin)
            .args(args)
            .current_dir(dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("failed to spawn formail");

        if let Err(e) = child.stdin.take().unwrap().write_all(input) {
            // formail may exit before reading all input (e.g. -x extracts
            // headers only)
            if e.kind() != ErrorKind::BrokenPipe {
                panic!("failed to write to stdin: {e}");
            }
        }
        let out = child.wait_with_output().expect("failed to wait");
        (out.stdout, out.status.code().unwrap_or(-1))
    }

    /// Run with fresh temp directories (for single-run tests).
    fn run_once(args: &[&str], input: &[u8]) -> GoldResult {
        Gold::new().run(args, input)
    }

    /// Run using persistent temp directories (for multi-run tests).
    fn run(&self, args: &[&str], input: &[u8]) -> GoldResult {
        let proc = procmail_formail();
        let (rust_out, rust_code) =
            Self::run_impl(self.rust_dir.path(), rust_formail(), args, input);
        let (proc_out, proc_code) =
            Self::run_impl(self.proc_dir.path(), &proc, args, input);
        GoldResult {
            rust_out,
            rust_code,
            proc_out,
            proc_code,
        }
    }
}

fn normalize_from_line(data: &[u8]) -> Vec<u8> {
    // Match From_ lines with varying whitespace before timestamp
    let re = regex::bytes::Regex::new(
        r"(?m)^From (\S+) +\w{3} \w{3} [ \d]\d \d{2}:\d{2}:\d{2} \d{4}\n",
    )
    .unwrap();
    re.replace_all(data, b"From $1 TIMESTAMP\n".as_slice())
        .into_owned()
}

fn normalize_message_id(data: &[u8]) -> Vec<u8> {
    let re = regex::bytes::Regex::new(r"Message-ID: <[^>]+>").unwrap();
    re.replace_all(data, b"Message-ID: <GENERATED>".as_slice())
        .into_owned()
}

macro_rules! gold {
    ($name:ident, $args:expr, $input:expr) => {
        #[test]
        fn $name() {
            Gold::run_once($args, $input).assert_eq();
        }
    };
    ($name:ident, $args:expr, $input:expr, $norm:expr) => {
        #[test]
        fn $name() {
            Gold::run_once($args, $input).assert_eq_with($norm);
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

    Gold::run_once(&["-f"], &input).assert_eq();
}

#[test]
fn duplicate_new() {
    let input = b"From: a@a\nMessage-ID: <gold-new@host>\n\nBody\n";
    Gold::run_once(&["-D", "1000", "cache"], input).assert_eq();
}

#[test]
fn duplicate_found() {
    let g = Gold::new();
    let input = b"From: a@a\nMessage-ID: <gold-found@host>\n\nBody\n";
    // First run populates cache
    g.run(&["-D", "1000", "cache"], input).assert_eq();
    // Second finds duplicate
    g.run(&["-D", "1000", "cache"], input).assert_eq();
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
    let g = Gold::new();
    let msg1 = b"From: a@a\nMessage-ID: <seq-1@test>\n\nBody1\n";
    let msg2 = b"From: a@a\nMessage-ID: <seq-2@test>\n\nBody2\n";

    // First occurrence of msg1 - unique
    g.run(&["-D", "1000", "cache"], msg1).assert_eq();
    // First occurrence of msg2 - unique
    g.run(&["-D", "1000", "cache"], msg2).assert_eq();
    // Second occurrence of msg1 - should be duplicate
    g.run(&["-D", "1000", "cache"], msg1).assert_eq();
}

#[test]
fn duplicate_wraparound() {
    let g = Gold::new();
    // <wrap-1@h> = 10 chars. Entry = 10 + 1 (null) + 1 (end marker) = 12 bytes.
    // With maxlen=11, scan stops before seeing end marker, so second entry
    // wraps to start, evicting the first.
    let msg1 = b"From: a@a\nMessage-ID: <wrap-1@h>\n\nBody\n";
    let msg2 = b"From: a@a\nMessage-ID: <wrap-2@h>\n\nBody\n";

    g.run(&["-D", "11", "cache"], msg1).assert_eq();
    g.run(&["-D", "11", "cache"], msg2).assert_eq();
    // msg2 overwrote msg1, so msg1 is now unique again
    g.run(&["-D", "11", "cache"], msg1).assert_eq();
    // msg2 should still be cached
    g.run(&["-D", "11", "cache"], msg2).assert_eq();
}

#[test]
fn duplicate_reply_mode() {
    let g = Gold::new();
    // Two messages with same From address but different Message-IDs
    let msg1 = b"From: same@sender.com\nMessage-ID: <reply-1@host>\n\nBody1\n";
    let msg2 = b"From: same@sender.com\nMessage-ID: <reply-2@host>\n\nBody2\n";

    // First message with this sender
    g.run(&["-D", "1000", "cache", "-r"], msg1).assert_eq();
    // Second message from same sender - should be duplicate in -r mode
    g.run(&["-D", "1000", "cache", "-r"], msg2).assert_eq();
}

#[test]
fn duplicate_leading_whitespace_match() {
    let g = Gold::new();
    // Procmail strips leading spaces only (not trailing)
    let msg1 = b"From: a@a\nMessage-ID: <ws-test@host>\n\nBody\n";
    let msg2 = b"From: a@a\nMessage-ID:   <ws-test@host>\n\nBody\n";

    // First occurrence
    g.run(&["-D", "1000", "cache"], msg1).assert_eq();
    // Same ID with extra leading whitespace - should match as duplicate
    g.run(&["-D", "1000", "cache"], msg2).assert_eq();
}

gold!(
    log_summary,
    &["-l", "/inbox"],
    b"From user@host Mon Jan  1 00:00:00 2024\nFrom: user@host\nSubject: Test message\n\nBody here\n"
);
