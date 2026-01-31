use super::*;

#[test]
fn assignment() {
    let items = parse("MAILDIR=/var/mail\nVERBOSE=yes").unwrap();
    assert_eq!(items.len(), 2);
    match &items[0] {
        Item::Assign { name, value } => {
            assert_eq!(name, "MAILDIR");
            assert_eq!(value, "/var/mail");
        }
        _ => panic!("expected assign"),
    }
}

#[test]
fn simple_recipe() {
    let rc = r#"
:0
* ^From:.*spam
/dev/null
"#;
    let items = parse(rc).unwrap();
    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Recipe(r) => {
            assert!(r.flags.head);
            assert_eq!(r.conds.len(), 1);
            match &r.action {
                Action::Folder(p) => {
                    assert_eq!(p.to_str().unwrap(), "/dev/null")
                }
                _ => panic!("expected folder"),
            }
        }
        _ => panic!("expected recipe"),
    }
}

#[test]
fn recipe_with_flags() {
    let rc = ":0 Bc:\n* ^Subject:.*test\nspam/";
    let items = parse(rc).unwrap();
    match &items[0] {
        Item::Recipe(r) => {
            assert!(!r.flags.head);
            assert!(r.flags.body);
            assert!(r.flags.copy);
            assert!(r.lockfile.is_some());
        }
        _ => panic!("expected recipe"),
    }
}

#[test]
fn nested_block() {
    let rc = r#"
:0
* ^From:.*important
{
    :0 c
    backup/

    :0
    | /usr/bin/notify
}
"#;
    let items = parse(rc).unwrap();
    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Recipe(r) => match &r.action {
            Action::Nested(inner) => {
                assert_eq!(inner.len(), 2);
            }
            _ => panic!("expected nested"),
        },
        _ => panic!("expected recipe"),
    }
}

#[test]
fn forward() {
    let rc = ":0\n* ^To:.*admin\n! admin@example.com";
    let items = parse(rc).unwrap();
    match &items[0] {
        Item::Recipe(r) => match &r.action {
            Action::Forward(addrs) => {
                assert_eq!(addrs[0], "admin@example.com");
            }
            _ => panic!("expected forward"),
        },
        _ => panic!("expected recipe"),
    }
}

#[test]
fn pipe_capture() {
    let rc = ":0\nRESULT=| /usr/bin/filter";
    let items = parse(rc).unwrap();
    match &items[0] {
        Item::Recipe(r) => match &r.action {
            Action::Pipe { cmd, capture } => {
                assert_eq!(cmd, "/usr/bin/filter");
                assert_eq!(capture.as_deref(), Some("RESULT"));
            }
            _ => panic!("expected pipe"),
        },
        _ => panic!("expected recipe"),
    }
}

#[test]
fn comments() {
    let rc = r#"
# This is a comment
MAILDIR=/var/mail  # inline comment not supported, this goes in value

:0  # recipe
* ^From:.*test
/dev/null
"#;
    let items = parse(rc).unwrap();
    assert_eq!(items.len(), 2);
}

#[test]
fn skips_garbage() {
    let rc = r#"
MAILDIR=/var/mail

garbage line here

:0
* ^From:.*test
/dev/null

more garbage

VERBOSE=yes
"#;
    let items = parse(rc).unwrap();
    assert_eq!(items.len(), 3); // MAILDIR, recipe, VERBOSE
}

#[test]
fn missing_action() {
    let rc = ":0\n* ^From:.*spam\n";
    assert!(matches!(parse(rc), Err(ParseError::MissingAction(_))));
}

#[test]
fn unclosed_block() {
    let rc = ":0\n{\n:0\nspam/\n";
    assert!(matches!(parse(rc), Err(ParseError::UnclosedBlock(_))));
}

#[test]
fn line_continuation() {
    let rc = ":0\n* ^From:.*\\\ncontinued\nspam/";
    let items = parse(rc).unwrap();
    match &items[0] {
        Item::Recipe(r) => {
            assert_eq!(r.conds.len(), 1);
            match &r.conds[0] {
                Condition::Regex { pattern, .. } => {
                    assert_eq!(pattern, "^From:.*continued");
                }
                _ => panic!("expected regex"),
            }
        }
        _ => panic!("expected recipe"),
    }
}

#[test]
fn explicit_lockfile() {
    let rc = ":0 HB:mylock\n* ^Subject:.*\nspam/";
    let items = parse(rc).unwrap();
    match &items[0] {
        Item::Recipe(r) => {
            assert!(r.flags.head);
            assert!(r.flags.body);
            assert_eq!(r.lockfile.as_deref(), Some("mylock"));
        }
        _ => panic!("expected recipe"),
    }
}

#[test]
fn variable_unset() {
    let items = parse("VERBOSE").unwrap();
    match &items[0] {
        Item::Assign { name, value } => {
            assert_eq!(name, "VERBOSE");
            assert!(value.is_empty());
        }
        _ => panic!("expected assign"),
    }
}

#[test]
fn inline_empty_block() {
    let rc = ":0\n{ }";
    let items = parse(rc).unwrap();
    match &items[0] {
        Item::Recipe(r) => match &r.action {
            Action::Nested(inner) => assert!(inner.is_empty()),
            _ => panic!("expected nested"),
        },
        _ => panic!("expected recipe"),
    }
}

#[test]
fn inline_block_with_assign() {
    let rc = ":0\n{ VAR=value }";
    let items = parse(rc).unwrap();
    match &items[0] {
        Item::Recipe(r) => match &r.action {
            Action::Nested(inner) => {
                assert_eq!(inner.len(), 1);
                match &inner[0] {
                    Item::Assign { name, value } => {
                        assert_eq!(name, "VAR");
                        assert_eq!(value, "value");
                    }
                    _ => panic!("expected assign"),
                }
            }
            _ => panic!("expected nested"),
        },
        _ => panic!("expected recipe"),
    }
}
