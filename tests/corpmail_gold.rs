//! Gold standard tests comparing Rust corpmail against procmail.
//!
//! Run with:
//!     PROCMAIL_PROCMAIL=/bin/procmail \
//!         cargo test --features gold --test corpmail_gold

#![cfg(feature = "gold")]

#[allow(unused)]
mod common;

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use common::{Gold, normalize_from_line, run};

fn procmail() -> String {
    std::env::var("PROCMAIL_PROCMAIL")
        .expect("PROCMAIL_PROCMAIL env var required")
}

fn corpmail() -> &'static str {
    env!("CARGO_BIN_EXE_corpmail")
}

fn setup(dir: &Path, rc_template: &str) {
    let maildir = dir.join("maildir");
    fs::create_dir(&maildir).unwrap();
    let default = maildir.join("default");
    let rc = rc_template
        .replace("$MAILDIR", maildir.to_str().unwrap())
        .replace("$DEFAULT", default.to_str().unwrap());
    let path = dir.join("rcfile");
    fs::write(&path, rc).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
}

fn run_gold_with(
    rc_template: &str, inputs: &[&[u8]], count: usize, cmp: fn(&Path, &Path),
) {
    let g = Gold::new();
    setup(g.rust_dir.path(), rc_template);
    setup(g.proc_dir.path(), rc_template);
    let rc_r = g.rust_dir.path().join("rcfile");
    let rc_p = g.proc_dir.path().join("rcfile");
    let args_r: Vec<&str> = vec!["-f", "sender@test", rc_r.to_str().unwrap()];
    let args_p: Vec<&str> = vec!["-f", "sender@test", rc_p.to_str().unwrap()];
    for input in inputs {
        let (_, rc) = run(g.rust_dir.path(), corpmail(), &args_r, input);
        let (_, pc) = run(g.proc_dir.path(), &procmail(), &args_p, input);
        assert_eq!(rc, pc, "exit codes differ: rust={rc}, proc={pc}");
    }
    let r = &g.rust_dir.path().join("maildir");
    let n = file_count(r);
    assert_eq!(n, count, "expected {count} files in maildir, got {n}");
    cmp(r, &g.proc_dir.path().join("maildir"));
}

fn run_gold(rc_template: &str, inputs: &[&[u8]], count: usize) {
    run_gold_with(rc_template, inputs, count, assert_dirs_eq);
}

fn file_count(dir: &Path) -> usize {
    snapshot(dir).len()
}

const MSGS: &[&[u8]] = &[
    b"From: a@host\nSubject: one\n\nBody one\n",
    b"From: b@host\nSubject: two\n\nBody two\n",
    b"From: c@host\nSubject: three\n\nBody three\n",
    b"From: d@host\nSubject: four\n\nBody four\n",
    b"From: e@host\nSubject: five\n\nBody five\n",
];

/// Compare two directory trees, asserting identical file names and contents.
fn assert_dirs_eq(rust: &Path, proc: &Path) {
    let r = snapshot(rust);
    let p = snapshot(proc);
    let rk: Vec<_> = r.keys().collect();
    let pk: Vec<_> = p.keys().collect();
    assert_eq!(rk, pk, "file sets differ:\n  rust: {rk:?}\n  proc: {pk:?}");
    for (path, rd) in &r {
        assert_contents_eq(path, rd, &p[path]);
    }
}

/// Compare two directory trees ignoring file names (for maildir, where
/// filenames contain timestamps/PIDs).  Asserts identical sorted contents.
fn assert_dirs_eq_contents(rust: &Path, proc: &Path) {
    let mut r: Vec<_> = snapshot(rust).into_values().collect();
    let mut p: Vec<_> = snapshot(proc).into_values().collect();
    assert_eq!(
        r.len(),
        p.len(),
        "file count differs: rust={}, proc={}",
        r.len(),
        p.len()
    );
    r.sort();
    p.sort();
    for (i, (rd, pd)) in r.iter().zip(p.iter()).enumerate() {
        assert_contents_eq(&format!("file #{i}"), rd, pd);
    }
}

fn assert_contents_eq(label: &str, rust: &[u8], proc: &[u8]) {
    let rn = normalize_from_line(rust);
    let pn = normalize_from_line(proc);
    if rn != pn {
        panic!(
            "contents differ for {label}:\
             \n--- rust ({} bytes) ---\n{:?}\
             \n--- proc ({} bytes) ---\n{:?}",
            rn.len(),
            String::from_utf8_lossy(&rn),
            pn.len(),
            String::from_utf8_lossy(&pn),
        );
    }
}

fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut map = BTreeMap::new();
    if dir.exists() {
        walk(dir, dir, &mut map);
    }
    map
}

fn walk(base: &Path, dir: &Path, map: &mut BTreeMap<String, Vec<u8>>) {
    for e in fs::read_dir(dir).unwrap() {
        let e = e.unwrap();
        let p = e.path();
        if p.is_dir() {
            walk(base, &p, map);
        } else {
            let rel = p.strip_prefix(base).unwrap();
            let key = rel.to_string_lossy().into_owned();
            map.insert(key, fs::read(&p).unwrap());
        }
    }
}

#[test]
fn deliver_mbox() {
    run_gold(
        "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
LOG=log

:0
inbox
",
        MSGS,
        1,
    );
}

#[test]
fn deliver_maildir() {
    run_gold_with(
        "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
LOG=log

:0
inbox/
",
        MSGS,
        5,
        assert_dirs_eq_contents,
    );
}

#[test]
fn deliver_mh() {
    run_gold(
        "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
LOG=log

:0
inbox/.
",
        MSGS,
        5,
    );
}

#[test]
fn deliver_dev_null() {
    run_gold(
        "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT
LOG=log

:0
/dev/null
",
        MSGS,
        0,
    );
}

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

fn build_complex_rc() -> String {
    let mut rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* ^X-Spam-Flag: YES
spam

:0:
* ^TO_lists\\.net
lists

:0 c:
* ^X-Priority: 1
urgent

"
    .to_string();
    for addr in &ADDRS[..2] {
        let name = addr.split('@').next().unwrap();
        let escaped = addr.replace('.', "\\.");
        rc += &format!(":0:\n* ^From:.*{escaped}\n{name}\n\n");
    }
    rc += ":0:\n* ^Subject:.*URGENT\nurgent\n";
    rc
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

#[test]
fn complex_filtering() {
    let rc = build_complex_rc();
    let msgs = build_complex_msgs();
    let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
    // Files: spam (2), lists (2), urgent (3), alice (2), bob (2), default (1)
    run_gold(&rc, &refs, 6);
}
