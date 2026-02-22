//! Integration tests for the rockmail binary.

use std::fs::{self, Permissions};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use std::process::{Command, Stdio};

use tempfile::TempDir;

#[allow(unused)]
mod common;

struct Output {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    code: i32,
}

fn run_full(dir: &Path, args: &[&str], input: &[u8]) -> Output {
    let mut child = Command::new(common::rockmail())
        .args(args)
        .current_dir(dir)
        .env("RUST_LOG", "info")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn rockmail");
    if let Err(e) = child.stdin.take().unwrap().write_all(input)
        && e.kind() != ErrorKind::BrokenPipe
    {
        panic!("write stdin: {e}");
    }
    let out = child.wait_with_output().expect("wait");
    Output {
        stdout: out.stdout,
        stderr: out.stderr,
        code: out.status.code().unwrap_or(-1),
    }
}

fn run_env(
    dir: &Path, args: &[&str], input: &[u8], env: &[(&str, &str)],
) -> Output {
    let mut cmd = Command::new(common::rockmail());
    cmd.args(args)
        .current_dir(dir)
        .env("RUST_LOG", "info")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for &(k, v) in env {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().expect("failed to spawn rockmail");
    if let Err(e) = child.stdin.take().unwrap().write_all(input)
        && e.kind() != ErrorKind::BrokenPipe
    {
        panic!("write stdin: {e}");
    }
    let out = child.wait_with_output().expect("wait");
    Output {
        stdout: out.stdout,
        stderr: out.stderr,
        code: out.status.code().unwrap_or(-1),
    }
}

fn run(dir: &Path, args: &[&str], input: &[u8]) -> (Vec<u8>, i32) {
    let o = run_full(dir, args, input);
    (o.stderr, o.code)
}

fn write_rc(dir: &Path, rc: &str) -> String {
    let path = dir.join("rcfile");
    let rc = rc.replace("$DIR", dir.to_str().unwrap());
    fs::write(&path, rc).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    path.to_str().unwrap().to_string()
}

#[test]
fn logfile_written() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let log = d.join("log");
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/default
LOGFILE=$DIR/log
VERBOSE=on

:0
inbox
",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    assert!(log.exists(), "logfile not created");
    let content = fs::read_to_string(&log).unwrap();
    assert!(
        content.contains("Delivered"),
        "logfile missing delivery entry: {content:?}"
    );
}

#[test]
fn log_variable_writes_to_logfile() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let log = d.join("log");
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/default
LOGFILE=$DIR/log

:0
{
    LOG=hello_from_LOG
}
",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    assert!(log.exists(), "logfile not created");
    let content = fs::read_to_string(&log).unwrap();
    assert!(
        content.contains("hello_from_LOG"),
        "LOG variable text not in logfile: {content:?}"
    );
}

// No gold test for Maildir secondaries: procmail has a bug where
// setlastfolder() clobbers buf2 before the secondary link loop
// (mailfold.c:282), so Maildir secondary linking always fails with
// ENOENT.  MH/Dir work because buf2 is restored at line 305.
#[test]
fn secondary_maildir() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/default/

:0
inbox/ copy/
",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);

    let inbox = d.join("inbox/new");
    let copy = d.join("copy/new");
    assert!(inbox.is_dir(), "inbox/new not created");
    assert!(copy.is_dir(), "copy/new not created");

    let inbox_files: Vec<_> = fs::read_dir(&inbox).unwrap().flatten().collect();
    let copy_files: Vec<_> = fs::read_dir(&copy).unwrap().flatten().collect();
    assert_eq!(inbox_files.len(), 1, "expected 1 file in inbox/new");
    assert_eq!(copy_files.len(), 1, "expected 1 file in copy/new");

    // Same content
    let a = fs::read(inbox_files[0].path()).unwrap();
    let b = fs::read(copy_files[0].path()).unwrap();
    assert_eq!(a, b, "inbox and copy content differ");

    // Hard-linked (same inode)
    let ia = fs::metadata(inbox_files[0].path()).unwrap().ino();
    let ib = fs::metadata(copy_files[0].path()).unwrap().ino();
    assert_eq!(ia, ib, "expected hard link (same inode)");
}

#[test]
fn secondary_dir() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    fs::create_dir(d.join("inbox")).unwrap();
    fs::create_dir(d.join("copy")).unwrap();
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/default

:0
inbox copy
",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);

    let inbox = d.join("inbox");
    let copy = d.join("copy");

    let inbox_files: Vec<_> = fs::read_dir(&inbox).unwrap().flatten().collect();
    let copy_files: Vec<_> = fs::read_dir(&copy).unwrap().flatten().collect();
    assert_eq!(inbox_files.len(), 1, "expected 1 file in inbox");
    assert_eq!(copy_files.len(), 1, "expected 1 file in copy");

    let a = fs::read(inbox_files[0].path()).unwrap();
    let b = fs::read(copy_files[0].path()).unwrap();
    assert_eq!(a, b, "inbox and copy content differ");

    let ia = fs::metadata(inbox_files[0].path()).unwrap().ino();
    let ib = fs::metadata(copy_files[0].path()).unwrap().ino();
    assert_eq!(ia, ib, "expected hard link (same inode)");
}

#[test]
fn builtin_defaults_expand() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let out = d.join("out");
    let rc = write_rc(
        d,
        &format!(
            "\
MAILDIR=$DIR
DEFAULT=$DIR/default

:0 hw
| /bin/echo $SENDMAIL $SHELLFLAGS $LOCKEXT > {}
",
            out.display()
        ),
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let text = fs::read_to_string(&out).unwrap();
    assert!(
        text.contains("/usr/sbin/sendmail"),
        "SENDMAIL not expanded: {text:?}"
    );
    assert!(text.contains("-c"), "SHELLFLAGS not expanded: {text:?}");
    assert!(text.contains(".lock"), "LOCKEXT not expanded: {text:?}");
}

#[test]
fn lockfile_global_acquired() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let lock = d.join("global.lock");
    let rc = write_rc(
        d,
        &format!(
            "\
MAILDIR=$DIR
DEFAULT=$DIR/default
LOCKFILE={}
",
            lock.display()
        ),
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    // Lock should be cleaned up after process exits
    assert!(
        !lock.exists(),
        "global lockfile should be removed after exit"
    );
}

const UMASK_MSG: &[u8] = b"From: user@host\nSubject: Test\n\nBody\n";

#[test]
fn default_umask_0600() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let mbox = d.join("inbox");
    let rc = write_rc(d, "MAILDIR=$DIR\nDEFAULT=$DIR/inbox\n");
    let (_, code) = run(d, &["-f", "sender@test", &rc], UMASK_MSG);
    assert_eq!(code, 0);
    let mode = fs::metadata(&mbox).unwrap().mode() & 0o777;
    assert_eq!(mode, 0o600, "expected 0600, got {mode:03o}");
}

#[test]
fn umask_override() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let mbox = d.join("inbox");
    let rc = write_rc(d, "MAILDIR=$DIR\nDEFAULT=$DIR/inbox\nUMASK=022\n");
    let (_, code) = run(d, &["-f", "sender@test", &rc], UMASK_MSG);
    assert_eq!(code, 0);
    let mode = fs::metadata(&mbox).unwrap().mode() & 0o777;
    assert_eq!(mode, 0o645, "expected 0645, got {mode:03o}");
}

#[test]
fn maildir_file_perms() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let inbox = d.join("inbox");
    let rc = write_rc(
        d,
        "MAILDIR=$DIR\nDEFAULT=$DIR/fallback\nUMASK=022\n\n:0\ninbox/\n",
    );
    let (_, code) = run(d, &["-f", "sender@test", &rc], UMASK_MSG);
    assert_eq!(code, 0);
    let entry = fs::read_dir(inbox.join("new"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap();
    let mode = entry.metadata().unwrap().mode() & 0o777;
    assert_eq!(mode, 0o644, "message file: expected 0644, got {mode:03o}");
}

#[test]
fn mh_perms() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let inbox = d.join("inbox");
    let rc = write_rc(
        d,
        "MAILDIR=$DIR\nDEFAULT=$DIR/fallback\nUMASK=022\n\n:0\ninbox/.\n",
    );
    let (_, code) = run(d, &["-f", "sender@test", &rc], UMASK_MSG);
    assert_eq!(code, 0);

    let mode = fs::metadata(&inbox).unwrap().mode() & 0o777;
    assert_eq!(mode, 0o755, "MH dir: expected 0755, got {mode:03o}");

    let msg = inbox.join("1");
    let mode = fs::metadata(&msg).unwrap().mode() & 0o777;
    assert_eq!(mode, 0o644, "MH message: expected 0644, got {mode:03o}");
}

#[test]
fn maildir_execute_bit() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let inbox = d.join("inbox");
    let rc = write_rc(
        d,
        "MAILDIR=$DIR\nDEFAULT=$DIR/fallback\nUMASK=022\n\n:0\ninbox/\n",
    );
    let (_, code) = run(d, &["-f", "sender@test", &rc], UMASK_MSG);
    assert_eq!(code, 0);
    for sub in ["tmp", "new", "cur"] {
        let p = inbox.join(sub);
        let mode = fs::metadata(&p).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o755, "inbox/{sub}: expected 0755, got {mode:03o}");
    }

    let mode = fs::metadata(&inbox).unwrap().mode() & 0o777;
    fs::set_permissions(&inbox, Permissions::from_mode(mode & !0o001)).unwrap();

    let (_, code) = run(d, &["-f", "sender@test", &rc], UMASK_MSG);
    assert_eq!(code, 0);
    let mode = fs::metadata(&inbox).unwrap().mode() & 0o777;
    assert_eq!(
        mode & 0o001,
        0o001,
        "inbox after re-delivery: expected o+x set, got {mode:03o}"
    );
}

#[test]
fn dryrun_default_delivery() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let mbox = d.join("inbox");
    let rc = write_rc(d, "MAILDIR=$DIR\nDEFAULT=$DIR/inbox\n");
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (stderr, code) = run(d, &["-n", "-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let err = String::from_utf8_lossy(&stderr);
    assert!(
        err.contains("deliver to default"),
        "expected dryrun message: {err:?}"
    );
    assert!(!mbox.exists(), "mbox should not be created in dryrun mode");
}

#[test]
fn dryrun_folder_delivery() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc =
        write_rc(d, "MAILDIR=$DIR\nDEFAULT=$DIR/default\n\n:0\n$DIR/inbox\n");
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (stderr, code) = run(d, &["-n", "-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let err = String::from_utf8_lossy(&stderr);
    assert!(err.contains("deliver:"), "expected dryrun message: {err:?}");
    assert!(
        !d.join("inbox").exists(),
        "folder should not be created in dryrun mode"
    );
}

#[test]
fn dryrun_forward() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        "MAILDIR=$DIR\nDEFAULT=$DIR/default\n\n:0\n! user@example.com\n",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (stderr, code) = run(d, &["-n", "-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let err = String::from_utf8_lossy(&stderr);
    assert!(
        err.contains("forward"),
        "expected forward dryrun message: {err:?}"
    );
}

#[test]
fn dryrun_pipe_runs_normally() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let marker = d.join("pipe_ran");
    let rc = write_rc(
        d,
        &format!(
            "MAILDIR=$DIR\nDEFAULT=$DIR/default\n\n:0\n| touch {}\n",
            marker.display()
        ),
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-n", "-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    assert!(marker.exists(), "pipe should still run in dryrun");
}

#[test]
fn recipe_lockfile_auto() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/default

:0 :
inbox
",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    assert!(d.join("inbox").exists(), "mbox not created");
    assert!(
        !d.join("inbox.lock").exists(),
        "auto lockfile should be cleaned up"
    );
}

#[test]
fn recipe_lockfile_named() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/default

:0 : my.lock
inbox
",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    assert!(d.join("inbox").exists(), "mbox not created");
    assert!(
        !d.join("my.lock").exists(),
        "named lockfile should be cleaned up"
    );
}

#[test]
fn header_op_add_always() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let mbox = d.join("inbox");
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/inbox

:0
@A X-Tag: first
:0
@A X-Tag: second
",
    );
    let input = b"From: user@host\nX-Tag: existing\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let content = fs::read_to_string(&mbox).unwrap();
    let lines: Vec<_> = content.lines().collect();
    assert!(
        lines.contains(&"X-Tag: existing"),
        "original header lost: {content:?}"
    );
    assert!(
        lines.contains(&"X-Tag: first"),
        "first @A not added: {content:?}"
    );
    assert!(
        lines.contains(&"X-Tag: second"),
        "second @A not added: {content:?}"
    );
}

#[test]
fn dryrun_show_msg() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(d, "MAILDIR=$DIR\nDEFAULT=$DIR/inbox\n");
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let o = run_full(d, &["-n", "-M", "-f", "sender@test", &rc], input);
    assert_eq!(o.code, 0);
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        out.contains("Body"),
        "stdout should contain message: {out:?}"
    );
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(
        err.contains("-----"),
        "stderr should contain separator: {err:?}"
    );
}

#[test]
fn dryrun_no_lockfile() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        "MAILDIR=$DIR\nDEFAULT=$DIR/inbox\nLOCKFILE=$DIR/global.lock\n",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-n", "-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    assert!(
        !d.join("global.lock").exists(),
        "lockfile should not be created in dryrun"
    );
}

#[test]
fn dryrun_condition_match() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        "MAILDIR=$DIR\nDEFAULT=$DIR/default\n\n:0\n* ^Subject: Match\n$DIR/matched\n",
    );
    let input = b"From: user@host\nSubject: Match\n\nBody\n";
    let (stderr, code) = run(d, &["-n", "-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let err = String::from_utf8_lossy(&stderr);
    assert!(
        err.contains("deliver:"),
        "matched recipe should log delivery: {err:?}"
    );
    assert!(
        !d.join("matched").exists(),
        "folder should not be created in dryrun"
    );
}

#[test]
fn dryrun_condition_no_match() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        "MAILDIR=$DIR\nDEFAULT=$DIR/default\n\n:0\n* ^Subject: NoMatch\n$DIR/matched\n",
    );
    let input = b"From: user@host\nSubject: Other\n\nBody\n";
    let (stderr, code) = run(d, &["-n", "-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let err = String::from_utf8_lossy(&stderr);
    assert!(
        !err.contains("deliver: ") || err.contains("deliver to default"),
        "non-matching recipe should not log folder delivery: {err:?}"
    );
}

#[test]
fn dryrun_no_logfile() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc =
        write_rc(d, "MAILDIR=$DIR\nDEFAULT=$DIR/inbox\nLOGFILE=$DIR/log\n");
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-n", "-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    assert!(
        !d.join("log").exists(),
        "logfile should not be created in dryrun"
    );
}

#[test]
fn dryrun_filter() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(d, "MAILDIR=$DIR\nDEFAULT=$DIR/inbox\n\n:0 fhb\n| cat\n");
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (stderr, code) = run(d, &["-n", "-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let err = String::from_utf8_lossy(&stderr);
    assert!(err.contains("filter:"), "expected filter log: {err:?}");
}

fn dryrun_subst(rc_body: &str, expect: &str) {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        &format!("MAILDIR=$DIR\nDEFAULT=$DIR/inbox\n\n{rc_body}\n"),
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (stderr, code) = run(d, &["-n", "-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let err = String::from_utf8_lossy(&stderr);
    assert!(err.contains(expect), "expected {expect:?} in: {err:?}");
}

#[test]
fn dryrun_subst_no_flags() {
    dryrun_subst("X=aaa\nX =~ s/a/b/", "s/a/b/ -> ");
}

#[test]
fn dryrun_subst_global() {
    dryrun_subst("X=aaa\nX =~ s/a/b/g", "s/a/b/g -> ");
}

#[test]
fn dryrun_subst_icase() {
    dryrun_subst("X=Hello\nX =~ s/hello/bye/i", "s/hello/bye/i -> ");
}

#[test]
fn dryrun_subst_global_icase() {
    dryrun_subst("X=Foo foo\nX =~ s/foo/bar/gi", "s/foo/bar/gi -> ");
}

#[test]
fn dryrun_subst_quoted() {
    dryrun_subst("X=aaa\nX =~ \"s/a/b/g\"", "s/a/b/g -> ");
}

#[test]
fn subst_routes_by_transformed_var() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/default

DEST=spam
DEST =~ s/am/ecial/

:0
$DIR/$DEST
",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    assert!(
        d.join("special").exists(),
        "substituted var not used for delivery"
    );
    assert!(
        !d.join("spam").exists(),
        "original value used despite subst"
    );
}

#[test]
fn header_op_delete_insert() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let mbox = d.join("inbox");
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/inbox

:0
@I Subject: replaced
",
    );
    let input = b"From: user@host\nSubject: original\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let content = fs::read_to_string(&mbox).unwrap();
    let lines: Vec<_> = content.lines().collect();
    assert!(
        lines.contains(&"Subject: replaced"),
        "new subject missing: {content:?}"
    );
    assert!(
        !lines.contains(&"Subject: original"),
        "old subject remains: {content:?}"
    );
}

#[test]
fn header_op_rename_insert() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let mbox = d.join("inbox");
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/inbox

:0
@i Subject: new subject
",
    );
    let input = b"From: user@host\nSubject: original\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let content = fs::read_to_string(&mbox).unwrap();
    let lines: Vec<_> = content.lines().collect();
    assert!(
        lines.contains(&"Subject: new subject"),
        "new subject missing: {content:?}"
    );
    assert!(
        lines.contains(&"Old-Subject: original"),
        "old subject not renamed: {content:?}"
    );
}

#[test]
fn header_op_add_if_not() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let mbox = d.join("inbox");
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/inbox

:0
@a X-New: added
:0
@a Subject: ignored
",
    );
    let input = b"From: user@host\nSubject: existing\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let content = fs::read_to_string(&mbox).unwrap();
    let lines: Vec<_> = content.lines().collect();
    assert!(
        lines.contains(&"X-New: added"),
        "absent header not added: {content:?}"
    );
    assert!(
        lines.contains(&"Subject: existing"),
        "original subject lost: {content:?}"
    );
    assert!(
        !lines.contains(&"Subject: ignored"),
        "@a should not add when present: {content:?}"
    );
}

#[test]
fn header_op_variable_expansion() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let mbox = d.join("inbox");
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/inbox

:0
@A X-Folder: $MAILDIR
",
    );
    let input = b"From: user@host\nSubject: test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let content = fs::read_to_string(&mbox).unwrap();
    let lines: Vec<_> = content.lines().collect();
    let expected = format!("X-Folder: {}", d.to_str().unwrap());
    assert!(
        lines.contains(&expected.as_str()),
        "variable not expanded: {content:?}"
    );
}

#[test]
fn header_op_rfc2047() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let mbox = d.join("inbox");
    let rc = write_rc(
        d,
        "MAILDIR=$DIR\nDEFAULT=$DIR/inbox\n\n:0\n@A X-Tag: caf\u{e9}\n",
    );
    let input = b"From: user@host\nSubject: test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let content = fs::read_to_string(&mbox).unwrap();
    assert!(
        content.contains("=?UTF-8?"),
        "non-ASCII value not RFC 2047 encoded: {content:?}"
    );
}

#[test]
fn header_op_not_rfc2047() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let mbox = d.join("inbox");
    let rc = write_rc(
        d,
        "MAILDIR=$DIR\nDEFAULT=$DIR/inbox\n\n:0\n@A X-Tag: cafe\n",
    );
    let input = b"From: user@host\nSubject: test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let content = fs::read_to_string(&mbox).unwrap();
    assert!(
        !content.contains("=?UTF-8?"),
        "ASCII value is RFC 2047 encoded: {content:?}"
    );
}

#[test]
fn dedup_new_message() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let mbox = d.join("inbox");
    let dupes = d.join("dupes");
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/inbox

:0 Wh:
@D 8192 $DIR/msgid.cache

:0
* DUPLICATE ?? yes
$DIR/dupes
",
    );
    let input = b"From: user@host\nMessage-ID: <abc@example>\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    assert!(mbox.exists(), "message not delivered to inbox");
    assert!(!dupes.exists(), "new message routed to dupes");
}

#[test]
fn dedup_duplicate() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let mbox = d.join("inbox");
    let dupes = d.join("dupes");
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/inbox

:0 Wh:
@D 8192 $DIR/msgid.cache

:0
* DUPLICATE ?? yes
$DIR/dupes
",
    );
    let input = b"From: user@host\nMessage-ID: <dup@example>\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    assert!(mbox.exists(), "first message not delivered");
    assert!(!dupes.exists(), "first message routed to dupes");

    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    assert!(dupes.exists(), "duplicate not routed to dupes");
}

#[test]
fn dedup_invalid_size() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/inbox

:0 Wh:
@D notanumber $DIR/msgid.cache

:0
* DUPLICATE ?? yes
$DIR/dupes
",
    );
    let input = b"From: user@host\nMessage-ID: <x@example>\n\nBody\n";
    let (stderr, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let s = String::from_utf8_lossy(&stderr);
    assert!(s.contains("@D: bad maxlen"), "expected bad maxlen: {s}");
    assert!(d.join("inbox").exists(), "not delivered to inbox");
    assert!(!d.join("dupes").exists(), "routed to dupes on bad size");
}

#[test]
fn dedup_invalid_path() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/inbox

:0 Wh:
@D 8192 /nonexistent/dir/cache

:0
* DUPLICATE ?? yes
$DIR/dupes
",
    );
    let input = b"From: user@host\nMessage-ID: <y@example>\n\nBody\n";
    let (stderr, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let s = String::from_utf8_lossy(&stderr);
    assert!(s.contains("@D: cache error"), "expected cache error: {s}");
    assert!(d.join("inbox").exists(), "not delivered to inbox");
    assert!(!d.join("dupes").exists(), "routed to dupes on bad path");
}

#[test]
fn invalid_regex_skips_recipe() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let bad = d.join("bad");
    let good = d.join("good");
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/default

:0
* [invalid
$DIR/bad

:0
* ^Subject
$DIR/good
",
    );
    let input = b"From: user@host\nSubject: test\n\nBody\n";
    let (stderr, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    assert!(!bad.exists(), "invalid regex recipe should not deliver");
    assert!(good.exists(), "subsequent recipe should still deliver");
    let err = String::from_utf8_lossy(&stderr);
    assert!(
        err.contains("Invalid regexp"),
        "should log invalid regex: {err}"
    );
}

#[test]
fn rcfile_crlf() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let mbox = d.join("inbox");
    // Write rcfile with \r\n line endings (Windows style)
    let rc = format!(
        "MAILDIR={d}\r\nDEFAULT={d}/inbox\r\n\r\n:0\r\n* ^Subject: Test\r\n{d}/inbox\r\n",
        d = d.display()
    );
    let p = d.join("rcfile");
    fs::write(&p, &rc).unwrap();
    fs::set_permissions(&p, Permissions::from_mode(0o644)).unwrap();
    let rc = p.to_str().unwrap();
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", rc], input);
    assert_eq!(code, 0);
    assert!(mbox.exists(), "delivery with CRLF rcfile failed");
    let content = fs::read_to_string(&mbox).unwrap();
    assert!(content.contains("Body"), "message body missing");
}

#[test]
fn delivery_error_cantcreat() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        "MAILDIR=$DIR\nDEFAULT=/nonexistent/path\nORGMAIL=/nonexistent/path\n",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 73, "expected EX_CANTCREAT without -t");
}

#[test]
fn delivery_error_tempfail() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        "MAILDIR=$DIR\nDEFAULT=/nonexistent/path\nORGMAIL=/nonexistent/path\n",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-t", "-f", "sender@test", &rc], input);
    assert_eq!(code, 75, "expected EX_TEMPFAIL with -t");
}

#[test]
fn from_override_without_o() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let mbox = d.join("inbox");
    let rc = write_rc(d, "MAILDIR=$DIR\nDEFAULT=$DIR/inbox\n");
    let input =
        b"From original@test Mon Jan  1 00:00:00 2024\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "override@test", &rc], input);
    assert_eq!(code, 0);
    let content = fs::read_to_string(&mbox).unwrap();
    assert!(
        content.starts_with("From original@test"),
        "without -o, original From_ should be kept: {content:?}"
    );
}

#[test]
fn from_override_with_o() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let mbox = d.join("inbox");
    let rc = write_rc(d, "MAILDIR=$DIR\nDEFAULT=$DIR/inbox\n");
    let input =
        b"From original@test Mon Jan  1 00:00:00 2024\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-o", "-f", "override@test", &rc], input);
    assert_eq!(code, 0);
    let content = fs::read_to_string(&mbox).unwrap();
    assert!(
        content.starts_with("From override@test"),
        "with -o, From_ should be overridden: {content:?}"
    );
}

#[test]
fn filter_with_header_op_action() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let mbox = d.join("inbox");
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/inbox

:0
* ^Subject: original
@i Subject: replaced
",
    );
    let input = b"From: user@host\nSubject: original\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let content = fs::read_to_string(&mbox).unwrap();
    let lines: Vec<_> = content.lines().collect();
    assert!(
        lines.contains(&"Subject: replaced"),
        "inserted subject missing: {content:?}"
    );
    assert!(
        lines.contains(&"Old-Subject: original"),
        "original not renamed: {content:?}"
    );
}

#[test]
fn invalid_arg_exits_usage() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["--bogus"], input);
    assert_eq!(code, 64, "expected EX_USAGE for invalid argument");
}

#[test]
fn lines_header_via_scoring() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/inbox

:0
* ! ^Lines:
{
    :0B
    * 1^1 ^.*$
    { }
    LINES=$=

    :0
    @a Lines: $LINES
}
",
    );
    // Three text lines; trailing newline creates a 4th empty match.
    let input = b"From: a@host\nSubject: test\n\nOne\nTwo\nThree\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let content = fs::read_to_string(d.join("inbox")).unwrap();
    assert!(
        content.contains("Lines: 4"),
        "expected Lines: 4: {content:?}"
    );

    // One text line + trailing empty match.
    let input = b"From: b@host\nSubject: test\n\nSingle\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let content = fs::read_to_string(d.join("inbox")).unwrap();
    assert!(
        content.contains("Lines: 2"),
        "expected Lines: 2: {content:?}"
    );

    // Already has Lines: header — should not be modified.
    let input = b"From: c@host\nSubject: test\nLines: 99\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let content = fs::read_to_string(d.join("inbox")).unwrap();
    assert!(
        content.contains("Lines: 99"),
        "existing Lines: should be preserved: {content:?}"
    );
}

#[test]
fn preserve_env() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/default

:0
* TESTVAR ?? hello
$DIR/matched
",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let args = ["-p", "-f", "sender@test", &rc];

    // With -p, TESTVAR survives init_env clearing.
    let o = run_env(d, &args, input, &[("TESTVAR", "hello")]);
    assert_eq!(o.code, 0);
    assert!(d.join("matched").exists(), "-p should preserve TESTVAR");

    fs::remove_file(d.join("matched")).unwrap();

    // Without -p, TESTVAR is wiped.
    let o = run_env(d, &["-f", "sender@test", &rc], input, &[("TESTVAR", "hello")]);
    assert_eq!(o.code, 0);
    assert!(!d.join("matched").exists(), "without -p, TESTVAR should be cleared");
    assert!(d.join("default").exists(), "without -p, should deliver to default");
}

#[test]
fn verbose_off_with_logfile() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let log = d.join("log");
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/default
LOGFILE=$DIR/log
VERBOSE=off

:0
* ^Subject: Test
inbox
",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    assert!(log.exists(), "logfile not created");
    let content = fs::read_to_string(&log).unwrap();
    assert!(
        content.contains("Folder:"),
        "logabstract should appear with LOGFILE set: {content:?}"
    );
    assert!(
        !content.contains("Match"),
        "verbose Match line should not appear with VERBOSE=off: {content:?}"
    );
    assert!(
        !content.contains("Delivered"),
        "verbose Delivered line should not appear with VERBOSE=off: {content:?}"
    );
}

#[test]
fn multiple_rcfiles_error() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(d, "MAILDIR=$DIR\nDEFAULT=$DIR/default\n");
    let rc2 = d.join("rcfile2");
    fs::write(&rc2, "# empty\n").unwrap();
    fs::set_permissions(&rc2, Permissions::from_mode(0o644)).unwrap();
    let rc2 = rc2.to_str().unwrap();
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (stderr, code) = run(d, &["-f", "sender@test", &rc, rc2], input);
    assert_eq!(code, 73, "expected EX_CANTCREAT for multiple rcfiles");
    let err = String::from_utf8_lossy(&stderr);
    assert!(
        err.contains("too many rc files"),
        "expected 'too many rc files' error: {err:?}"
    );
}
