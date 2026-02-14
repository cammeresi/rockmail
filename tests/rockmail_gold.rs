//! Gold standard tests comparing Rust rockmail against procmail.
//!
//! Run with:
//!     cargo test --features gold --test rockmail_gold
//!
//! The original procmail binary is found automatically.  To override,
//! set PROCMAIL_PROCMAIL to the path of the original procmail.

#![cfg(feature = "gold")]

use std::borrow::Borrow;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::{fs, panic, process};

use rand::Rng;
use rand::seq::SliceRandom;

use rockmail::delivery::FolderType;

#[allow(unused)]
mod common;

use common::{
    Gold, RcBuilder, diff_dirs, procmail, rockmail, run, setup, snapshot,
};

const MSGS: &[&[u8]] = &[
    b"From: a@host\nSubject: one\n\nBody one\n",
    b"From: b@host\nSubject: two\n\nBody two\n",
    b"From: c@host\nSubject: three\n\nBody three\n",
    b"From: d@host\nSubject: four\n\nBody four\n",
    b"From: e@host\nSubject: five\n\nBody five\n",
];

const ADDRS: &[&str] = &[
    "alice@example.com",
    "bob@work.org",
    "carol@lists.net",
    "dave@spam.biz",
    "eve@friend.io",
];

const SUBJECTS: &[&str] = &[
    "Meeting tomorrow",
    "URGENT deal",
    "Re: project update",
    "Newsletter #42",
    "Invitation to connect",
];

const LISTS: &[&str] =
    &["dev@lists.net", "announce@lists.net", "security@lists.net"];

fn file_count(dir: &Path) -> usize {
    snapshot(dir).len()
}

fn copy_dir(src: &Path, dst: &Path) {
    let _ = fs::create_dir_all(dst);
    let Ok(entries) = fs::read_dir(src) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        let target = dst.join(e.file_name());
        if p.is_dir() {
            copy_dir(&p, &target);
        } else {
            fs::copy(&p, &target).ok();
        }
    }
}

fn preserve_failure(g: &Gold, rc_template: &str, inputs: &[&[u8]]) -> PathBuf {
    let dir = PathBuf::from("tmp")
        .join(format!("rockmail-gold-fail-{}", process::id(),));
    let _ = fs::create_dir_all(&dir);
    fs::write(dir.join("rcfile"), rc_template).ok();
    for (i, msg) in inputs.iter().enumerate() {
        fs::write(dir.join(format!("msg-{i:02}")), msg).ok();
    }

    // Copy the maildir trees for comparison
    let rust_out = dir.join("rust");
    let proc_out = dir.join("proc");
    copy_dir(&g.rust_dir.path().join("maildir"), &rust_out);
    copy_dir(&g.proc_dir.path().join("maildir"), &proc_out);
    dir
}

fn assert_dirs(a: &Path, b: &Path) {
    diff_dirs(a, b).unwrap();
}

fn run_gold_inner(
    g: &Gold, extra: &[&str], inputs: &[&[u8]], count: Option<usize>,
    cmp: fn(&Path, &Path),
) {
    let rc_r = g.rust_dir.path().join("rcfile");
    let rc_p = g.proc_dir.path().join("rcfile");
    let mut args_r: Vec<&str> = vec!["-f", "sender@test"];
    let mut args_p: Vec<&str> = vec!["-f", "sender@test"];
    args_r.extend_from_slice(extra);
    args_p.extend_from_slice(extra);
    args_r.push(rc_r.to_str().unwrap());
    args_p.push(rc_p.to_str().unwrap());
    for input in inputs {
        let (_, rc) = run(g.rust_dir.path(), rockmail(), &args_r, input);
        let (_, pc) = run(g.proc_dir.path(), procmail(), &args_p, input);
        assert_eq!(rc, pc, "exit codes differ: rust={rc}, proc={pc}");
    }
    let r = &g.rust_dir.path().join("maildir");
    if let Some(count) = count {
        let n = file_count(r);
        assert_eq!(n, count, "expected {count} files in maildir, got {n}");
    }
    cmp(r, &g.proc_dir.path().join("maildir"));
}

fn run_gold_setup(
    rc_template: &str, extra: &[&str], inputs: &[&[u8]], count: Option<usize>,
    cmp: fn(&Path, &Path), pre: impl Fn(&Path),
) {
    let g = Gold::new();
    setup(g.rust_dir.path(), rc_template);
    setup(g.proc_dir.path(), rc_template);
    pre(&g.rust_dir.path().join("maildir"));
    pre(&g.proc_dir.path().join("maildir"));

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        run_gold_inner(&g, extra, inputs, count, cmp);
    }));
    if let Err(e) = result {
        let dir = preserve_failure(&g, rc_template, inputs);
        eprintln!("preserved failure artifacts in {}", dir.display());
        panic::resume_unwind(e);
    }
}

fn run_gold_full(
    rc_template: &str, extra: &[&str], inputs: &[&[u8]], count: Option<usize>,
    cmp: fn(&Path, &Path),
) {
    run_gold_setup(rc_template, extra, inputs, count, cmp, |_| {});
}

fn run_gold_with<S>(
    rc_template: S, inputs: &[&[u8]], count: Option<usize>,
    cmp: fn(&Path, &Path),
) where
    S: Borrow<str>,
{
    run_gold_full(rc_template.borrow(), &[], inputs, count, cmp);
}

fn run_gold(rc_template: &str, inputs: &[&[u8]], count: usize) {
    run_gold_with(rc_template, inputs, Some(count), assert_dirs);
}

fn run_gold_args(
    rc_template: &str, extra: &[&str], inputs: &[&[u8]], count: usize,
) {
    run_gold_full(rc_template, extra, inputs, Some(count), assert_dirs);
}

fn build_complex_rc(kind: FolderType) -> String {
    let mut b = RcBuilder::new(kind);
    b.spam().folder("spam");
    b.lists().folder("lists");
    b.copy().priority().folder("urgent");
    for addr in &ADDRS[..2] {
        b.from(addr).folder(addr.split('@').next().unwrap());
    }
    b.subject("URGENT").folder("urgent");
    b.build()
}

fn build_complex_msgs() -> Vec<Vec<u8>> {
    let msg = |from: &str, to: &str, subj: &str, extra: &str| {
        let hdrs = if extra.is_empty() {
            format!("From: {from}\nTo: {to}\nSubject: {subj}")
        } else {
            format!("From: {from}\nTo: {to}\nSubject: {subj}\n{extra}")
        };
        format!("{hdrs}\n\nBody\n").into_bytes()
    };
    vec![
        // 0: spam — discarded
        msg(ADDRS[3], ADDRS[0], SUBJECTS[0], "X-Spam-Flag: YES"),
        // 1: to list, no other match — lists
        msg(ADDRS[4], LISTS[0], SUBJECTS[2], ""),
        // 2: high priority from alice — urgent (copy) + alice
        msg(ADDRS[0], ADDRS[4], SUBJECTS[0], "X-Priority: 1"),
        // 3: from bob, normal — bob
        msg(ADDRS[1], ADDRS[0], SUBJECTS[2], ""),
        // 4: URGENT subject, from carol — urgent
        msg(ADDRS[2], ADDRS[0], SUBJECTS[1], ""),
        // 5: high priority to list — urgent (copy) + lists
        msg(ADDRS[4], LISTS[1], SUBJECTS[3], "X-Priority: 1"),
        // 6: from alice, normal — alice
        msg(ADDRS[0], ADDRS[1], SUBJECTS[4], ""),
        // 7: no match — default
        msg(ADDRS[4], ADDRS[3], SUBJECTS[0], ""),
        // 8: spam with high priority — discarded (spam rule first)
        msg(
            ADDRS[3],
            ADDRS[0],
            SUBJECTS[1],
            "X-Spam-Flag: YES\nX-Priority: 1",
        ),
        // 9: from bob, URGENT subject — bob (from rule before subject rule)
        msg(ADDRS[1], ADDRS[0], SUBJECTS[1], ""),
    ]
}

fn build_random_rc<R>(rng: &mut R, kind: FolderType) -> String
where
    R: Rng,
{
    let mut b = RcBuilder::new(kind);
    if rng.gen_bool(0.5) {
        b.spam().folder("spam");
    }
    if rng.gen_bool(0.5) {
        b.lists().folder("lists");
    }
    if rng.gen_bool(0.5) {
        b.copy().priority().folder("urgent");
    }
    let n = rng.gen_range(1..=ADDRS.len());
    let mut addrs: Vec<_> = ADDRS.to_vec();
    addrs.shuffle(rng);
    for addr in &addrs[..n] {
        b.from(addr).folder(addr.split('@').next().unwrap());
    }
    if rng.gen_bool(0.5) {
        b.subject("URGENT").folder("urgent");
    }
    b.build()
}

fn build_random_msgs<R>(rng: &mut R) -> Vec<Vec<u8>>
where
    R: Rng,
{
    let n = rng.gen_range(10..=20);
    let msg = |from: &str, to: &str, subj: &str, extra: &str| {
        let hdrs = if extra.is_empty() {
            format!("From: {from}\nTo: {to}\nSubject: {subj}")
        } else {
            format!("From: {from}\nTo: {to}\nSubject: {subj}\n{extra}")
        };
        format!("{hdrs}\n\nBody\n").into_bytes()
    };
    let extras = ["", "", "", "X-Spam-Flag: YES", "X-Priority: 1"];
    (0..n)
        .map(|_| {
            let from = ADDRS.choose(rng).unwrap();
            let to = if rng.gen_bool(0.3) {
                LISTS.choose(rng).unwrap()
            } else {
                ADDRS.choose(rng).unwrap()
            };
            let subj = SUBJECTS.choose(rng).unwrap();
            let extra = extras.choose(rng).unwrap();
            msg(from, to, subj, extra)
        })
        .collect()
}

fn build_size_msgs<R>(rng: &mut R) -> Vec<Vec<u8>>
where
    R: Rng,
{
    let n = rng.gen_range(10..=20);
    (0..n)
        .map(|_| {
            let from = ADDRS.choose(rng).unwrap();
            let to = ADDRS.choose(rng).unwrap();
            let subj = SUBJECTS.choose(rng).unwrap();
            let body_len = rng.gen_range(10..=500);
            let body: String = (0..body_len).map(|_| 'x').collect();
            format!("From: {from}\nTo: {to}\nSubject: {subj}\n\n{body}\n")
                .into_bytes()
        })
        .collect()
}

fn build_size_rc<R>(rng: &mut R, kind: FolderType) -> String
where
    R: Rng,
{
    let thresh = rng.gen_range(100..=300);
    let addr = ADDRS.choose(rng).unwrap();
    let mut b = RcBuilder::new(kind);
    b.size_gt(thresh).folder("big");
    b.size_lt(thresh).folder("small");
    b.negate().from(addr).folder("other");
    b.build()
}

fn run_random(kind: FolderType) {
    let mut rng = rand::thread_rng();
    let rc = build_random_rc(&mut rng, kind);
    let msgs = build_random_msgs(&mut rng);
    let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
    run_gold_with(rc, &refs, None, assert_dirs);
}

fn run_size(kind: FolderType) {
    let mut rng = rand::thread_rng();
    let rc = build_size_rc(&mut rng, kind);
    let msgs = build_size_msgs(&mut rng);
    let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
    run_gold_with(rc, &refs, None, assert_dirs);
}

#[test]
fn deliver_mbox() {
    let rc = RcBuilder::new(FolderType::File).folder("inbox").build();
    run_gold(&rc, MSGS, 1);
}

#[test]
fn deliver_maildir() {
    let rc = RcBuilder::new(FolderType::Maildir).folder("inbox").build();
    run_gold_with(rc, MSGS, Some(5), assert_dirs);
}

#[test]
fn deliver_mh() {
    let rc = RcBuilder::new(FolderType::Mh).folder("inbox").build();
    run_gold(&rc, MSGS, 5);
}

#[test]
fn deliver_dev_null() {
    let rc = RcBuilder::new(FolderType::File).dev_null().build();
    run_gold(&rc, MSGS, 0);
}

#[test]
fn complex_filtering_static_mbox() {
    let msgs = build_complex_msgs();
    let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
    run_gold(&build_complex_rc(FolderType::File), &refs, 6);
}

#[test]
fn complex_filtering_static_maildir() {
    let msgs = build_complex_msgs();
    let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
    run_gold_with(
        build_complex_rc(FolderType::Maildir),
        &refs,
        None,
        assert_dirs,
    );
}

#[test]
fn complex_filtering_static_mh() {
    let msgs = build_complex_msgs();
    let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
    run_gold_with(build_complex_rc(FolderType::Mh), &refs, None, assert_dirs);
}

#[test]
fn complex_filtering_random_mbox() {
    run_random(FolderType::File);
}

#[test]
fn complex_filtering_random_maildir() {
    run_random(FolderType::Maildir);
}

#[test]
fn complex_filtering_random_mh() {
    run_random(FolderType::Mh);
}

#[test]
fn size_and_negation_mbox() {
    run_size(FolderType::File);
}

#[test]
fn size_and_negation_maildir() {
    run_size(FolderType::Maildir);
}

#[test]
fn size_and_negation_mh() {
    run_size(FolderType::Mh);
}

#[test]
fn subst_expands_variable_in_condition() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
SENDER=one

:0
* $ ^Subject:.*$SENDER
matched
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: one\n\nBody\n",
        b"From: b@host\nSubject: two\n\nBody\n",
    ];
    run_gold(rc, msgs, 2);
}

#[test]
fn regex_without_subst_no_expansion() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
SENDER=one

:0
* ^Subject:.*$SENDER
matched
";
    // Without $, the literal "$SENDER" won't match "one", so both
    // messages go to default.
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: one\n\nBody\n",
        b"From: b@host\nSubject: two\n\nBody\n",
    ];
    run_gold(rc, msgs, 1);
}

#[test]
fn mh_trailing_blank() {
    let rc = RcBuilder::new(FolderType::Mh).folder("inbox").build();
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: one\n\nBody\n\n"];
    run_gold(&rc, msgs, 1);
}

#[test]
fn mh_trailing_no_newline() {
    let rc = RcBuilder::new(FolderType::Mh).folder("inbox").build();
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: one\n\nBody"];
    run_gold(&rc, msgs, 1);
}

#[test]
fn mh_trailing_single_newline() {
    let rc = RcBuilder::new(FolderType::Mh).folder("inbox").build();
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: one\n\nBody\n"];
    run_gold(&rc, msgs, 1);
}

fn run_dir_gold(rc: &str, inputs: &[&[u8]], count: usize) {
    run_gold_setup(rc, &[], inputs, Some(count), assert_dirs, |maildir| {
        fs::create_dir(maildir.join("inbox")).unwrap();
    });
}

#[test]
fn deliver_dir() {
    let rc = RcBuilder::new(FolderType::Dir).folder("inbox").build();
    run_dir_gold(&rc, MSGS, 5);
}

#[test]
fn dir_trailing_blank() {
    let rc = RcBuilder::new(FolderType::Dir).folder("inbox").build();
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: one\n\nBody\n\n"];
    run_dir_gold(&rc, msgs, 1);
}

#[test]
fn dir_trailing_no_newline() {
    let rc = RcBuilder::new(FolderType::Dir).folder("inbox").build();
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: one\n\nBody"];
    run_dir_gold(&rc, msgs, 1);
}

#[test]
fn dir_trailing_single_newline() {
    let rc = RcBuilder::new(FolderType::Dir).folder("inbox").build();
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: one\n\nBody\n"];
    run_dir_gold(&rc, msgs, 1);
}

#[test]
fn complex_filtering_static_dir() {
    let msgs = build_complex_msgs();
    let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
    let rc = build_complex_rc(FolderType::Dir);
    run_gold_setup(&rc, &[], &refs, None, assert_dirs, |maildir| {
        for name in ["spam", "lists", "urgent", "alice", "bob"] {
            fs::create_dir(maildir.join(name)).unwrap();
        }
    });
}

#[test]
fn subst_brace_syntax() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
PAT=one

:0
* $ ^Subject:.*${PAT}
matched
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: one\n\nBody\n",
        b"From: b@host\nSubject: two\n\nBody\n",
    ];
    run_gold(rc, msgs, 2);
}

#[test]
fn subst_default_value() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* $ ^Subject:.*${UNSET:-fallback}
matched
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: fallback here\n\nBody\n",
        b"From: b@host\nSubject: other\n\nBody\n",
    ];
    run_gold(rc, msgs, 2);
}

#[test]
fn subst_default_value_set() {
    // When VAR is set, ${VAR:-default} should use VAR's value.
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
PAT=hello

:0
* $ ^Subject:.*${PAT:-fallback}
matched
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: hello world\n\nBody\n",
        b"From: b@host\nSubject: fallback\n\nBody\n",
    ];
    run_gold(rc, msgs, 2);
}

#[test]
fn subst_positional_args() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* $ ^Subject:.*$1
matched
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: target\n\nBody\n",
        b"From: b@host\nSubject: other\n\nBody\n",
    ];
    run_gold_args(rc, &["-a", "target"], msgs, 2);
}

#[test]
fn pipe_capture() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
CAPTURED=| /bin/echo captured

:0
* $ ^Subject:.*$CAPTURED
matched
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: captured\n\nBody\n",
        b"From: b@host\nSubject: other\n\nBody\n",
    ];
    run_gold(rc, msgs, 2);
}

#[test]
fn secondary_mh() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT/.

:0
inbox/. copy/.
";
    run_gold_with(rc, MSGS, Some(10), assert_dirs);
}

#[test]
fn secondary_mbox_skipped() {
    // Mbox primary can't link secondaries; both should deliver to mbox only.
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
inbox copy/
";
    run_gold_with(rc, MSGS, Some(1), assert_dirs);
}

#[test]
fn subst_in_shell_condition() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
WANT=one

:0
* $ ? /bin/echo $WANT | /bin/grep -q one
matched
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: any\n\nBody\n"];
    run_gold(rc, msgs, 1);
}

#[test]
fn weighted_positive_score_delivers() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* 1^1 test
matched/
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: test test test\n\nBody\n"];
    run_gold(rc, msgs, 1);
}

#[test]
fn weighted_no_match_skips() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* 100^1 nope
matched/
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: hello\n\nBody\n"];
    run_gold(rc, msgs, 1);
}

#[test]
fn weighted_negated_match_zero() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* 100^1 ! test
matched/
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: test\n\nBody\n"];
    run_gold(rc, msgs, 1);
}

#[test]
fn weighted_negated_nonmatch_delivers() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* 100^1 ! nope
matched/
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: hello\n\nBody\n"];
    run_gold(rc, msgs, 1);
}

#[test]
fn weighted_convergent_exponent() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* 10^0.5 test
matched/
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: test test test test\n\nBody\n",
        b"From: b@host\nSubject: nope\n\nBody\n",
    ];
    run_gold(rc, msgs, 2);
}

#[test]
fn weighted_negative_weight_blocks() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* -10^1 test
matched/
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: test\n\nBody\n"];
    run_gold(rc, msgs, 1);
}

#[test]
fn weighted_multiple_conditions() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* 100^1 test
* -50^1 hello
matched/
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: test hello\n\nBody\n"];
    run_gold(rc, msgs, 1);
}

#[test]
fn weighted_mixed_conditions() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* ^From:.*alice
* 10^1 urgent
matched/
";
    let msgs: &[&[u8]] = &[
        b"From: alice@host\nSubject: urgent\n\nBody\n",
        b"From: bob@host\nSubject: urgent\n\nBody\n",
    ];
    run_gold(rc, msgs, 2);
}

#[test]
fn weighted_empty_match_tail_sum() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* -10^0.5 ^^
matched/
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: hi\n\nBody\n"];
    run_gold(rc, msgs, 1);
}

#[test]
fn weighted_score_determines_delivery() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* 50^1 test
* -30^1 hello
matched/
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: test hello\n\nBody\n",
        b"From: b@host\nSubject: hello hello hello\n\nBody\n",
    ];
    run_gold(rc, msgs, 2);
}

#[test]
fn refresh_from_line_preserves_timestamp() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT/
";
    let input =
        b"From old@host  Wed Jan  1 12:34:56 2025\nSubject: Test\n\nBody\n";
    let g = Gold::new();
    setup(g.rust_dir.path(), rc);
    setup(g.proc_dir.path(), rc);
    let rc_r = g.rust_dir.path().join("rcfile");
    let rc_p = g.proc_dir.path().join("rcfile");
    let args_r = ["-f", "-", rc_r.to_str().unwrap()];
    let args_p = ["-f", "-", rc_p.to_str().unwrap()];
    let (_, rc) = run(g.rust_dir.path(), rockmail(), &args_r, input);
    let (_, pc) = run(g.proc_dir.path(), procmail(), &args_p, input);
    assert_eq!(rc, pc, "exit codes differ: rust={rc}, proc={pc}");
    diff_dirs(
        &g.rust_dir.path().join("maildir"),
        &g.proc_dir.path().join("maildir"),
    )
    .unwrap();
}

/// Extract and normalize LOGABSTRACT lines from a logfile.
///
/// Keeps only the From_, Subject, and Folder lines.  Normalizes the
/// From_ timestamp and replaces absolute folder paths with basenames
/// so that the two temp-dir trees can be compared.
fn normalize_abstract(log: &str) -> Vec<String> {
    let from_re = regex::Regex::new(r"^(From \S+).*").unwrap();
    let folder_re =
        regex::Regex::new(r"^(  Folder: )\S*/([^\t]+)(\t.*)").unwrap();
    log.lines()
        .filter(|l| {
            l.starts_with("From ")
                || l.starts_with(" Subject:")
                || l.starts_with("  Folder:")
        })
        .map(|l| {
            if let Some(c) = from_re.captures(l) {
                c[1].to_string()
            } else if let Some(c) = folder_re.captures(l) {
                format!("{}{}{}", &c[1], &c[2], &c[3])
            } else {
                l.to_string()
            }
        })
        .collect()
}

#[test]
fn logabstract_matches_procmail() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
LOGFILE=$MAILDIR/log
LOGABSTRACT=yes
";
    let g = Gold::new();
    setup(g.rust_dir.path(), rc);
    setup(g.proc_dir.path(), rc);
    // Pre-create the mbox so delivery succeeds
    fs::write(g.rust_dir.path().join("maildir/default"), b"").unwrap();
    fs::write(g.proc_dir.path().join("maildir/default"), b"").unwrap();

    let input = b"Subject: Hello World\n\nBody\n";
    let rc_r = g.rust_dir.path().join("rcfile");
    let rc_p = g.proc_dir.path().join("rcfile");
    let args_r = ["-f", "sender@test", rc_r.to_str().unwrap()];
    let args_p = ["-f", "sender@test", rc_p.to_str().unwrap()];
    let (_, rc) = run(g.rust_dir.path(), rockmail(), &args_r, input);
    let (_, pc) = run(g.proc_dir.path(), procmail(), &args_p, input);
    assert_eq!(rc, pc, "exit codes differ");

    let rlog =
        fs::read_to_string(g.rust_dir.path().join("maildir/log")).unwrap();
    let plog =
        fs::read_to_string(g.proc_dir.path().join("maildir/log")).unwrap();
    let ra = normalize_abstract(&rlog);
    let pa = normalize_abstract(&plog);
    assert_eq!(ra, pa, "logabstract differs:\nrust: {ra:?}\nproc: {pa:?}");
}

#[test]
fn macro_to_underscore() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* ^TO_alice@example\\.com
matched
";
    let msgs: &[&[u8]] = &[
        b"From: x@host\nTo: alice@example.com\n\nBody\n",
        b"From: x@host\nCc: alice@example.com\n\nBody\n",
        b"From: x@host\nTo: bob@host\n\nBody\n",
    ];
    run_gold(rc, msgs, 2);
}

#[test]
fn macro_to_word_boundary() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* ^TOalice
matched
";
    let msgs: &[&[u8]] = &[
        b"From: x@host\nTo: alice@host\n\nBody\n",
        b"From: x@host\nTo: malice@host\n\nBody\n",
    ];
    run_gold(rc, msgs, 2);
}

#[test]
fn macro_from_daemon() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* ^FROM_DAEMON
daemon
";
    let msgs: &[&[u8]] = &[
        b"From: MAILER-DAEMON@host\nSubject: bounce\n\nBody\n",
        b"From: x@host\nPrecedence: bulk\nSubject: list\n\nBody\n",
        b"From: user@host\nSubject: hello\n\nBody\n",
    ];
    run_gold(rc, msgs, 2);
}
