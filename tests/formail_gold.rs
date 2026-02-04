//! Gold standard tests comparing Rust formail against procmail's formail.
//!
//! Run with:
//!     PROCMAIL_FORMAIL=/bin/formail \
//!         cargo test --features gold --test formail_gold

#![cfg(feature = "gold")]

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn procmail_formail() -> String {
    std::env::var("PROCMAIL_FORMAIL")
        .expect("PROCMAIL_FORMAIL env var required")
}

fn rust_formail() -> &'static str {
    env!("CARGO_BIN_EXE_formail")
}

fn run(dir: &Path, bin: &str, args: &[&str], input: &[u8]) -> (Vec<u8>, i32) {
    let mut child = Command::new(bin)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn formail");

    child.stdin.take().unwrap().write_all(input).unwrap();
    let out = child.wait_with_output().expect("failed to wait");
    (out.stdout, out.status.code().unwrap_or(-1))
}

struct Gold {
    rust_out: Vec<u8>,
    rust_code: i32,
    proc_out: Vec<u8>,
    proc_code: i32,
}

impl Gold {
    fn run(args: &[&str], input: &[u8]) -> Self {
        let rust_dir = tempfile::tempdir().unwrap();
        let proc_dir = tempfile::tempdir().unwrap();
        let proc = procmail_formail();
        let (rust_out, rust_code) =
            run(rust_dir.path(), rust_formail(), args, input);
        let (proc_out, proc_code) = run(proc_dir.path(), &proc, args, input);
        Self {
            rust_out,
            rust_code,
            proc_out,
            proc_code,
        }
    }

    fn assert_eq(&self) {
        assert_eq!(
            self.rust_code, self.proc_code,
            "exit codes differ: rust={}, proc={}",
            self.rust_code, self.proc_code
        );
        if self.rust_out != self.proc_out {
            panic!(
                "stdout differs:\n--- rust ---\n{}\n--- proc ---\n{}",
                String::from_utf8_lossy(&self.rust_out),
                String::from_utf8_lossy(&self.proc_out)
            );
        }
    }

    fn assert_eq_with<F: Fn(&[u8]) -> Vec<u8>>(&self, norm: F) {
        assert_eq!(
            self.rust_code, self.proc_code,
            "exit codes differ: rust={}, proc={}",
            self.rust_code, self.proc_code
        );
        let rust = norm(&self.rust_out);
        let proc = norm(&self.proc_out);
        if rust != proc {
            panic!(
                "stdout differs after normalization:\n--- rust ---\n{}\n--- proc ---\n{}",
                String::from_utf8_lossy(&rust),
                String::from_utf8_lossy(&proc)
            );
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
            Gold::run($args, $input).assert_eq();
        }
    };
    ($name:ident, $args:expr, $input:expr, $norm:expr) => {
        #[test]
        fn $name() {
            Gold::run($args, $input).assert_eq_with($norm);
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
    extract_with_name,
    &["-X", "Subject"],
    b"From: user@host\nSubject: Hello\n\nBody\n"
);

gold!(
    extract_missing,
    &["-x", "X-Missing"],
    b"From: user@host\n\nBody\n"
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
    keep_last,
    &["-f", "-U", "Received"],
    b"Received: first\nReceived: second\nSubject: Test\n\nBody\n"
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
    add_always,
    &["-f", "-A", "X-Foo: bar"],
    b"From: user@host\nX-Foo: original\n\nBody\n"
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

    Gold::run(&["-f"], &input).assert_eq();
}

#[test]
fn duplicate_new() {
    let input = b"From: a@a\nMessage-ID: <gold-new@host>\n\nBody\n";
    Gold::run(&["-D", "1000", "cache"], input).assert_eq();
}

#[test]
fn duplicate_found() {
    let input = b"From: a@a\nMessage-ID: <gold-found@host>\n\nBody\n";
    // First run populates cache, second finds duplicate
    Gold::run(&["-D", "1000", "cache"], input);
    Gold::run(&["-D", "1000", "cache"], input).assert_eq();
}
