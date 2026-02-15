//! Gold standard tests comparing Rust rockmail against procmail.
//!
//! Run with:
//!     cargo test --features gold --test rockmail_gold
//!
//! The original procmail binary is found automatically.  To override,
//! set PROCMAIL_PROCMAIL to the path of the original procmail.

#![cfg(feature = "gold")]

use std::fs;

use rand::Rng;
use rand::seq::SliceRandom;

use rockmail::delivery::FolderType;

#[allow(unused)]
mod common;
mod gold;

use common::RcBuilder;
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

#[test]
fn complex_filtering_static_mbox() {
    let msgs = build_complex_msgs();
    let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
    let rc = build_complex_rc(FolderType::File);
    GoldTest::new(&rc, &refs).run();
}

#[test]
fn complex_filtering_static_maildir() {
    let msgs = build_complex_msgs();
    let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
    let rc = build_complex_rc(FolderType::Maildir);
    GoldTest::new(&rc, &refs).run();
}

#[test]
fn complex_filtering_static_mh() {
    let msgs = build_complex_msgs();
    let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
    let rc = build_complex_rc(FolderType::Mh);
    GoldTest::new(&rc, &refs).run();
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

#[test]
fn mh_trailing_blank() {
    let rc = RcBuilder::new(FolderType::Mh).folder("inbox").build();
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: one\n\nBody\n\n"];
    GoldTest::new(&rc, msgs).run();
}

#[test]
fn mh_trailing_no_newline() {
    let rc = RcBuilder::new(FolderType::Mh).folder("inbox").build();
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: one\n\nBody"];
    GoldTest::new(&rc, msgs).run();
}

#[test]
fn mh_trailing_single_newline() {
    let rc = RcBuilder::new(FolderType::Mh).folder("inbox").build();
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: one\n\nBody\n"];
    GoldTest::new(&rc, msgs).run();
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
    let rc = RcBuilder::new(FolderType::Dir).folder("inbox").build();
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: one\n\nBody\n\n"];
    run_dir_gold(&rc, msgs);
}

#[test]
fn dir_trailing_no_newline() {
    let rc = RcBuilder::new(FolderType::Dir).folder("inbox").build();
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: one\n\nBody"];
    run_dir_gold(&rc, msgs);
}

#[test]
fn dir_trailing_single_newline() {
    let rc = RcBuilder::new(FolderType::Dir).folder("inbox").build();
    let msgs: &[&[u8]] = &[b"From: a@host\nSubject: one\n\nBody\n"];
    run_dir_gold(&rc, msgs);
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
    GoldTest::new(rc, msgs).run();
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

#[test]
fn logabstract_matches_procmail() {
    let rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
LOGFILE=$MAILDIR/log
LOGABSTRACT=yes
";
    let input: &[&[u8]] = &[b"Subject: Hello World\n\nBody\n"];
    GoldTest::new(rc, input)
        .pre(|d| fs::write(d.join("default"), b"").unwrap())
        .no_cmp()
        .post(|g| {
            let rlog =
                fs::read_to_string(g.rust_dir.path().join("maildir/log"))
                    .unwrap();
            let plog =
                fs::read_to_string(g.proc_dir.path().join("maildir/log"))
                    .unwrap();
            let ra = normalize_abstract(&rlog);
            let pa = normalize_abstract(&plog);
            assert_eq!(
                ra, pa,
                "logabstract differs:\nrust: {ra:?}\nproc: {pa:?}"
            );
        })
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
