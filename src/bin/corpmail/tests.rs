use super::*;
use corpmail::variables::MockEnv;

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
fn collect_trailing_args_basic() {
    let rest = vec![
        "VAR=val".into(),
        "rcfile.rc".into(),
        "arg1".into(),
        "arg2".into(),
    ];
    let args = collect_trailing_args(&rest);
    assert_eq!(args, vec!["arg1", "arg2"]);
}

#[test]
fn collect_trailing_args_no_rcfile() {
    let rest = vec!["VAR=val".into(), "OTHER=x".into()];
    let args = collect_trailing_args(&rest);
    assert!(args.is_empty());
}

#[test]
fn resolve_rcpath_absolute() {
    let env = ProcEnv {
        home: "/home/user".into(),
        ..Default::default()
    };
    let p = resolve_rcpath("/etc/rc", &env, false);
    assert_eq!(p, PathBuf::from("/etc/rc"));
}

#[test]
fn resolve_rcpath_dotslash() {
    let env = ProcEnv {
        home: "/home/user".into(),
        ..Default::default()
    };
    let p = resolve_rcpath("./local.rc", &env, false);
    assert_eq!(p, PathBuf::from("./local.rc"));
}

#[test]
fn resolve_rcpath_relative_normal() {
    let env = ProcEnv {
        home: "/home/user".into(),
        ..Default::default()
    };
    let p = resolve_rcpath("mail/filter.rc", &env, false);
    assert_eq!(p, PathBuf::from("/home/user/mail/filter.rc"));
}

#[test]
fn resolve_rcpath_mailfilter_mode() {
    let env = ProcEnv {
        home: "/home/user".into(),
        ..Default::default()
    };
    let p = resolve_rcpath("filter.rc", &env, true);
    assert_eq!(p, PathBuf::from("filter.rc"));
}

#[test]
fn find_rcfile_explicit() {
    let tmp = tempfile::tempdir().unwrap();
    let rc = tmp.path().join("test.rc");
    std::fs::write(&rc, ":0\n/dev/null\n").unwrap();

    let env = ProcEnv {
        home: tmp.path().to_string_lossy().into(),
        ..Default::default()
    };
    let files = vec![rc.to_string_lossy().into()];
    let result = find_rcfile(&files, &env, false).unwrap();
    assert_eq!(result, Some(rc));
}

#[test]
fn find_rcfile_missing_mailfilter() {
    let env = ProcEnv::default();
    let result = find_rcfile(&[], &env, true);
    assert!(result.is_err());
}

#[test]
fn find_rcfile_default_procmailrc() {
    let tmp = tempfile::tempdir().unwrap();
    let rc = tmp.path().join(".procmailrc");
    std::fs::write(&rc, ":0\n/dev/null\n").unwrap();

    let env = ProcEnv {
        home: tmp.path().to_string_lossy().into(),
        ..Default::default()
    };
    let result = find_rcfile(&[], &env, false).unwrap();
    assert_eq!(result, Some(rc));
}

#[test]
fn find_rcfile_no_default() {
    let tmp = tempfile::tempdir().unwrap();
    let env = ProcEnv {
        home: tmp.path().to_string_lossy().into(),
        ..Default::default()
    };
    let result = find_rcfile(&[], &env, false).unwrap();
    assert_eq!(result, None);
}

#[test]
fn deliver_default_to_mbox() {
    let tmp = tempfile::tempdir().unwrap();
    let mbox = tmp.path().join("inbox");

    let penv = ProcEnv {
        orgmail: mbox.to_string_lossy().into(),
        ..Default::default()
    };
    let env = MockEnv::new();

    let msg = Message::parse(
        b"From sender@test Mon Jan 1 00:00:00 2024\nSubject: Test\n\nBody\n",
    );
    deliver_default_with_env(&penv, &msg, &env).unwrap();

    let content = std::fs::read_to_string(&mbox).unwrap();
    assert!(content.contains("Subject: Test"));
    assert!(content.contains("Body"));
}

#[test]
fn is_assignment_valid() {
    assert!(is_assignment("FOO=bar"));
    assert!(is_assignment("_x=1"));
    assert!(is_assignment("A="));
}

#[test]
fn is_assignment_invalid() {
    assert!(!is_assignment("123=bad"));
    assert!(!is_assignment("foo-bar=x"));
    assert!(!is_assignment("noequals"));
    assert!(!is_assignment(""));
}
