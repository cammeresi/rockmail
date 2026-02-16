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
fn exitcode_override() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/default
EXITCODE=42
",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 42);
}

#[test]
fn exitcode_not_set_returns_zero() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/default
",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
}

#[test]
fn shift_positional_args() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let out = d.join("out");
    let rc = write_rc(
        d,
        &format!(
            "\
MAILDIR=$DIR
DEFAULT=$DIR/default
SHIFT=1

:0 hw
| /bin/echo $1 > {}
",
            out.display()
        ),
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(
        d,
        &["-f", "sender@test", "-a", "first", "-a", "second", &rc],
        input,
    );
    assert_eq!(code, 0);
    let text = fs::read_to_string(&out).unwrap();
    assert!(
        text.contains("second"),
        "SHIFT didn't move $1 to second arg: {text:?}"
    );
}

#[test]
fn host_mismatch_stops_processing() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/default
HOST=no.such.host.invalid

:0
$DIR/matched
",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (stderr, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let err = String::from_utf8_lossy(&stderr);
    assert!(
        err.contains("HOST mismatch"),
        "expected HOST mismatch warning: {err:?}"
    );
    assert!(
        !d.join("matched").exists(),
        "recipe after HOST mismatch should not run"
    );
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

#[test]
fn trap_runs_on_exit() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let marker = d.join("trap_ran");
    let rc = write_rc(
        d,
        &format!(
            "MAILDIR=$DIR\nDEFAULT=$DIR/default\nTRAP=touch {}\n",
            marker.display()
        ),
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    assert!(marker.exists(), "TRAP did not run");
}

#[test]
fn trap_receives_message_on_stdin() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let out = d.join("stdin_dump");
    let rc = write_rc(
        d,
        &format!(
            "MAILDIR=$DIR\nDEFAULT=$DIR/default\nTRAP=cat > {}\n",
            out.display()
        ),
    );
    let input = b"From: user@host\nSubject: Test\n\nTrapBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let content = fs::read_to_string(&out).unwrap();
    assert!(
        content.contains("TrapBody"),
        "TRAP didn't get message: {content:?}"
    );
}

#[test]
fn trap_exitcode_available() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let out = d.join("exitcode");
    let rc = write_rc(
        d,
        &format!(
            "MAILDIR=$DIR\nDEFAULT=$DIR/default\nTRAP=echo \\$EXITCODE > {}\n",
            out.display()
        ),
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let content = fs::read_to_string(&out).unwrap();
    assert_eq!(content.trim(), "0", "EXITCODE not available: {content:?}");
}

#[test]
fn trap_exit_overrides_exitcode() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(d, "MAILDIR=$DIR\nDEFAULT=$DIR/default\nTRAP=exit 7\n");
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 7, "TRAP exit code should override");
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
    assert_eq!(mode, 0o644, "expected 0644, got {mode:03o}");
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
fn forward_with_sendmail_true() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let rc = write_rc(
        d,
        "\
MAILDIR=$DIR
DEFAULT=$DIR/default
SENDMAIL=/bin/true
SENDMAILFLAGS=

:0
! user@example.com
",
    );
    let input = b"From: user@host\nSubject: Test\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    assert!(
        !d.join("default").exists(),
        "message should be consumed by forward, not delivered to default"
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

@A X-Tag: first
@A X-Tag: second
",
    );
    let input = b"From: user@host\nX-Tag: existing\n\nBody\n";
    let (_, code) = run(d, &["-f", "sender@test", &rc], input);
    assert_eq!(code, 0);
    let content = fs::read_to_string(&mbox).unwrap();
    assert!(content.contains("X-Tag: existing"), "original header lost");
    assert!(content.contains("X-Tag: first"), "first @A not added");
    assert!(content.contains("X-Tag: second"), "second @A not added");
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
