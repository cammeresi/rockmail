//! Regression tests from preserved gold-test failures.
//!
//! Each `.tar.gz` in `tests/regressions/` becomes a separate test case.
//! Without the `gold` feature, compares corpmail output against stored
//! procmail output.  With `gold`, runs procmail live instead.

use std::fs::{self, File};
use std::path::Path;

use flate2::read::GzDecoder;
use libtest_mimic::{Arguments, Failed, Trial};
use tar::Archive;

#[cfg(feature = "gold")]
use common::procmail;
use common::{corpmail, diff_dirs, run, setup};

#[allow(unused)]
mod common;

fn collect_msgs(root: &Path) -> Result<Vec<(String, Vec<u8>)>, Failed> {
    let mut msgs = Vec::new();
    for e in fs::read_dir(root).map_err(|e| format!("readdir: {e}"))? {
        let e = e.map_err(|e| format!("entry: {e}"))?;
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("msg-") {
            let data =
                fs::read(e.path()).map_err(|e| format!("{name}: {e}"))?;
            msgs.push((name, data));
        }
    }
    msgs.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(msgs)
}

#[cfg(not(feature = "gold"))]
fn compare(
    root: &Path, _rc_tmpl: &str, rdir: &Path, msgs: &[(String, Vec<u8>)],
    ra: &[&str],
) -> Result<(), Failed> {
    let corpmail = corpmail();
    for (_, data) in msgs {
        run(rdir, corpmail, ra, data);
    }
    diff_dirs(&rdir.join("maildir"), &root.join("proc")).map_err(|e| e.into())
}

#[cfg(feature = "gold")]
fn compare(
    _root: &Path, rc_tmpl: &str, rdir: &Path, msgs: &[(String, Vec<u8>)],
    ra: &[&str],
) -> Result<(), Failed> {
    let pdir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    setup(pdir.path(), rc_tmpl);
    let prc = pdir.path().join("rcfile");
    let pa = vec!["-f", "sender@test", prc.to_str().unwrap()];
    for (name, data) in msgs {
        let (_, rerr) = run(rdir, corpmail(), ra, data);
        let (_, perr) = run(pdir.path(), procmail(), &pa, data);
        if rerr != perr {
            return Err(format!(
                "{name}: exit codes differ: rust={rerr}, proc={perr}"
            )
            .into());
        }
    }
    diff_dirs(&rdir.join("maildir"), &pdir.path().join("maildir"))
        .map_err(|e| e.into())
}

fn replay(tarball: &Path) -> Result<(), Failed> {
    let tmp = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    let f = File::open(tarball).map_err(|e| format!("open: {e}"))?;
    let mut ar = Archive::new(GzDecoder::new(f));
    ar.unpack(tmp.path()).map_err(|e| format!("unpack: {e}"))?;

    let root = tmp.path();
    let rc_tmpl = fs::read_to_string(root.join("rcfile"))
        .map_err(|e| format!("rcfile: {e}"))?;
    let msgs = collect_msgs(root)?;

    let rdir = tempfile::tempdir().map_err(|e| format!("tempdir: {e}"))?;
    setup(rdir.path(), &rc_tmpl);
    let rrc = rdir.path().join("rcfile");
    let ra: Vec<&str> = vec!["-f", "sender@test", rrc.to_str().unwrap()];

    compare(root, &rc_tmpl, rdir.path(), &msgs, &ra)
}

fn tarball_name(path: &Path) -> Option<String> {
    let s = path.file_name()?.to_str()?;
    s.strip_suffix(".tar.gz").map(String::from)
}

fn main() {
    let args = Arguments::from_args();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/regressions");
    let mut trials = Vec::new();

    if let Ok(entries) = fs::read_dir(&dir) {
        for e in entries.flatten() {
            let path = e.path();
            let Some(name) = tarball_name(&path) else {
                continue;
            };
            let p = path.clone();
            trials.push(Trial::test(name, move || replay(&p)));
        }
    }
    trials.sort_by(|a, b| a.name().cmp(b.name()));
    libtest_mimic::run(&args, trials).exit();
}
