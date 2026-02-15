//! Gold-test infrastructure: GoldTest builder, failure preservation,
//! and dictionary constants for generating test messages.

#![allow(unused)]

use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::{fs, panic, process};

use crate::common::{Gold, bin_dir, diff_dirs, procmail, rockmail, run, setup};

pub const MSGS: &[&[u8]] = &[
    b"From: a@host\nSubject: one\n\nBody one\n",
    b"From: b@host\nSubject: two\n\nBody two\n",
    b"From: c@host\nSubject: three\n\nBody three\n",
    b"From: d@host\nSubject: four\n\nBody four\n",
    b"From: e@host\nSubject: five\n\nBody five\n",
];

pub const ADDRS: &[&str] = &[
    "alice@example.com",
    "bob@work.org",
    "carol@lists.net",
    "dave@spam.biz",
    "eve@friend.io",
];

pub const SUBJECTS: &[&str] = &[
    "Meeting tomorrow",
    "URGENT deal",
    "Re: project update",
    "Newsletter #42",
    "Invitation to connect",
];

pub const LISTS: &[&str] =
    &["dev@lists.net", "announce@lists.net", "security@lists.net"];

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

fn preserve_failure(g: &Gold, rc: &str, inputs: &[&[u8]]) -> PathBuf {
    let dir = PathBuf::from("tmp")
        .join(format!("rockmail-gold-fail-{}", process::id()));
    let _ = fs::create_dir_all(&dir);
    fs::write(dir.join("rcfile"), rc).ok();
    for (i, msg) in inputs.iter().enumerate() {
        fs::write(dir.join(format!("msg-{i:02}")), msg).ok();
    }
    let rust_out = dir.join("rust");
    let proc_out = dir.join("proc");
    copy_dir(&g.rust_dir.path().join("maildir"), &rust_out);
    copy_dir(&g.proc_dir.path().join("maildir"), &proc_out);
    dir
}

#[allow(clippy::type_complexity)]
pub struct GoldTest<'a> {
    rc: &'a str,
    inputs: &'a [&'a [u8]],
    args: Vec<&'a str>,
    sender: &'a str,
    cmp: Option<Box<dyn Fn(&Path, &Path) + 'a>>,
    pre: Option<Box<dyn Fn(&Path) + 'a>>,
    post: Option<Box<dyn FnOnce(&Gold) + 'a>>,
}

impl<'a> GoldTest<'a> {
    pub fn new(rc: &'a str, inputs: &'a [&'a [u8]]) -> Self {
        Self {
            rc,
            inputs,
            args: Vec::new(),
            sender: "sender@test",
            cmp: Some(Box::new(|a, b| diff_dirs(a, b).unwrap())),
            pre: None,
            post: None,
        }
    }

    pub fn args(mut self, extra: &[&'a str]) -> Self {
        self.args.extend_from_slice(extra);
        self
    }

    pub fn sender(mut self, s: &'a str) -> Self {
        self.sender = s;
        self
    }

    pub fn no_cmp(mut self) -> Self {
        self.cmp = None;
        self
    }

    pub fn pre(mut self, f: impl Fn(&Path) + 'a) -> Self {
        self.pre = Some(Box::new(f));
        self
    }

    pub fn post(mut self, f: impl FnOnce(&Gold) + 'a) -> Self {
        self.post = Some(Box::new(f));
        self
    }

    pub fn run(self) {
        let g = Gold::new();
        setup(g.rust_dir.path(), self.rc, Some(bin_dir()));
        setup(g.proc_dir.path(), self.rc, None);
        if let Some(ref pre) = self.pre {
            pre(&g.rust_dir.path().join("maildir"));
            pre(&g.proc_dir.path().join("maildir"));
        }

        let rc = self.rc;
        let inputs = self.inputs;
        let result = panic::catch_unwind(AssertUnwindSafe(|| {
            self.run_inner(&g);
        }));
        if let Err(e) = result {
            let dir = preserve_failure(&g, rc, inputs);
            eprintln!("preserved failure artifacts in {}", dir.display());
            panic::resume_unwind(e);
        }
    }

    fn run_inner(self, g: &Gold) {
        let rc_r = g.rust_dir.path().join("rcfile");
        let rc_p = g.proc_dir.path().join("rcfile");
        let mut args_r = vec!["-f", self.sender];
        let mut args_p = vec!["-f", self.sender];
        args_r.extend_from_slice(&self.args);
        args_p.extend_from_slice(&self.args);
        args_r.push(rc_r.to_str().unwrap());
        args_p.push(rc_p.to_str().unwrap());
        for input in self.inputs {
            let (_, rc) = run(g.rust_dir.path(), rockmail(), &args_r, input);
            let (_, pc) = run(g.proc_dir.path(), procmail(), &args_p, input);
            assert_eq!(rc, pc, "exit codes differ: rust={rc}, proc={pc}");
        }
        if let Some(cmp) = &self.cmp {
            cmp(
                &g.rust_dir.path().join("maildir"),
                &g.proc_dir.path().join("maildir"),
            );
        }
        if let Some(post) = self.post {
            post(g);
        }
    }
}
