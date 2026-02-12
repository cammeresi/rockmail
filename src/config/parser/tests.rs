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

#[test]
fn skip_blank_lines() {
    let mut p = Parser::new("\n\n\nVAR=x");
    p.skip_blank_and_comments();
    assert_eq!(p.peek(), Some("VAR=x"));
}

#[test]
fn skip_comment_lines() {
    let mut p = Parser::new("# comment\n#another\nVAR=x");
    p.skip_blank_and_comments();
    assert_eq!(p.peek(), Some("VAR=x"));
}

#[test]
fn skip_mixed_blank_and_comments() {
    let mut p = Parser::new("\n# comment\n\n# another\n\nVAR=x");
    p.skip_blank_and_comments();
    assert_eq!(p.peek(), Some("VAR=x"));
}

#[test]
fn skip_indented_comments() {
    let mut p = Parser::new("  # indented comment\n\t# tab comment\nVAR=x");
    p.skip_blank_and_comments();
    assert_eq!(p.peek(), Some("VAR=x"));
}

#[test]
fn skip_whitespace_only_lines() {
    let mut p = Parser::new("   \n\t\n  \t  \nVAR=x");
    p.skip_blank_and_comments();
    assert_eq!(p.peek(), Some("VAR=x"));
}

#[test]
fn skip_stops_at_content() {
    let mut p = Parser::new("VAR=x\n# comment");
    p.skip_blank_and_comments();
    assert_eq!(p.peek(), Some("VAR=x"));
}

#[test]
fn skip_all_blank_and_comments() {
    let mut p = Parser::new("# comment\n\n# another\n");
    p.skip_blank_and_comments();
    assert_eq!(p.peek(), None);
}

#[test]
fn continuation_no_backslash() {
    let mut p = Parser::new("second\nthird");
    assert_eq!(p.collect_continuation("first"), "first");
    assert_eq!(p.peek(), Some("second"));
}

#[test]
fn continuation_one_line() {
    let mut p = Parser::new("second\nthird");
    assert_eq!(p.collect_continuation("first\\"), "firstsecond");
    assert_eq!(p.peek(), Some("third"));
}

#[test]
fn continuation_multiple_lines() {
    let mut p = Parser::new("second\\\nthird\nfourth");
    assert_eq!(p.collect_continuation("first\\"), "firstsecondthird");
    assert_eq!(p.peek(), Some("fourth"));
}

#[test]
fn continuation_trims_next_line() {
    let mut p = Parser::new("  second  \nthird");
    assert_eq!(p.collect_continuation("first\\"), "firstsecond");
}

#[test]
fn continuation_eof_after_backslash() {
    let mut p = Parser::new("");
    assert_eq!(p.collect_continuation("first\\"), "first");
}

#[test]
fn continuation_backslash_not_at_end() {
    let mut p = Parser::new("second");
    assert_eq!(p.collect_continuation("fi\\rst"), "fi\\rst");
    assert_eq!(p.peek(), Some("second"));
}

#[test]
fn assign_simple() {
    let p = Parser::new("");
    let item = p.parse_assignment("VAR=value").unwrap();
    assert!(matches!(item, Item::Assign { ref name, ref value }
        if name == "VAR" && value == "value"));
}

#[test]
fn assign_empty_value() {
    let p = Parser::new("");
    let item = p.parse_assignment("VAR=").unwrap();
    assert!(matches!(item, Item::Assign { ref name, ref value }
        if name == "VAR" && value.is_empty()));
}

#[test]
fn assign_with_spaces_around_name() {
    let p = Parser::new("");
    let item = p.parse_assignment("  VAR  = value ").unwrap();
    assert!(matches!(item, Item::Assign { ref name, ref value }
        if name == "VAR" && value == "value"));
}

#[test]
fn assign_underscore_name() {
    let p = Parser::new("");
    let item = p.parse_assignment("_FOO_2=bar").unwrap();
    assert!(matches!(item, Item::Assign { ref name, .. } if name == "_FOO_2"));
}

#[test]
fn assign_unset() {
    let p = Parser::new("");
    let item = p.parse_assignment("VERBOSE").unwrap();
    assert!(matches!(item, Item::Assign { ref name, ref value }
        if name == "VERBOSE" && value.is_empty()));
}

#[test]
fn assign_includerc() {
    let p = Parser::new("");
    let item = p.parse_assignment("INCLUDERC=other.rc").unwrap();
    assert!(matches!(item, Item::Include(ref path) if path == "other.rc"));
}

#[test]
fn assign_switchrc() {
    let p = Parser::new("");
    let item = p.parse_assignment("SWITCHRC=other.rc").unwrap();
    assert!(matches!(item, Item::Switch(ref path) if path == "other.rc"));
}

#[test]
fn assign_switchrc_unset() {
    let p = Parser::new("");
    let item = p.parse_assignment("SWITCHRC").unwrap();
    assert!(matches!(item, Item::Switch(ref path) if path.is_empty()));
}

#[test]
fn assign_invalid_name() {
    let p = Parser::new("");
    assert!(p.parse_assignment("123=value").is_none());
}

#[test]
fn assign_garbage() {
    let p = Parser::new("");
    assert!(p.parse_assignment("not a valid line").is_none());
}

#[test]
fn assign_value_with_equals() {
    let p = Parser::new("");
    let item = p.parse_assignment("VAR=a=b=c").unwrap();
    assert!(matches!(item, Item::Assign { ref value, .. }
        if value == "a=b=c"));
}

#[test]
fn header_minimal() {
    let p = Parser::new("");
    let (flags, lock) = p.parse_recipe_header(":0", 1).unwrap();
    assert!(flags.head);
    assert!(!flags.body);
    assert!(lock.is_none());
}

#[test]
fn header_with_flags() {
    let p = Parser::new("");
    let (flags, lock) = p.parse_recipe_header(":0 Bc", 1).unwrap();
    assert!(!flags.head);
    assert!(flags.body);
    assert!(flags.copy);
    assert!(lock.is_none());
}

#[test]
fn header_auto_lockfile() {
    let p = Parser::new("");
    let (_, lock) = p.parse_recipe_header(":0:", 1).unwrap();
    assert_eq!(lock.as_deref(), Some(""));
}

#[test]
fn header_explicit_lockfile() {
    let p = Parser::new("");
    let (_, lock) = p.parse_recipe_header(":0:mylock", 1).unwrap();
    assert_eq!(lock.as_deref(), Some("mylock"));
}

#[test]
fn header_flags_and_lockfile() {
    let p = Parser::new("");
    let (flags, lock) = p.parse_recipe_header(":0 HBc:mylock", 1).unwrap();
    assert!(flags.head);
    assert!(flags.body);
    assert!(flags.copy);
    assert_eq!(lock.as_deref(), Some("mylock"));
}

#[test]
fn header_flags_and_auto_lockfile() {
    let p = Parser::new("");
    let (flags, lock) = p.parse_recipe_header(":0 fw:", 1).unwrap();
    assert!(flags.filter);
    assert!(flags.wait);
    assert_eq!(lock.as_deref(), Some(""));
}

#[test]
fn header_leading_whitespace() {
    let p = Parser::new("");
    let (flags, _) = p.parse_recipe_header("  :0 B", 1).unwrap();
    assert!(flags.body);
}

#[test]
fn header_legacy_number() {
    let p = Parser::new("");
    let (flags, lock) = p.parse_recipe_header(":27 Bc:", 1).unwrap();
    assert!(flags.body);
    assert!(flags.copy);
    assert_eq!(lock.as_deref(), Some(""));
}

#[test]
fn header_no_colon_prefix() {
    let p = Parser::new("");
    assert!(p.parse_recipe_header("0 Bc", 1).is_err());
}

#[test]
fn header_lockfile_with_spaces() {
    let p = Parser::new("");
    let (_, lock) = p.parse_recipe_header(":0 : mylock ", 1).unwrap();
    assert_eq!(lock.as_deref(), Some("mylock"));
}

#[test]
fn block_empty() {
    let mut p = Parser::new("}");
    let items = p.parse_block(1).unwrap();
    assert!(items.is_empty());
}

#[test]
fn block_one_assign() {
    let mut p = Parser::new("VAR=x\n}");
    let items = p.parse_block(1).unwrap();
    assert_eq!(items.len(), 1);
    assert!(matches!(&items[0], Item::Assign { name, .. }
        if name == "VAR"));
}

#[test]
fn block_multiple_items() {
    let mut p = Parser::new("A=1\nB=2\nC=3\n}");
    let items = p.parse_block(1).unwrap();
    assert_eq!(items.len(), 3);
}

#[test]
fn block_with_recipe() {
    let mut p = Parser::new(":0\n* ^From:.*spam\n/dev/null\n}");
    let items = p.parse_block(1).unwrap();
    assert_eq!(items.len(), 1);
    assert!(matches!(&items[0], Item::Recipe(_)));
}

#[test]
fn block_skips_blanks_and_comments() {
    let mut p = Parser::new("\n# comment\n\nVAR=x\n\n}");
    let items = p.parse_block(1).unwrap();
    assert_eq!(items.len(), 1);
}

#[test]
fn block_unclosed() {
    let mut p = Parser::new("VAR=x\n");
    assert!(matches!(
        p.parse_block(1),
        Err(ParseError::UnclosedBlock(_))
    ));
}

#[test]
fn block_unclosed_empty() {
    let mut p = Parser::new("");
    assert!(matches!(
        p.parse_block(1),
        Err(ParseError::UnclosedBlock(_))
    ));
}

#[test]
fn block_nested() {
    let mut p = Parser::new(":0\n{\nVAR=x\n}\n}");
    let items = p.parse_block(1).unwrap();
    assert_eq!(items.len(), 1);
    match &items[0] {
        Item::Recipe(r) => match &r.action {
            Action::Nested(inner) => assert_eq!(inner.len(), 1),
            _ => panic!("expected nested"),
        },
        _ => panic!("expected recipe"),
    }
}

#[test]
fn block_depth_restored_on_success() {
    let mut p = Parser::new("VAR=x\n}");
    assert_eq!(p.depth, 0);
    let _ = p.parse_block(1).unwrap();
    assert_eq!(p.depth, 0);
}

#[test]
fn block_depth_restored_on_error() {
    let mut p = Parser::new("");
    assert_eq!(p.depth, 0);
    let _ = p.parse_block(1);
    assert_eq!(p.depth, 0);
}

#[test]
fn recipe_no_conditions() {
    let mut p = Parser::new(":0\n/dev/null");
    let r = p.parse_recipe().unwrap();
    assert!(r.conds.is_empty());
    assert!(matches!(r.action, Action::Folder(_)));
}

#[test]
fn recipe_multiple_conditions() {
    let mut p = Parser::new(":0\n* ^From:.*a\n* ^To:.*b\nspam/");
    let r = p.parse_recipe().unwrap();
    assert_eq!(r.conds.len(), 2);
}

#[test]
fn recipe_blanks_between_header_and_conds() {
    let mut p = Parser::new(":0\n\n# comment\n* ^From:.*x\n/dev/null");
    let r = p.parse_recipe().unwrap();
    assert_eq!(r.conds.len(), 1);
}

#[test]
fn recipe_blanks_between_conds_and_action() {
    let mut p = Parser::new(":0\n* ^From:.*x\n\n/dev/null");
    let r = p.parse_recipe().unwrap();
    assert_eq!(r.conds.len(), 1);
    assert!(matches!(r.action, Action::Folder(_)));
}

#[test]
fn recipe_flags_and_lockfile() {
    let mut p = Parser::new(":0 Bc:mylock\n* ^Subject:.*x\nspam/");
    let r = p.parse_recipe().unwrap();
    assert!(r.flags.body);
    assert!(r.flags.copy);
    assert_eq!(r.lockfile.as_deref(), Some("mylock"));
}

#[test]
fn recipe_missing_action() {
    let mut p = Parser::new(":0\n* ^From:.*x\n");
    assert!(matches!(
        p.parse_recipe(),
        Err(ParseError::MissingAction(_))
    ));
}

#[test]
fn recipe_eof_immediately() {
    let mut p = Parser::new("");
    assert!(matches!(
        p.parse_recipe(),
        Err(ParseError::UnexpectedEof(_))
    ));
}

#[test]
fn recipe_with_block_action() {
    let mut p = Parser::new(":0\n{\nVAR=x\n}");
    let r = p.parse_recipe().unwrap();
    assert!(matches!(r.action, Action::Nested(_)));
}
