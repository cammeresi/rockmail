//! Gold standard tests comparing Rust rockmail against procmail.
//!
//! Run with:
//!     PROCMAIL_PROCMAIL=/bin/procmail \
//!         cargo test --features gold --test rockmail_gold

#![cfg(feature = "gold")]

use std::borrow::Borrow;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::{env, fs, panic, process};

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
    let dir =
        env::temp_dir().join(format!("rockmail-gold-fail-{}", process::id(),));
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

fn run_gold_full(
    rc_template: &str, extra: &[&str], inputs: &[&[u8]], count: Option<usize>,
    cmp: fn(&Path, &Path),
) {
    let g = Gold::new();
    setup(g.rust_dir.path(), rc_template);
    setup(g.proc_dir.path(), rc_template);

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        run_gold_inner(&g, extra, inputs, count, cmp);
    }));
    if let Err(e) = result {
        let dir = preserve_failure(&g, rc_template, inputs);
        eprintln!("preserved failure artifacts in {}", dir.display());
        panic::resume_unwind(e);
    }
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
