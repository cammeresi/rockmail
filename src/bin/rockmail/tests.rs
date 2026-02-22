use std::fs;
use std::os::unix::fs::PermissionsExt;

use super::*;
use rockmail::engine::Engine;
use rockmail::variables::{Environment, SubstCtx, VAR_DEFAULT, VAR_HOME};

#[test]
fn parse_rest_assignments() {
    let rest = vec!["FOO=bar".into(), "BAZ=qux".into(), "rcfile.rc".into()];
    let (assigns, files) = parse_rest(&rest);
    assert_eq!(
        assigns,
        vec![("FOO".into(), "bar".into()), ("BAZ".into(), "qux".into()),]
    );
    assert_eq!(files, vec!["rcfile.rc"]);
}

#[test]
fn parse_rest_no_assignments() {
    let rest = vec!["rcfile.rc".into(), "another.rc".into()];
    let (assigns, files) = parse_rest(&rest);
    assert!(assigns.is_empty());
    assert_eq!(files, vec!["rcfile.rc", "another.rc"]);
}

#[test]
fn parse_rest_invalid_var_name() {
    let rest = vec!["123=bad".into(), "good=val".into()];
    let (assigns, files) = parse_rest(&rest);
    assert_eq!(assigns, vec![("good".into(), "val".into())]);
    assert_eq!(files, vec!["123=bad"]);
}

#[test]
fn is_var_name_valid() {
    assert!(is_var_name("FOO"));
    assert!(is_var_name("_bar"));
    assert!(is_var_name("foo123"));
    assert!(is_var_name("A"));
}

#[test]
fn is_var_name_invalid() {
    assert!(!is_var_name("123"));
    assert!(!is_var_name("foo-bar"));
    assert!(!is_var_name(""));
    assert!(!is_var_name("foo.bar"));
}

#[test]
fn resolve_rcpath_absolute() {
    let p = resolve_rcpath("/etc/rc", "/home/user");
    assert_eq!(p, PathBuf::from("/etc/rc"));
}

#[test]
fn resolve_rcpath_dotslash() {
    let p = resolve_rcpath("./local.rc", "/home/user");
    assert_eq!(p, PathBuf::from("./local.rc"));
}

#[test]
fn resolve_rcpath_relative_normal() {
    let p = resolve_rcpath("mail/filter.rc", "/home/user");
    assert_eq!(p, PathBuf::from("/home/user/mail/filter.rc"));
}

fn engine_with_home(home: &str) -> Engine {
    let mut env = Environment::new();
    env.set(VAR_HOME, home);
    Engine::new(env, SubstCtx::default())
}

#[test]
fn find_rcfile_explicit() {
    let tmp = tempfile::tempdir().unwrap();
    let rc = tmp.path().join("test.rc");
    fs::write(&rc, ":0\n/dev/null\n").unwrap();

    let engine = engine_with_home(&tmp.path().to_string_lossy());
    let files = vec![rc.to_string_lossy().into()];
    let result = find_rcfile(&files, &engine).unwrap();
    assert_eq!(result.map(|r| r.path), Some(rc));
}

#[test]
fn find_rcfile_default_procmailrc() {
    let tmp = tempfile::tempdir().unwrap();
    let rc = tmp.path().join(".procmailrc");
    fs::write(&rc, ":0\n/dev/null\n").unwrap();

    let engine = engine_with_home(&tmp.path().to_string_lossy());
    let result = find_rcfile(&[], &engine).unwrap();
    assert_eq!(result.map(|r| r.path), Some(rc));
}

#[test]
fn find_rcfile_no_default() {
    let tmp = tempfile::tempdir().unwrap();
    let engine = engine_with_home(&tmp.path().to_string_lossy());
    let result = find_rcfile(&[], &engine).unwrap();
    assert!(result.is_none());
}

#[test]
fn deliver_default_to_mbox() {
    let tmp = tempfile::tempdir().unwrap();
    let mbox = tmp.path().join("inbox");

    let mut env = Environment::new();
    env.set(VAR_DEFAULT, mbox.to_string_lossy());
    let mut engine = Engine::new(env, SubstCtx::default());

    let msg = Message::parse(
        b"From sender@test Mon Jan 1 00:00:00 2024\nSubject: Test\n\nBody\n",
    );
    deliver_default(&mut engine, &msg).unwrap();

    let content = fs::read_to_string(&mbox).unwrap();
    assert!(content.contains("Subject: Test"));
    assert!(content.contains("Body"));
}

#[test]
fn security_rejects_world_writable() {
    let tmp = tempfile::tempdir().unwrap();
    let rc = tmp.path().join("test.rc");
    fs::write(&rc, ":0\n/dev/null\n").unwrap();
    fs::set_permissions(&rc, fs::Permissions::from_mode(0o666)).unwrap();

    let engine = engine_with_home(&tmp.path().to_string_lossy());
    let files = vec![rc.to_string_lossy().into()];
    let result = find_rcfile(&files, &engine);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("world writable"));
}

#[test]
fn security_rejects_group_writable_default() {
    let tmp = tempfile::tempdir().unwrap();
    let rc = tmp.path().join(".procmailrc");
    fs::write(&rc, ":0\n/dev/null\n").unwrap();
    fs::set_permissions(&rc, fs::Permissions::from_mode(0o664)).unwrap();

    let engine = engine_with_home(&tmp.path().to_string_lossy());
    let result = find_rcfile(&[], &engine);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("group writable"));
}

#[test]
fn security_allows_group_writable_explicit() {
    let tmp = tempfile::tempdir().unwrap();
    let rc = tmp.path().join("test.rc");
    fs::write(&rc, ":0\n/dev/null\n").unwrap();
    fs::set_permissions(&rc, fs::Permissions::from_mode(0o664)).unwrap();

    let engine = engine_with_home(&tmp.path().to_string_lossy());
    let files = vec![rc.to_string_lossy().into()];
    let result = find_rcfile(&files, &engine);
    assert!(result.is_ok());
}

#[test]
fn security_accepts_safe_permissions() {
    let tmp = tempfile::tempdir().unwrap();
    let rc = tmp.path().join("test.rc");
    fs::write(&rc, ":0\n/dev/null\n").unwrap();
    fs::set_permissions(&rc, fs::Permissions::from_mode(0o600)).unwrap();

    let engine = engine_with_home(&tmp.path().to_string_lossy());
    let files = vec![rc.to_string_lossy().into()];
    let result = find_rcfile(&files, &engine);
    assert!(result.is_ok());
    assert!(result.unwrap().is_some());
}

#[test]
fn dir_security_rejects_world_writable() {
    let tmp = tempfile::tempdir().unwrap();
    let subdir = tmp.path().join("unsafe");
    fs::create_dir(&subdir).unwrap();
    fs::set_permissions(&subdir, fs::Permissions::from_mode(0o777)).unwrap();

    let rc = subdir.join("test.rc");
    fs::write(&rc, ":0\n/dev/null\n").unwrap();
    fs::set_permissions(&rc, fs::Permissions::from_mode(0o600)).unwrap();

    let engine = engine_with_home(&tmp.path().to_string_lossy());
    let files = vec![rc.to_string_lossy().into()];
    let result = find_rcfile(&files, &engine);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("directory"));
}

#[test]
fn dir_security_allows_sticky_world_writable() {
    let tmp = tempfile::tempdir().unwrap();
    let subdir = tmp.path().join("sticky");
    fs::create_dir(&subdir).unwrap();
    fs::set_permissions(&subdir, fs::Permissions::from_mode(0o1777)).unwrap();

    let rc = subdir.join("test.rc");
    fs::write(&rc, ":0\n/dev/null\n").unwrap();
    fs::set_permissions(&rc, fs::Permissions::from_mode(0o600)).unwrap();

    let engine = engine_with_home(&tmp.path().to_string_lossy());
    let files = vec![rc.to_string_lossy().into()];
    let result = find_rcfile(&files, &engine);
    assert!(result.is_ok());
}
