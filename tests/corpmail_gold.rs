//! Gold standard tests comparing Rust corpmail against procmail.
//!
//! Run with:
//!     PROCMAIL_PROCMAIL=/bin/procmail \
//!         cargo test --features gold --test corpmail_gold

#![cfg(feature = "gold")]

#[allow(unused)]
mod common;

use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::{env, fs, panic, process};

use common::{Gold, normalize_from_line, run};
use rand::Rng;
use rand::seq::SliceRandom;

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

fn run_gold_with<S>(
    rc_template: S, inputs: &[&[u8]], count: Option<usize>,
    cmp: fn(&Path, &Path),
) where
    S: Borrow<str>,
{
    let rc_template = rc_template.borrow();
    let g = Gold::new();
    setup(g.rust_dir.path(), rc_template);
    setup(g.proc_dir.path(), rc_template);

    let result = panic::catch_unwind(AssertUnwindSafe(|| {
        run_gold_inner(&g, inputs, count, cmp);
    }));
    if let Err(e) = result {
        let dir = preserve_failure(&g, rc_template, inputs);
        eprintln!("preserved failure artifacts in {}", dir.display());
        panic::resume_unwind(e);
    }
}

fn run_gold_inner(
    g: &Gold, inputs: &[&[u8]], count: Option<usize>, cmp: fn(&Path, &Path),
) {
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
    if let Some(count) = count {
        let n = file_count(r);
        assert_eq!(n, count, "expected {count} files in maildir, got {n}");
    }
    cmp(r, &g.proc_dir.path().join("maildir"));
}

fn preserve_failure(g: &Gold, rc_template: &str, inputs: &[&[u8]]) -> PathBuf {
    let dir =
        env::temp_dir().join(format!("corpmail-gold-fail-{}", process::id(),));
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

fn run_gold(rc_template: &str, inputs: &[&[u8]], count: usize) {
    run_gold_with(rc_template, inputs, Some(count), assert_dirs_eq);
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
        Some(5),
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

fn build_complex_rc(suffix: &str) -> String {
    let s = suffix;
    let mut rc = format!(
        "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* ^X-Spam-Flag: YES
spam{s}

:0
* ^TO_lists\\.net
lists{s}

:0 c
* ^X-Priority: 1
urgent{s}

"
    );
    for addr in &ADDRS[..2] {
        let name = addr.split('@').next().unwrap();
        let escaped = addr.replace('.', "\\.");
        rc += &format!(":0\n* ^From:.*{escaped}\n{name}{s}\n\n");
    }
    rc += &format!(":0\n* ^Subject:.*URGENT\nurgent{s}\n");
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
fn complex_filtering_static_mbox() {
    let msgs = build_complex_msgs();
    let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
    // 10 messages + 2 copies across 6 destinations
    run_gold(&build_complex_rc(""), &refs, 6);
}

#[test]
fn complex_filtering_static_maildir() {
    let msgs = build_complex_msgs();
    let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
    run_gold_with(build_complex_rc("/"), &refs, None, assert_dirs_eq_contents);
}

#[test]
fn complex_filtering_static_mh() {
    let msgs = build_complex_msgs();
    let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
    run_gold_with(build_complex_rc("/."), &refs, None, assert_dirs_eq);
}

fn build_random_rc(rng: &mut impl Rng, suffix: &str) -> String {
    let s = suffix;
    let mut rc = "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

"
    .to_string();
    if rng.gen_bool(0.5) {
        rc += &format!(":0\n* ^X-Spam-Flag: YES\nspam{s}\n\n");
    }
    if rng.gen_bool(0.5) {
        rc += &format!(":0\n* ^TO_lists\\.net\nlists{s}\n\n");
    }
    if rng.gen_bool(0.5) {
        rc += &format!(":0 c\n* ^X-Priority: 1\nurgent{s}\n\n");
    }
    let n = rng.gen_range(1..=ADDRS.len());
    let mut addrs: Vec<_> = ADDRS.to_vec();
    addrs.shuffle(rng);
    for addr in &addrs[..n] {
        let name = addr.split('@').next().unwrap();
        let escaped = addr.replace('.', "\\.");
        rc += &format!(":0\n* ^From:.*{escaped}\n{name}{s}\n\n");
    }
    if rng.gen_bool(0.5) {
        rc += &format!(":0\n* ^Subject:.*URGENT\nurgent{s}\n");
    }
    rc
}

fn build_random_msgs(rng: &mut impl Rng) -> Vec<Vec<u8>> {
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

fn run_random(suffix: &str, cmp: fn(&Path, &Path)) {
    let mut rng = rand::thread_rng();
    let rc = build_random_rc(&mut rng, suffix);
    let msgs = build_random_msgs(&mut rng);
    let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
    run_gold_with(rc, &refs, None, cmp);
}

#[test]
fn complex_filtering_random_mbox() {
    run_random("", assert_dirs_eq);
}

#[test]
fn complex_filtering_random_maildir() {
    run_random("/", assert_dirs_eq_contents);
}

#[test]
fn complex_filtering_random_mh() {
    run_random("/.", assert_dirs_eq);
}

fn build_size_msgs(rng: &mut impl Rng) -> Vec<Vec<u8>> {
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

fn build_size_rc(rng: &mut impl Rng) -> String {
    let thresh = rng.gen_range(100..=300);
    let mut rc = format!(
        "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
* > {thresh}
big

:0
* < {thresh}
small

"
    );
    // Negated From rule: deliver to "other" if NOT from a random addr
    let addr = ADDRS.choose(rng).unwrap();
    let escaped = addr.replace('.', "\\.");
    rc += &format!(":0\n* ! ^From:.*{escaped}\nother\n");
    rc
}

#[test]
fn size_and_negation() {
    let mut rng = rand::thread_rng();
    let rc = build_size_rc(&mut rng);
    let msgs = build_size_msgs(&mut rng);
    let refs: Vec<&[u8]> = msgs.iter().map(|m| m.as_slice()).collect();
    run_gold_with(rc, &refs, None, assert_dirs_eq);
}
