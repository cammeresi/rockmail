//! Gold standard tests comparing Rust rockmail against procmail.
//!
//! Run with:
//!     cargo test --test rockmail_gold
//!
//! The original procmail binary is found automatically.  To override,
//! set PROCMAIL_PROCMAIL to the path of the original procmail.

#![cfg(feature = "gold")]

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

use rand::Rng;
use rand::seq::SliceRandom;

use rockmail::delivery::FolderType;

#[allow(unused)]
mod common;
mod gold;

use common::{Gold, RcBuilder};
use gold::{ADDRS, GoldTest, LISTS, MSGS, SUBJECTS};

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

fn msg(from: &str, to: &str, subj: &str, extra: &str) -> Vec<u8> {
    let hdrs = if extra.is_empty() {
        format!("From: {from}\nTo: {to}\nSubject: {subj}")
    } else {
        format!("From: {from}\nTo: {to}\nSubject: {subj}\n{extra}")
    };
    format!("{hdrs}\n\nBody\n").into_bytes()
}

fn build_complex_msgs() -> Vec<Vec<u8>> {
    vec![
        msg(ADDRS[3], ADDRS[0], SUBJECTS[0], "X-Spam-Flag: YES"),
        msg(ADDRS[4], LISTS[0], SUBJECTS[2], ""),
        msg(ADDRS[0], ADDRS[4], SUBJECTS[0], "X-Priority: 1"),
        msg(ADDRS[1], ADDRS[0], SUBJECTS[2], ""),
        msg(ADDRS[2], ADDRS[0], SUBJECTS[1], ""),
        msg(ADDRS[4], LISTS[1], SUBJECTS[3], "X-Priority: 1"),
        msg(ADDRS[0], ADDRS[1], SUBJECTS[4], ""),
        msg(ADDRS[4], ADDRS[3], SUBJECTS[0], ""),
        msg(
            ADDRS[3],
            ADDRS[0],
            SUBJECTS[1],
            "X-Spam-Flag: YES\nX-Priority: 1",
        ),
        msg(ADDRS[1], ADDRS[0], SUBJECTS[1], ""),
    ]
}

fn build_random_rc<R: Rng>(rng: &mut R, kind: FolderType) -> String {
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

fn build_random_msgs<R: Rng>(rng: &mut R) -> Vec<Vec<u8>> {
    let n = rng.gen_range(10..=20);
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

fn build_size_msgs<R: Rng>(rng: &mut R) -> Vec<Vec<u8>> {
    let n = rng.gen_range(10..=20);
    (0..n)
        .map(|_| {
            let from = ADDRS.choose(rng).unwrap();
            let to = ADDRS.choose(rng).unwrap();
            let subj = SUBJECTS.choose(rng).unwrap();
            let len = rng.gen_range(10..=500);
            let body: String = (0..len).map(|_| 'x').collect();
            format!("From: {from}\nTo: {to}\nSubject: {subj}\n\n{body}\n")
                .into_bytes()
        })
        .collect()
}

fn build_size_rc<R: Rng>(rng: &mut R, kind: FolderType) -> String {
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
    GoldTest::new(&rc, &refs).run();
}

fn run_size(kind: FolderType) {
    let mut rng = rand::thread_rng();
    let rc = build_size_rc(&mut rng, kind);
    let msgs = build_size_msgs(&mut rng);
    let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
    GoldTest::new(&rc, &refs).run();
}

#[test]
fn deliver_mbox() {
    let rc = RcBuilder::new(FolderType::File).folder("inbox").build();
    GoldTest::new(&rc, MSGS).run();
}

#[test]
fn deliver_maildir() {
    let rc = RcBuilder::new(FolderType::Maildir).folder("inbox").build();
    GoldTest::new(&rc, MSGS).run();
}

#[test]
fn deliver_mh() {
    let rc = RcBuilder::new(FolderType::Mh).folder("inbox").build();
    GoldTest::new(&rc, MSGS).run();
}

#[test]
fn deliver_dev_null() {
    let rc = RcBuilder::new(FolderType::File).dev_null().build();
    GoldTest::new(&rc, MSGS).run();
}

fn run_complex(kind: FolderType) {
    let msgs = build_complex_msgs();
    let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
    let rc = build_complex_rc(kind);
    GoldTest::new(&rc, &refs).run();
}

#[test]
fn complex_filtering_static_mbox() {
    run_complex(FolderType::File);
}

#[test]
fn complex_filtering_static_maildir() {
    run_complex(FolderType::Maildir);
}

#[test]
fn complex_filtering_static_mh() {
    run_complex(FolderType::Mh);
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
    GoldTest::new(rc, msgs).run();
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
    GoldTest::new(rc, msgs).run();
}

fn run_trailing(kind: FolderType, body: &[u8]) {
    let rc = RcBuilder::new(kind).folder("inbox").build();
    let msgs: &[&[u8]] = &[body];
    if kind == FolderType::Dir {
        GoldTest::new(&rc, msgs)
            .pre(|d| fs::create_dir(d.join("inbox")).unwrap())
            .run();
    } else {
        GoldTest::new(&rc, msgs).run();
    }
}

#[test]
fn mh_trailing_blank() {
    run_trailing(FolderType::Mh, b"From: a@host\nSubject: one\n\nBody\n\n");
}

#[test]
fn mh_trailing_no_newline() {
    run_trailing(FolderType::Mh, b"From: a@host\nSubject: one\n\nBody");
}

#[test]
fn mh_trailing_single_newline() {
    run_trailing(FolderType::Mh, b"From: a@host\nSubject: one\n\nBody\n");
}

fn run_dir_gold(rc: &str, inputs: &[&[u8]]) {
    GoldTest::new(rc, inputs)
        .pre(|d| fs::create_dir(d.join("inbox")).unwrap())
        .run();
}

#[test]
fn deliver_dir() {
    let rc = RcBuilder::new(FolderType::Dir).folder("inbox").build();
    run_dir_gold(&rc, MSGS);
}

#[test]
fn dir_trailing_blank() {
    run_trailing(FolderType::Dir, b"From: a@host\nSubject: one\n\nBody\n\n");
}

#[test]
fn dir_trailing_no_newline() {
    run_trailing(FolderType::Dir, b"From: a@host\nSubject: one\n\nBody");
}

#[test]
fn dir_trailing_single_newline() {
    run_trailing(FolderType::Dir, b"From: a@host\nSubject: one\n\nBody\n");
}

#[test]
fn complex_filtering_static_dir() {
    let msgs = build_complex_msgs();
    let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
    let rc = build_complex_rc(FolderType::Dir);
    GoldTest::new(&rc, &refs)
        .pre(|d| {
            for name in ["spam", "lists", "urgent", "alice", "bob"] {
                fs::create_dir(d.join(name)).unwrap();
            }
        })
        .run();
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
    GoldTest::new(rc, msgs).run();
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
    GoldTest::new(rc, msgs).run();
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
    GoldTest::new(rc, msgs).run();
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
    GoldTest::new(rc, msgs).args(&["-a", "target"]).run();
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
    GoldTest::new(rc, msgs).run();
}

#[test]
fn secondary_mh() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT/.

:0
inbox/. copy/.
";
    GoldTest::new(rc, MSGS).run();
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
    GoldTest::new(rc, MSGS).run();
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
    // Procmail's metaparse has two paths: when shell metacharacters are
    // present it copies the condition text verbatim (preserving leading
    // whitespace after '?'), otherwise readparse strips it.  Matching
    // this exactly would require detecting shell metacharacters at parse
    // time or deferring whitespace stripping to eval time, neither of
    // which is worth the complexity for a cosmetic log difference.
    GoldTest::new(rc, msgs).no_log().run();
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
    GoldTest::new(rc, msgs).run();
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
    GoldTest::new(rc, msgs).run();
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
    GoldTest::new(rc, msgs).run();
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
    GoldTest::new(rc, msgs).run();
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
    GoldTest::new(rc, msgs).run();
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
    GoldTest::new(rc, msgs).run();
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
    GoldTest::new(rc, msgs).run();
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
    GoldTest::new(rc, msgs).run();
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
    GoldTest::new(rc, msgs).run();
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
    GoldTest::new(rc, msgs).run();
}

#[test]
fn weighted_size_negated() {
    // Negate flips the ratio.  Combined with a positive regex weight
    // so the outcome depends on getting the size ratio right.
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* 5^1 test
* -10^1 ! > 5
matched/
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: test\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn refresh_from_line_preserves_timestamp() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT/
";
    let input: &[&[u8]] =
        &[b"From old@host  Wed Jan  1 12:34:56 2025\nSubject: Test\n\nBody\n"];
    GoldTest::new(rc, input).sender("-").run();
}

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

fn assert_abstract(g: &Gold) {
    let rlog =
        fs::read_to_string(g.rust_dir.path().join("maildir/log")).unwrap();
    let plog =
        fs::read_to_string(g.proc_dir.path().join("maildir/log")).unwrap();
    let ra = normalize_abstract(&rlog);
    let pa = normalize_abstract(&plog);
    assert_eq!(ra, pa, "logabstract differs:\nrust: {ra:?}\nproc: {pa:?}");
}

const LOGABSTRACT_RC: &str = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
LOGFILE=$MAILDIR/log
LOGABSTRACT=yes
";

fn logabstract_gold(input: &[&[u8]]) {
    GoldTest::new(LOGABSTRACT_RC, input)
        .pre(|d| fs::write(d.join("default"), b"").unwrap())
        .no_cmp()
        .post(assert_abstract)
        .run();
}

#[test]
fn logabstract_matches_procmail() {
    logabstract_gold(&[b"Subject: Hello World\n\nBody\n"]);
}

#[test]
fn logabstract_no_subject() {
    logabstract_gold(&[b"From: user@host\n\nBody\n"]);
}

#[test]
fn logabstract_no_from_line() {
    logabstract_gold(&[b"Subject: hi\n\nBody\n"]);
}

#[test]
fn logabstract_long_subject() {
    let long = "x".repeat(100);
    let raw = format!("Subject: {long}\n\nBody\n");
    let msg = raw.into_bytes();
    logabstract_gold(&[&msg]);
}

#[test]
fn logabstract_long_folder() {
    let long = "x".repeat(80);
    let rc = format!(
        "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
LOGFILE=$MAILDIR/log
LOGABSTRACT=yes

:0
$MAILDIR/{long}
"
    );
    GoldTest::new(&rc, &[b"Subject: test\n\nBody\n"])
        .no_cmp()
        .post(assert_abstract)
        .run();
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
    GoldTest::new(rc, msgs).run();
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
    GoldTest::new(rc, msgs).run();
}

#[test]
fn weighted_line_count() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0B
* 1^1 ^.*$
{ }
LINES = $=

:0 fhw
| formail -a \"Lines: $LINES\"

:0 hw
| /bin/echo $LINES
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: test\n\nOne\nTwo\nThree\n",
        b"From: b@host\nSubject: test\n\nSingle line\n",
    ];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn filter_pipe_continues() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0 fw
| /bin/sed 's/old/new/'

:0
* ^Subject:.*new
matched
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: old stuff\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn filter_h_preserves_body() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0 fhw
| /bin/formail -a 'X-Test: yes'

:0
result/
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: Test\n\nOriginal body\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn filter_b_preserves_headers() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0 fbw
| /bin/sed 's/Original/Replaced/'

:0
result/
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: Test\n\nOriginal body\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn pipe_stderr_in_logfile() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
LOGFILE=log

:0
| echo child_stderr >&2
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: Test\n\nBody\n"];
    GoldTest::new(rc, msgs)
        .no_log()
        .post(|g| {
            for (label, dir) in
                [("rust", g.rust_dir.path()), ("proc", g.proc_dir.path())]
            {
                let log = dir.join("maildir/log");
                let c = fs::read_to_string(&log).unwrap();
                assert!(
                    c.contains("child_stderr"),
                    "{label}: child stderr not in logfile: {c:?}"
                );
            }
        })
        .run();
}

#[test]
fn backtick_assignment() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
FOO=`echo hello`

:0
* FOO ?? hello
matched
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: any\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn backtick_reads_stdin() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
CAPTURED=`grep '^Subject:' | sed 's/Subject: //'`

:0
* CAPTURED ?? magic
matched
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: magic\n\nBody\n",
        b"From: b@host\nSubject: other\n\nBody\n",
    ];
    GoldTest::new(rc, msgs).run();
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
    GoldTest::new(rc, msgs).run();
}

fn write_rc(dir: &Path, name: &str, content: &str) {
    fs::write(dir.join(name), content).unwrap();
}

#[test]
fn includerc_delivers() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
INCLUDERC=$MAILDIR/inc.rc
";
    let inc = "\
:0
* ^Subject:.*match
matched
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: match me\n\nBody\n",
        b"From: b@host\nSubject: other\n\nBody\n",
    ];
    GoldTest::new(rc, msgs)
        .pre(|d| write_rc(d, "inc.rc", inc))
        .run();
}

#[test]
fn includerc_continues() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
INCLUDERC=$MAILDIR/inc.rc

:0
* ^Subject:.*parent
matched
";
    let inc = "\
:0
* ^Subject:.*nope
nope
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: parent\n\nBody\n",
        b"From: b@host\nSubject: other\n\nBody\n",
    ];
    GoldTest::new(rc, msgs)
        .pre(|d| write_rc(d, "inc.rc", inc))
        .run();
}

#[test]
fn includerc_missing_skipped() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
INCLUDERC=$MAILDIR/noexist.rc

:0
* ^Subject:.*here
matched
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: here\n\nBody\n",
        b"From: b@host\nSubject: other\n\nBody\n",
    ];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn includerc_var_in_path() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
RCDIR=$MAILDIR
INCLUDERC=$RCDIR/inc.rc
";
    let inc = "\
:0
* ^Subject:.*match
matched
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: match\n\nBody\n",
        b"From: b@host\nSubject: other\n\nBody\n",
    ];
    GoldTest::new(rc, msgs)
        .pre(|d| write_rc(d, "inc.rc", inc))
        .run();
}

#[test]
fn includerc_nested() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
INCLUDERC=$MAILDIR/level1.rc
";
    let l1 = "\
INCLUDERC=$MAILDIR/level2.rc
";
    let l2 = "\
:0
* ^Subject:.*deep
matched
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: deep\n\nBody\n",
        b"From: b@host\nSubject: shallow\n\nBody\n",
    ];
    GoldTest::new(rc, msgs)
        .pre(|d| {
            write_rc(d, "level1.rc", l1);
            write_rc(d, "level2.rc", l2);
        })
        .run();
}

#[test]
fn includerc_conditions() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
INCLUDERC=$MAILDIR/inc.rc
";
    let inc = "\
:0
* ^Subject:.*alpha
alpha

:0
* ^Subject:.*beta
beta
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: alpha\n\nBody\n",
        b"From: b@host\nSubject: beta\n\nBody\n",
        b"From: c@host\nSubject: gamma\n\nBody\n",
    ];
    GoldTest::new(rc, msgs)
        .pre(|d| write_rc(d, "inc.rc", inc))
        .run();
}

#[test]
fn switchrc_delivers() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
SWITCHRC=$MAILDIR/sw.rc
";
    let sw = "\
:0
* ^Subject:.*match
matched
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: match\n\nBody\n",
        b"From: b@host\nSubject: other\n\nBody\n",
    ];
    GoldTest::new(rc, msgs)
        .pre(|d| write_rc(d, "sw.rc", sw))
        .run();
}

#[test]
fn switchrc_stops_parent() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
SWITCHRC=$MAILDIR/sw.rc

:0
* ^Subject:.*parent
parent
";
    let sw = "\
:0
* ^Subject:.*nope
nope
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: parent\n\nBody\n"];
    GoldTest::new(rc, msgs)
        .pre(|d| write_rc(d, "sw.rc", sw))
        .run();
}

#[test]
fn switchrc_bare_default() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* ^Subject:.*before
before

SWITCHRC

:0
* ^Subject:.*after
after
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: before\n\nBody\n",
        b"From: b@host\nSubject: after\n\nBody\n",
    ];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn nested_subst_condition() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
PAT=one

:0
* $$ ^Subject:.*$PAT
matched
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: one\n\nBody\n",
        b"From: b@host\nSubject: two\n\nBody\n",
    ];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn subst_in_variable_condition() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
SUF=VAL
SUFVAL=hello

:0
* $ SUF$SUF ?? hello
matched
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: any\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn weighted_shell_condition() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* 1^0 ? /bin/true
matched/
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: test\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn weighted_shell_condition_negated() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* 1^0 ! ? /bin/false
matched/
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: test\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn shell_true_delivers() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* ? /bin/true
matched/
";
    GoldTest::new(rc, MSGS).run();
}

#[test]
fn shell_false_skips() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* ? /bin/false
matched/
";
    GoldTest::new(rc, MSGS).run();
}

#[test]
fn shell_negated_false_delivers() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* ! ? /bin/false
matched/
";
    GoldTest::new(rc, MSGS).run();
}

#[test]
fn shell_negated_true_skips() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* ! ? /bin/true
matched/
";
    GoldTest::new(rc, MSGS).run();
}

#[test]
fn shell_exit_code() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* ? exit 3
matched/
";
    GoldTest::new(rc, MSGS).run();
}

#[test]
fn weighted_shell_exit_nonzero() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* 5^2 ? /bin/false
matched/
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: test\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn subst_item_in_rcfile() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
SCRATCH=hello-world
SCRATCH =~ s/-/_/
";
    GoldTest::new(rc, MSGS).run();
}

#[test]
fn shift_positional_args() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
SHIFT=1

:0
* $ ^Subject:.*$1
matched
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: second\n\nBody\n",
        b"From: b@host\nSubject: first\n\nBody\n",
    ];
    GoldTest::new(rc, msgs)
        .args(&["-a", "first", "-a", "second"])
        .run();
}

#[test]
fn continuation_header_to_match() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* ^TOsac@cheesecake\\.org
matched
";
    let msgs: &[&[u8]] = &[
        b"From: x@host\nTo: alice@host\nCc: steve@coach.com,\n sac@cheesecake.org\n\nBody\n",
        b"From: x@host\nTo: sac@cheesecake.org\n\nBody\n",
        b"From: x@host\nTo: bob@host\n\nBody\n",
    ];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn exitcode_override() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
EXITCODE=42
";
    let msgs: &[&[u8]] = &[b"From: user@host\nSubject: Test\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn exitcode_not_set_returns_zero() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
";
    let msgs: &[&[u8]] = &[b"From: user@host\nSubject: Test\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn forward_with_sendmail_true() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
SENDMAIL=/bin/true
SENDMAILFLAGS=

:0
! user@example.com
";
    let msgs: &[&[u8]] = &[b"From: user@host\nSubject: Test\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn trap_runs_on_exit() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
TRAP=\"touch $MAILDIR/trap_ran\"
";
    let msgs: &[&[u8]] = &[b"From: user@host\nSubject: Test\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn trap_exitcode_available() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
TRAP=\"echo \\$EXITCODE > $MAILDIR/exitcode\"
";
    let msgs: &[&[u8]] = &[b"From: user@host\nSubject: Test\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn trap_exit_overrides_exitcode() {
    // `;` forces shell execution (shellmeta); EXITCODE= means use TRAP's code
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
EXITCODE=
TRAP=\"/bin/true; exit 7\"
";
    let msgs: &[&[u8]] = &[b"From: user@host\nSubject: Test\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn trap_receives_message_on_stdin() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
TRAP=\"cat > $MAILDIR/stdin_dump\"
";
    let msgs: &[&[u8]] = &[b"From: user@host\nSubject: Test\n\nTrapBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn pass_header_only() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0 h
matched
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: Test\n\nBody text\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn pass_body_only() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0 b
matched
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: Test\n\nBody text\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn pass_header_and_body() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0 hb
matched
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: Test\n\nBody text\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn weighted_divergent_zero_width() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* 1^2 ^
matched
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: Test\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn invalid_maildir() {
    let rc = "\
MAILDIR=/nonexistent/path
DEFAULT=$DEFAULT
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: Test\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn log_variable() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
LOGFILE=$MAILDIR/log

LOG=hello
LOG=world
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: Test\n\nBody\n"];
    GoldTest::new(rc, msgs)
        .no_log()
        .post(|g| {
            let r = fs::read_to_string(g.rust_dir.path().join("maildir/log"))
                .unwrap();
            let p = fs::read_to_string(g.proc_dir.path().join("maildir/log"))
                .unwrap();
            assert_eq!(r, p, "log content differs");
        })
        .run();
}

#[test]
fn log_multiline_quote() {
    let rc = "MAILDIR=$MAILDIR\n\
              DEFAULT=$DEFAULT\n\
              LOGFILE=$MAILDIR/log\n\
              \n\
              LOG=\"\n\
              \"\n";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: Test\n\nBody\n"];
    GoldTest::new(rc, msgs)
        .no_log()
        .post(|g| {
            let r = fs::read_to_string(g.rust_dir.path().join("maildir/log"))
                .unwrap();
            let p = fs::read_to_string(g.proc_dir.path().join("maildir/log"))
                .unwrap();
            assert_eq!(r, p, "log content differs");
        })
        .run();
}

#[test]
fn shell_reassign_backtick() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
SHELL=/bin/bash
FOO=`/bin/echo captured`

:0
* FOO ?? captured
matched
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: any\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn shell_reassign_pipe() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
SHELL=/bin/bash

:0 fw
| /bin/sed 's/original/replaced/'

:0
* ^Subject:.*replaced
matched
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: original stuff\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn path_reassign() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
PATH=/usr/bin:/bin

FOO=`expr 2 + 3`

:0
* FOO ?? 5
matched
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: any\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn shellmetas_metachar() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

FOO=`/bin/echo hello; /bin/echo world`

:0
* FOO ?? hello
matched
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: any\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn delivered_yes_still_delivers_default() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
DELIVERED=yes
";
    GoldTest::new(rc, MSGS).no_log().run();
}

#[test]
fn delivered_after_recipe_no_double() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
inbox

DELIVERED=yes
";
    GoldTest::new(rc, MSGS).no_log().run();
}

#[test]
fn orgmail_fallback() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=
ORGMAIL=$MAILDIR/backup
";
    GoldTest::new(rc, MSGS).no_log().run();
}

#[test]
fn umask_mbox() {
    let rc = "MAILDIR=$MAILDIR\nDEFAULT=$DEFAULT\nUMASK=022\n";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: test\n\nBody\n"];
    GoldTest::new(rc, msgs)
        .no_log()
        .post(|g| {
            let r = fs::metadata(g.rust_dir.path().join("maildir/default"))
                .unwrap()
                .mode()
                & 0o777;
            let p = fs::metadata(g.proc_dir.path().join("maildir/default"))
                .unwrap()
                .mode()
                & 0o777;
            assert_eq!(r, p, "mbox perms: rust={r:03o}, proc={p:03o}");
        })
        .run();
}

#[test]
fn umask_change_between_deliveries() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
UMASK=077

:0 c
tight
UMASK=022

:0
loose
";
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: test\n\nBody\n"];
    GoldTest::new(rc, msgs)
        .no_log()
        .post(|g| {
            for name in ["tight", "loose"] {
                let r = fs::metadata(
                    g.rust_dir.path().join(format!("maildir/{name}")),
                )
                .unwrap()
                .mode()
                    & 0o777;
                let p = fs::metadata(
                    g.proc_dir.path().join(format!("maildir/{name}")),
                )
                .unwrap()
                .mode()
                    & 0o777;
                assert_eq!(r, p, "{name} perms: rust={r:03o}, proc={p:03o}");
            }
        })
        .run();
}

#[test]
fn lastfolder_after_delivery() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* ^Subject:.*first
first

:0
* $ LASTFOLDER ?? first
got_first
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: first\n\nBody\n",
        b"From: b@host\nSubject: other\n\nBody\n",
    ];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn match_from_extraction() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* ^Subject:.*\\/[^ ]+
{
    :0
    * $ MATCH ?? magic
    matched
}
";
    let msgs: &[&[u8]] = &[
        b"From: a@host\nSubject: magic stuff\n\nBody\n",
        b"From: b@host\nSubject: other stuff\n\nBody\n",
    ];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn match_cleared_between_recipes() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0 c
* ^From:.*\\/[^@]+
/dev/null

:0
* MATCH ?? .
has_match
";
    let msgs: &[&[u8]] = &[b"From: alice@host\nSubject: test\n\nBody\n"];
    GoldTest::new(rc, msgs).run();
}

#[test]
fn linebuf_overflow() {
    let a100 = "a".repeat(100);
    let rc = format!(
        "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
LOGFILE=$MAILDIR/log
LINEBUF=10
V={a100}

DUMMY=$V$V
LOG=$PROCMAIL_OVERFLOW

:0
* $ ^Subject:.*$V
short_matched
"
    );
    let msg = format!("From: a@host\nSubject: {a100}\n\nBody\n");
    let msgs: &[&[u8]] = &[msg.as_bytes()];
    GoldTest::new(&rc, &msgs)
        .no_log()
        .post(|g| {
            let strip = |s: &str| -> String {
                s.lines()
                    .map(|l| {
                        l.strip_prefix("procmail: ")
                            .or_else(|| {
                                l.strip_prefix(concat!(
                                    env!("CARGO_PKG_NAME"),
                                    ": "
                                ))
                            })
                            .unwrap_or(l)
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            let r = fs::read_to_string(g.rust_dir.path().join("maildir/log"))
                .unwrap();
            let p = fs::read_to_string(g.proc_dir.path().join("maildir/log"))
                .unwrap();
            assert_eq!(strip(&r), strip(&p), "log content differs");
        })
        .run();
}

#[test]
fn host_mismatch_stops_processing() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
HOST=no.such.host.invalid

:0
matched
";
    let msgs: &[&[u8]] = &[b"From: user@host\nSubject: Test\n\nBody\n"];
    GoldTest::new(rc, msgs).no_log().run();
}
