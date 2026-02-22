use std::path::PathBuf;

use super::*;

#[test]
fn folder() {
    assert_eq!(
        Action::parse_line("/var/mail/spam"),
        Action::Folder(vec![PathBuf::from("/var/mail/spam")]),
    );
}

#[test]
fn pipe() {
    assert_eq!(
        Action::parse_line("| /usr/bin/spamassassin"),
        Action::Pipe {
            cmd: "/usr/bin/spamassassin".into(),
            capture: None,
        },
    );
}

#[test]
fn pipe_capture() {
    assert_eq!(
        Action::parse_line("RESULT=| /usr/bin/filter"),
        Action::Pipe {
            cmd: "/usr/bin/filter".into(),
            capture: Some("RESULT".into()),
        },
    );
}

#[test]
fn forward() {
    assert_eq!(
        Action::parse_line("! admin@example.com backup@example.com"),
        Action::Forward(vec![
            "admin@example.com".into(),
            "backup@example.com".into(),
        ]),
    );
}

#[test]
fn maildir() {
    assert_eq!(
        Action::parse_line("Maildir/"),
        Action::Folder(vec![PathBuf::from("Maildir/")]),
    );
}

#[test]
fn empty_forward_becomes_folder() {
    assert_eq!(
        Action::parse_line("!"),
        Action::Folder(vec![PathBuf::from("!")]),
    );
}

#[test]
fn pipe_capture_empty_name() {
    // =| cmd — empty var name, not a valid capture
    assert_eq!(
        Action::parse_line("=| /bin/cmd"),
        Action::Folder(vec![PathBuf::from("=|"), PathBuf::from("/bin/cmd")]),
    );
}

#[test]
fn pipe_capture_underscore() {
    assert_eq!(
        Action::parse_line("_=| /bin/cmd"),
        Action::Pipe {
            cmd: "/bin/cmd".into(),
            capture: Some("_".into()),
        },
    );
}

#[test]
fn pipe_capture_space_before_eq() {
    // VAR =| cmd — space before = makes it not a var name
    assert_eq!(
        Action::parse_line("VAR =| /bin/cmd"),
        Action::Folder(vec![
            PathBuf::from("VAR"),
            PathBuf::from("=|"),
            PathBuf::from("/bin/cmd"),
        ]),
    );
}

#[test]
fn pipe_capture_space_after_eq() {
    // VAR= | cmd — space between = and | is OK
    assert_eq!(
        Action::parse_line("VAR= | /bin/cmd"),
        Action::Pipe {
            cmd: "/bin/cmd".into(),
            capture: Some("VAR".into()),
        },
    );
}

#[test]
fn pipe_capture_invalid_name() {
    assert_eq!(
        Action::parse_line("123=| /bin/cmd"),
        Action::Folder(vec![PathBuf::from("123=|"), PathBuf::from("/bin/cmd")]),
    );
}

#[test]
fn multi_folder() {
    assert_eq!(
        Action::parse_line("dir1/ dir2/ dir3/"),
        Action::Folder(vec![
            PathBuf::from("dir1/"),
            PathBuf::from("dir2/"),
            PathBuf::from("dir3/"),
        ]),
    );
}

#[test]
fn multi_folder_mixed() {
    assert_eq!(
        Action::parse_line("/var/mail/spam Maildir/"),
        Action::Folder(vec![
            PathBuf::from("/var/mail/spam"),
            PathBuf::from("Maildir/"),
        ]),
    );
}

#[test]
fn dupecheck() {
    assert_eq!(
        Action::parse_line("@D 8192 .cache"),
        Action::DupeCheck {
            maxlen: "8192".into(),
            cache: ".cache".into(),
        },
    );
}

#[test]
fn dupecheck_extra_whitespace() {
    assert_eq!(
        Action::parse_line("@D  8192   .cache"),
        Action::DupeCheck {
            maxlen: "8192".into(),
            cache: ".cache".into(),
        },
    );
}

#[test]
fn dupecheck_no_cache_becomes_folder() {
    assert_eq!(
        Action::parse_line("@D"),
        Action::Folder(vec![PathBuf::from("@D")]),
    );
}

#[test]
fn header_op_delete_insert() {
    assert_eq!(
        Action::parse_line("@I Subject: hello"),
        Action::HeaderOp(HeaderOp::DeleteInsert {
            field: "Subject".into(),
            value: "hello".into(),
        }),
    );
}

#[test]
fn header_op_rename_insert() {
    assert_eq!(
        Action::parse_line("@i Subject: hello"),
        Action::HeaderOp(HeaderOp::RenameInsert {
            field: "Subject".into(),
            value: "hello".into(),
        }),
    );
}

#[test]
fn header_op_add_if_not() {
    assert_eq!(
        Action::parse_line("@a Lines: 42"),
        Action::HeaderOp(HeaderOp::AddIfNot {
            field: "Lines".into(),
            value: "42".into(),
        }),
    );
}

#[test]
fn header_op_add_always() {
    assert_eq!(
        Action::parse_line("@A X-Tag: spam"),
        Action::HeaderOp(HeaderOp::AddAlways {
            field: "X-Tag".into(),
            value: "spam".into(),
        }),
    );
}

#[test]
fn header_op_empty_field() {
    assert_eq!(
        Action::parse_line("@I : value"),
        Action::Folder(vec![
            PathBuf::from("@I"),
            PathBuf::from(":"),
            PathBuf::from("value"),
        ]),
    );
}

#[test]
fn header_op_unknown_op() {
    assert_eq!(
        Action::parse_line("@Z Foo: bar"),
        Action::Folder(vec![
            PathBuf::from("@Z"),
            PathBuf::from("Foo:"),
            PathBuf::from("bar"),
        ]),
    );
}
