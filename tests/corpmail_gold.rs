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

fn run_gold_with(rc_template: &str, input: &[u8], cmp: fn(&Path, &Path)) {
    let g = Gold::new();
    setup(g.rust_dir.path(), rc_template);
    setup(g.proc_dir.path(), rc_template);
    let rc_r = g.rust_dir.path().join("rcfile");
    let rc_p = g.proc_dir.path().join("rcfile");
    let args_r: Vec<&str> = vec!["-f", "sender@test", rc_r.to_str().unwrap()];
    let args_p: Vec<&str> = vec!["-f", "sender@test", rc_p.to_str().unwrap()];
    let (_, rc) = run(g.rust_dir.path(), corpmail(), &args_r, input);
    let (_, pc) = run(g.proc_dir.path(), &procmail(), &args_p, input);
    assert_eq!(rc, pc, "exit codes differ: rust={rc}, proc={pc}");
    cmp(
        &g.rust_dir.path().join("maildir"),
        &g.proc_dir.path().join("maildir"),
    );
}

fn run_gold(rc_template: &str, input: &[u8]) {
    run_gold_with(rc_template, input, assert_dirs_eq);
}

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
        } else if !p.to_string_lossy().ends_with(".lock") {
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

:0
inbox
",
        b"From: user@host\nSubject: Test\n\nBody\n",
    );
}

#[test]
fn deliver_maildir() {
    run_gold_with(
        "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
inbox/
",
        b"From: user@host\nSubject: Test\n\nBody\n",
        assert_dirs_eq_contents,
    );
}

#[test]
fn deliver_mh() {
    run_gold(
        "\
MAILDIR=$MAILDIR
DEFAULT=$DEFAULT

:0
inbox/.
",
        b"From: user@host\nSubject: Test\n\nBody\n",
    );
}
