//! Shared infrastructure for gold standard tests.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{ErrorKind, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use tempfile::TempDir;

use corpmail::delivery::FolderType;

pub fn corpmail() -> &'static str {
    env!("CARGO_BIN_EXE_corpmail")
}

pub fn procmail() -> &'static str {
    static P: OnceLock<String> = OnceLock::new();
    P.get_or_init(|| {
        env::var("PROCMAIL_PROCMAIL")
            .expect("PROCMAIL_PROCMAIL env var required")
    })
}

/// Run a binary with args and stdin, returning (stdout, exit_code).
pub fn run(
    dir: &Path, bin: &str, args: &[&str], input: &[u8],
) -> (Vec<u8>, i32) {
    let mut child = Command::new(bin)
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn {bin}: {e}"));

    if let Err(e) = child.stdin.take().unwrap().write_all(input)
        && e.kind() != ErrorKind::BrokenPipe
    {
        panic!("failed to write to stdin: {e}");
    }
    let out = child.wait_with_output().expect("failed to wait");
    (out.stdout, out.status.code().unwrap_or(-1))
}

/// Set up a directory with a maildir and an rcfile expanded from a template.
pub fn setup(dir: &Path, tmpl: &str) {
    let maildir = dir.join("maildir");
    fs::create_dir(&maildir).unwrap();
    let rc = tmpl
        .replace("$MAILDIR", maildir.to_str().unwrap())
        .replace("$DEFAULT", maildir.join("default").to_str().unwrap());
    let p = dir.join("rcfile");
    fs::write(&p, &rc).unwrap();
    fs::set_permissions(&p, fs::Permissions::from_mode(0o644)).unwrap();
}

/// Result of running both Rust and procmail implementations.
#[must_use]
pub struct GoldResult {
    pub rust_out: Vec<u8>,
    pub rust_code: i32,
    pub proc_out: Vec<u8>,
    pub proc_code: i32,
}

impl GoldResult {
    pub fn assert_codes_eq(rust: i32, proc: i32) {
        assert_eq!(
            rust, proc,
            "exit codes differ: rust={}, proc={}",
            rust, proc
        );
    }

    pub fn assert_output_eq(rust: &[u8], proc: &[u8]) {
        if rust != proc {
            panic!(
                "stdout differs:\n--- rust ---\n{}\n--- proc ---\n{}",
                String::from_utf8_lossy(rust),
                String::from_utf8_lossy(proc)
            );
        }
    }

    pub fn assert_eq(self) {
        Self::assert_codes_eq(self.rust_code, self.proc_code);
        Self::assert_output_eq(&self.rust_out, &self.proc_out);
    }

    pub fn assert_eq_with<F>(self, norm: F)
    where
        F: Fn(&[u8]) -> Vec<u8>,
    {
        Self::assert_codes_eq(self.rust_code, self.proc_code);
        Self::assert_output_eq(&norm(&self.rust_out), &norm(&self.proc_out));
    }
}

/// Paired temp directories for running Rust and procmail side by side.
pub struct Gold {
    pub rust_dir: TempDir,
    pub proc_dir: TempDir,
}

impl Gold {
    pub fn new() -> Self {
        Self {
            rust_dir: TempDir::new().unwrap(),
            proc_dir: TempDir::new().unwrap(),
        }
    }

    /// Run with fresh temp directories (for single-run tests).
    pub fn run_once(
        rust_bin: &str, proc_bin: &str, args: &[&str], input: &[u8],
    ) -> GoldResult {
        Gold::new().run(rust_bin, proc_bin, args, input)
    }

    /// Run both binaries with the same args and input, return a
    /// `GoldResult` comparing their stdout and exit codes.
    pub fn run(
        &self, rust_bin: &str, proc_bin: &str, args: &[&str], input: &[u8],
    ) -> GoldResult {
        let (rust_out, rust_code) =
            run(self.rust_dir.path(), rust_bin, args, input);
        let (proc_out, proc_code) =
            run(self.proc_dir.path(), proc_bin, args, input);
        GoldResult {
            rust_out,
            rust_code,
            proc_out,
            proc_code,
        }
    }
}

pub fn normalize_from_line(data: &[u8]) -> Vec<u8> {
    let re = regex::bytes::Regex::new(
        r"(?m)^From (\S+) +\w{3} \w{3} [ \d]\d \d{2}:\d{2}:\d{2} \d{4}\n",
    )
    .unwrap();
    re.replace_all(data, b"From $1 TIMESTAMP\n".as_slice())
        .into_owned()
}

pub fn normalize_message_id(data: &[u8]) -> Vec<u8> {
    let re = regex::bytes::Regex::new(r"Message-ID: <[^>]+>").unwrap();
    re.replace_all(data, b"Message-ID: <GENERATED>".as_slice())
        .into_owned()
}

fn walk(base: &Path, dir: &Path, map: &mut BTreeMap<String, Vec<u8>>) {
    for e in fs::read_dir(dir).unwrap() {
        let e = e.unwrap();
        let p = e.path();
        if p.is_dir() {
            walk(base, &p, map);
        } else {
            let rel = p.strip_prefix(base).unwrap();
            map.insert(
                rel.to_string_lossy().into_owned(),
                fs::read(&p).unwrap(),
            );
        }
    }
}

/// Recursively snapshot a directory tree into a map of relative paths to
/// file contents.
pub fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut map = BTreeMap::new();
    if dir.exists() {
        walk(dir, dir, &mut map);
    }
    map
}

fn is_maildir(base: &Path, top: &str) -> bool {
    base.join(top).join("new").is_dir()
}

fn diff_by_name(
    a: &[(String, Vec<u8>)], b: &[(String, Vec<u8>)],
) -> Result<(), String> {
    let ma: BTreeMap<_, _> = a.iter().cloned().collect();
    let mb: BTreeMap<_, _> = b.iter().cloned().collect();
    let ka: Vec<_> = ma.keys().collect();
    let kb: Vec<_> = mb.keys().collect();
    if ka != kb {
        return Err(format!("file sets differ:\n  a: {ka:?}\n  b: {kb:?}"));
    }
    for (k, va) in &ma {
        let na = normalize_from_line(va);
        let nb = normalize_from_line(&mb[k.as_str()]);
        if na != nb {
            return Err(format!(
                "{k} differs:\n--- a ---\n{}\n--- b ---\n{}",
                String::from_utf8_lossy(&na),
                String::from_utf8_lossy(&nb),
            ));
        }
    }
    Ok(())
}

fn diff_by_content(
    label: &str, a: &[(String, Vec<u8>)], b: &[(String, Vec<u8>)],
) -> Result<(), String> {
    if a.len() != b.len() {
        return Err(format!(
            "{label}: file count differs: a={}, b={}",
            a.len(),
            b.len(),
        ));
    }
    let mut va: Vec<_> =
        a.iter().map(|(_, d)| normalize_from_line(d)).collect();
    let mut vb: Vec<_> =
        b.iter().map(|(_, d)| normalize_from_line(d)).collect();
    va.sort();
    vb.sort();
    for (i, (da, db)) in va.iter().zip(vb.iter()).enumerate() {
        if da != db {
            return Err(format!(
                "{label} file #{i} differs:\n--- a ---\n{}\n--- b ---\n{}",
                String::from_utf8_lossy(da),
                String::from_utf8_lossy(db),
            ));
        }
    }
    Ok(())
}

/// Compare two directory trees.  For each entry in the tree, if it looks
/// like a maildir (has a `new` subdir), compare by sorted content.
/// Otherwise compare by filename.
pub fn diff_dirs(a: &Path, b: &Path) -> Result<(), String> {
    let sa = snapshot(a);
    let sb = snapshot(b);
    let group = |s: &BTreeMap<String, Vec<u8>>| {
        let mut m: BTreeMap<_, Vec<(_, _)>> = BTreeMap::new();
        for (k, v) in s {
            let top = k.split('/').next().unwrap_or("").to_string();
            m.entry(top).or_default().push((k.clone(), v.clone()));
        }
        m
    };
    let ga = group(&sa);
    let gb = group(&sb);
    let ka: Vec<_> = ga.keys().collect();
    let kb: Vec<_> = gb.keys().collect();
    if ka != kb {
        return Err(format!(
            "top-level entries differ:\n  a: {ka:?}\n  b: {kb:?}"
        ));
    }
    for (top, fa) in &ga {
        let fb = &gb[top];
        if is_maildir(a, top) {
            diff_by_content(top, fa, fb)?;
        } else {
            diff_by_name(fa, fb)?;
        }
    }
    Ok(())
}

#[derive(Default)]
pub struct RcBuilder {
    suffix: &'static str,
    flags: String,
    negated: bool,
    conditions: Vec<String>,
    rules: Vec<String>,
}

impl RcBuilder {
    pub fn new(kind: FolderType) -> Self {
        Self {
            suffix: kind.suffix(),
            ..Default::default()
        }
    }

    pub fn build(&mut self) -> String {
        assert!(
            self.conditions.is_empty(),
            "pending conditions without folder()"
        );
        let suffix = self.suffix;
        let mut rc =
            format!("MAILDIR=$MAILDIR\nDEFAULT=$DEFAULT{suffix}\nLOG=log\n");
        if !self.rules.is_empty() {
            rc += "\n";
            rc += &self.rules.join("\n\n");
            rc += "\n";
        }
        rc
    }

    fn emit(&mut self, action: &str) {
        let flags = if self.flags.is_empty() {
            String::new()
        } else {
            format!(" {}", self.flags)
        };
        let mut rule = format!(":0{flags}");
        for c in self.conditions.drain(..) {
            rule += &format!("\n{c}");
        }
        rule += &format!("\n{action}");
        self.rules.push(rule);
        self.flags.clear();
    }

    fn cond(&mut self, c: &str) -> &mut Self {
        let prefix = if self.negated { "* ! " } else { "* " };
        self.negated = false;
        self.conditions.push(format!("{prefix}{c}"));
        self
    }

    pub fn copy(&mut self) -> &mut Self {
        self.flags = "c".into();
        self
    }

    pub fn negate(&mut self) -> &mut Self {
        self.negated = true;
        self
    }

    pub fn spam(&mut self) -> &mut Self {
        self.cond("^X-Spam-Flag: YES")
    }
    pub fn lists(&mut self) -> &mut Self {
        self.cond("^TO_lists\\.net")
    }
    pub fn priority(&mut self) -> &mut Self {
        self.cond("^X-Priority: 1")
    }

    pub fn from(&mut self, addr: &str) -> &mut Self {
        let escaped = addr.replace('.', "\\.");
        self.cond(&format!("^From:.*{escaped}"))
    }

    pub fn subject(&mut self, pat: &str) -> &mut Self {
        self.cond(&format!("^Subject:.*{pat}"))
    }

    pub fn size_gt(&mut self, n: usize) -> &mut Self {
        self.cond(&format!("> {n}"))
    }

    pub fn size_lt(&mut self, n: usize) -> &mut Self {
        self.cond(&format!("< {n}"))
    }

    pub fn folder(&mut self, name: &str) -> &mut Self {
        let suffix = self.suffix;
        self.emit(&format!("{name}{suffix}"));
        self
    }

    pub fn dev_null(&mut self) -> &mut Self {
        self.emit("/dev/null");
        self
    }
}
