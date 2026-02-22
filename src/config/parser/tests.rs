use std::path::PathBuf;

use miette::SourceOffset;

use super::*;
use crate::config::{HeaderOp, MailParts};

fn dummy_src() -> miette::NamedSource<String> {
    miette::NamedSource::new("", String::new())
}

fn dummy_span() -> SourceOffset {
    SourceOffset::from(0)
}

fn parse_rc(input: &str) -> Result<Vec<Item>, ParseError> {
    parse(input, "test")
}

fn recipe(item: &Item) -> &Recipe {
    let Item::Recipe { recipe, .. } = item else {
        panic!("expected Recipe, got {item:?}");
    };
    recipe
}

fn nested(item: &Item) -> &[Item] {
    let Action::Nested(inner) = &recipe(item).action else {
        panic!("expected Nested, got {:?}", recipe(item).action);
    };
    inner
}

fn assign(name: &str, val: &str) -> Item {
    Item::Assign {
        name: name.into(),
        value: val.into(),
        line: 1,
    }
}

#[test]
fn assignment() {
    let items = parse_rc("MAILDIR=/var/mail\nVERBOSE=yes").unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0],
        Item::Assign {
            name: "MAILDIR".into(),
            value: "/var/mail".into(),
            line: 1,
        },
    );
}

#[test]
fn simple_recipe() {
    let rc = r#"
:0
* ^From:.*spam
/dev/null
"#;
    let items = parse_rc(rc).unwrap();
    assert_eq!(items.len(), 1);
    let r = recipe(&items[0]);
    assert_eq!(r.flags.grep, MailParts::Headers);
    assert_eq!(r.conds.len(), 1);
    assert_eq!(r.action, Action::Folder(vec![PathBuf::from("/dev/null")]));
}

#[test]
fn recipe_with_flags() {
    let rc = ":0 Bc:\n* ^Subject:.*test\nspam/";
    let items = parse_rc(rc).unwrap();
    let r = recipe(&items[0]);
    assert_eq!(r.flags.grep, MailParts::Body);
    assert!(r.flags.copy);
    assert!(r.lockfile.is_some());
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
    let items = parse_rc(rc).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(nested(&items[0]).len(), 2);
}

#[test]
fn forward() {
    let rc = ":0\n* ^To:.*admin\n! admin@example.com";
    let items = parse_rc(rc).unwrap();
    assert_eq!(
        recipe(&items[0]).action,
        Action::Forward(vec!["admin@example.com".into()]),
    );
}

#[test]
fn pipe_capture() {
    let rc = ":0\nRESULT=| /usr/bin/filter";
    let items = parse_rc(rc).unwrap();
    assert_eq!(
        recipe(&items[0]).action,
        Action::Pipe {
            cmd: "/usr/bin/filter".into(),
            capture: Some("RESULT".into()),
        },
    );
}

#[test]
fn comments() {
    let rc = r#"
# This is a comment
MAILDIR=/var/mail  # inline comment

:0  # recipe
* ^From:.*test
/dev/null
"#;
    let items = parse_rc(rc).unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(
        items[0],
        Item::Assign {
            name: "MAILDIR".into(),
            value: "/var/mail".into(),
            line: 3,
        },
    );
}

#[test]
fn inline_comment_edge_cases() {
    assert_eq!(
        parse_rc("PATH=/tmp/#nasty").unwrap()[0],
        assign("PATH", "/tmp/#nasty")
    );
    assert_eq!(
        parse_rc("VAR=hello#world").unwrap()[0],
        assign("VAR", "hello#world")
    );
    assert_eq!(
        parse_rc("VAR=hello # world # more").unwrap()[0],
        assign("VAR", "hello")
    );
    assert_eq!(
        parse_rc("VAR=value\t# tab comment").unwrap()[0],
        assign("VAR", "value")
    );
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
    let items = parse_rc(rc).unwrap();
    assert_eq!(items.len(), 3); // MAILDIR, recipe, VERBOSE
}

#[test]
fn missing_action() {
    let rc = ":0\n* ^From:.*spam\n";
    assert_eq!(parse_rc(rc), Err(ParseError::MissingAction(3)));
}

#[test]
fn unclosed_block() {
    let rc = ":0\n{\n:0\nspam/\n";
    assert_eq!(parse_rc(rc), Err(ParseError::UnclosedBlock(2)));
}

#[test]
fn line_continuation() {
    let rc = ":0\n* ^From:.*\\\ncontinued\nspam/";
    let items = parse_rc(rc).unwrap();
    assert_eq!(
        recipe(&items[0]).conds[0],
        Condition::Regex {
            pattern: "^From:.*continued".into(),
            negate: false,
            weight: None,
        },
    );
}

#[test]
fn explicit_lockfile() {
    let rc = ":0 HB:mylock\n* ^Subject:.*\nspam/";
    let items = parse_rc(rc).unwrap();
    let r = recipe(&items[0]);
    assert_eq!(r.flags.grep, MailParts::Full);
    assert_eq!(r.lockfile.as_deref(), Some("mylock"));
}

#[test]
fn variable_unset() {
    assert_eq!(parse_rc("VERBOSE").unwrap()[0], assign("VERBOSE", ""));
}

#[test]
fn inline_empty_block() {
    let rc = ":0\n{ }";
    let items = parse_rc(rc).unwrap();
    assert_eq!(recipe(&items[0]).action, Action::Nested(vec![]));
}

#[test]
fn inline_block_with_assign() {
    let rc = ":0\n{ VAR=value }";
    let items = parse_rc(rc).unwrap();
    assert_eq!(
        recipe(&items[0]).action,
        Action::Nested(vec![Item::Assign {
            name: "VAR".into(),
            value: "value".into(),
            line: 1,
        }]),
    );
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
    assert_eq!(
        p.parse_assignment("VAR=value", 1).unwrap(),
        assign("VAR", "value")
    );
}

#[test]
fn assign_empty_value() {
    let p = Parser::new("");
    assert_eq!(p.parse_assignment("VAR=", 1).unwrap(), assign("VAR", ""));
}

#[test]
fn assign_with_spaces_around_name() {
    let p = Parser::new("");
    assert_eq!(
        p.parse_assignment("  VAR  = value ", 1).unwrap(),
        assign("VAR", "value")
    );
}

#[test]
fn assign_underscore_name() {
    let p = Parser::new("");
    assert_eq!(
        p.parse_assignment("_FOO_2=bar", 1).unwrap(),
        assign("_FOO_2", "bar")
    );
}

#[test]
fn assign_unset() {
    let p = Parser::new("");
    assert_eq!(
        p.parse_assignment("VERBOSE", 1).unwrap(),
        assign("VERBOSE", "")
    );
}

#[test]
fn assign_includerc() {
    let p = Parser::new("");
    assert_eq!(
        p.parse_assignment("INCLUDERC=other.rc", 1).unwrap(),
        Item::Include {
            path: "other.rc".into(),
            line: 1,
        },
    );
}

#[test]
fn assign_switchrc() {
    let p = Parser::new("");
    assert_eq!(
        p.parse_assignment("SWITCHRC=other.rc", 1).unwrap(),
        Item::Switch {
            path: "other.rc".into(),
            line: 1,
        },
    );
}

#[test]
fn assign_switchrc_unset() {
    let p = Parser::new("");
    assert_eq!(
        p.parse_assignment("SWITCHRC", 1).unwrap(),
        Item::Switch {
            path: "".into(),
            line: 1,
        },
    );
}

#[test]
fn assign_invalid_name() {
    let p = Parser::new("");
    assert!(p.parse_assignment("123=value", 1).is_none());
}

#[test]
fn assign_garbage() {
    let p = Parser::new("");
    assert!(p.parse_assignment("not a valid line", 1).is_none());
}

#[test]
fn assign_value_with_equals() {
    let p = Parser::new("");
    assert_eq!(
        p.parse_assignment("VAR=a=b=c", 1).unwrap(),
        assign("VAR", "a=b=c")
    );
}

#[test]
fn header_minimal() {
    let mut p = Parser::new("");
    let (flags, lock) = p.parse_recipe_header(":0", 1, 0).unwrap();
    assert_eq!(flags.grep, MailParts::Headers);
    assert!(lock.is_none());
}

#[test]
fn header_with_flags() {
    let mut p = Parser::new("");
    let (flags, lock) = p.parse_recipe_header(":0 Bc", 1, 0).unwrap();
    assert_eq!(flags.grep, MailParts::Body);
    assert!(flags.copy);
    assert!(lock.is_none());
}

#[test]
fn header_auto_lockfile() {
    let mut p = Parser::new("");
    let (_, lock) = p.parse_recipe_header(":0:", 1, 0).unwrap();
    assert_eq!(lock.as_deref(), Some(""));
}

#[test]
fn header_explicit_lockfile() {
    let mut p = Parser::new("");
    let (_, lock) = p.parse_recipe_header(":0:mylock", 1, 0).unwrap();
    assert_eq!(lock.as_deref(), Some("mylock"));
}

#[test]
fn header_flags_and_lockfile() {
    let mut p = Parser::new("");
    let (flags, lock) = p.parse_recipe_header(":0 HBc:mylock", 1, 0).unwrap();
    assert_eq!(flags.grep, MailParts::Full);
    assert!(flags.copy);
    assert_eq!(lock.as_deref(), Some("mylock"));
}

#[test]
fn header_flags_and_auto_lockfile() {
    let mut p = Parser::new("");
    let (flags, lock) = p.parse_recipe_header(":0 fw:", 1, 0).unwrap();
    assert!(flags.filter);
    assert!(flags.wait);
    assert_eq!(lock.as_deref(), Some(""));
}

#[test]
fn header_leading_whitespace() {
    let mut p = Parser::new("");
    let (flags, _) = p.parse_recipe_header("  :0 B", 1, 0).unwrap();
    assert_eq!(flags.grep, MailParts::Body);
}

#[test]
fn header_legacy_number() {
    let mut p = Parser::new("");
    let (flags, lock) = p.parse_recipe_header(":27 Bc:", 1, 0).unwrap();
    assert_eq!(flags.grep, MailParts::Body);
    assert!(flags.copy);
    assert_eq!(lock.as_deref(), Some(""));
}

#[test]
fn header_no_colon_prefix() {
    let mut p = Parser::new("");
    assert!(p.parse_recipe_header("0 Bc", 1, 0).is_err());
}

#[test]
fn header_lockfile_with_spaces() {
    let mut p = Parser::new("");
    let (_, lock) = p.parse_recipe_header(":0 : mylock ", 1, 0).unwrap();
    assert_eq!(lock.as_deref(), Some("mylock"));
}

#[test]
fn header_lockfile_with_variable() {
    let mut p = Parser::new("");
    let (_, lock) = p.parse_recipe_header(":0:/tmp/lock-$USER", 1, 0).unwrap();
    assert_eq!(lock.as_deref(), Some("/tmp/lock-$USER"));
}

#[test]
fn header_lockfile_special_chars() {
    let mut p = Parser::new("");
    let (_, lock) = p.parse_recipe_header(":0:my.lock_file-1", 1, 0).unwrap();
    assert_eq!(lock.as_deref(), Some("my.lock_file-1"));
}

#[test]
fn header_multiple_colons() {
    let mut p = Parser::new("");
    let (_, lock) = p.parse_recipe_header(":0::extra", 1, 0).unwrap();
    assert_eq!(lock.as_deref(), Some("extra"));
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
    assert_eq!(items, [assign("VAR", "x")]);
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
    recipe(&items[0]);
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
    assert_eq!(p.parse_block(1), Err(ParseError::UnclosedBlock(1)));
}

#[test]
fn block_unclosed_empty() {
    let mut p = Parser::new("");
    assert_eq!(p.parse_block(1), Err(ParseError::UnclosedBlock(1)));
}

#[test]
fn block_nested() {
    let mut p = Parser::new(":0\n{\nVAR=x\n}\n}");
    let items = p.parse_block(1).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(nested(&items[0]).len(), 1);
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
    assert_eq!(r.action, Action::Folder(vec![PathBuf::from("/dev/null")]));
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
    assert_eq!(r.action, Action::Folder(vec![PathBuf::from("/dev/null")]));
}

#[test]
fn recipe_flags_and_lockfile() {
    let mut p = Parser::new(":0 Bc:mylock\n* ^Subject:.*x\nspam/");
    let r = p.parse_recipe().unwrap();
    assert_eq!(r.flags.grep, MailParts::Body);
    assert!(r.flags.copy);
    assert_eq!(r.lockfile.as_deref(), Some("mylock"));
}

#[test]
fn recipe_missing_action() {
    let mut p = Parser::new(":0\n* ^From:.*x\n");
    assert_eq!(p.parse_recipe(), Err(ParseError::MissingAction(3)));
}

#[test]
fn recipe_eof_immediately() {
    let mut p = Parser::new("");
    assert_eq!(p.parse_recipe(), Err(ParseError::UnexpectedEof(1)));
}

#[test]
fn recipe_with_block_action() {
    let mut p = Parser::new(":0\n{\nVAR=x\n}");
    let r = p.parse_recipe().unwrap();
    assert_eq!(
        r.action,
        Action::Nested(vec![Item::Assign {
            name: "VAR".into(),
            value: "x".into(),
            line: 3,
        }]),
    );
}

#[test]
#[should_panic(expected = "expected Recipe")]
fn recipe_helper_panics_on_non_recipe() {
    recipe(&assign("X", "y"));
}

#[test]
#[should_panic(expected = "expected Nested")]
fn nested_helper_panics_on_non_nested() {
    let item = Item::Recipe {
        recipe: Recipe::new(Flags::new(), None, vec![], Action::Folder(vec![])),
        line: 0,
    };
    nested(&item);
}

#[test]
fn warns_on_garbage() {
    let mut p = Parser::new("garbage line here\nVAR=x");
    let items = p.parse().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        p.warnings(),
        [ParseWarning::SkippedLine {
            src: dummy_src(),
            span: dummy_span()
        }],
    );
}

#[test]
fn warns_on_bad_var_name() {
    let mut p = Parser::new("123=value\nVAR=x");
    let items = p.parse().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        p.warnings(),
        [ParseWarning::BadVarName {
            src: dummy_src(),
            span: dummy_span()
        }],
    );
}

#[test]
fn warns_on_bad_condition() {
    let mut p = Parser::new(":0\n* < notanumber\n/dev/null");
    let items = p.parse().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        p.warnings(),
        [ParseWarning::BadCondition {
            src: dummy_src(),
            span: dummy_span()
        }],
    );
}

#[test]
fn warns_on_unknown_flag() {
    let mut p = Parser::new(":0 Xz\n/dev/null");
    let items = p.parse().unwrap();
    assert_eq!(items.len(), 1);
    let w = p.warnings();
    assert_eq!(w.len(), 2);
    assert_eq!(
        w[0],
        ParseWarning::UnknownFlag {
            flag: 'X',
            src: dummy_src(),
            span: dummy_span()
        },
    );
    assert_eq!(
        w[1],
        ParseWarning::UnknownFlag {
            flag: 'z',
            src: dummy_src(),
            span: dummy_span()
        },
    );
}

#[test]
fn subst_basic() {
    let items = parse_rc("VAR =~ s/foo/bar/").unwrap();
    assert_eq!(
        items[0],
        Item::Subst {
            name: "VAR".into(),
            pattern: "foo".into(),
            replace: "bar".into(),
            global: false,
            case_insensitive: false,
            line: 1,
        },
    );
}

#[test]
fn subst_global_icase() {
    let items = parse_rc("X =~ s/a/b/gi").unwrap();
    assert_eq!(
        items[0],
        Item::Subst {
            name: "X".into(),
            pattern: "a".into(),
            replace: "b".into(),
            global: true,
            case_insensitive: true,
            line: 1,
        },
    );
}

#[test]
fn subst_alternate_delimiter() {
    let items = parse_rc("X =~ s|foo|bar|g").unwrap();
    assert_eq!(
        items[0],
        Item::Subst {
            name: "X".into(),
            pattern: "foo".into(),
            replace: "bar".into(),
            global: true,
            case_insensitive: false,
            line: 1,
        },
    );
}

#[test]
fn subst_empty_replace() {
    let items = parse_rc("X =~ s/foo//").unwrap();
    assert_eq!(
        items[0],
        Item::Subst {
            name: "X".into(),
            pattern: "foo".into(),
            replace: "".into(),
            global: false,
            case_insensitive: false,
            line: 1,
        },
    );
}

#[test]
fn subst_quoted() {
    for input in [r#"X =~ "s/a/b/g""#, "X =~ 's/a/b/g'"] {
        let items = parse_rc(input).unwrap();
        assert_eq!(
            items[0],
            Item::Subst {
                name: "X".into(),
                pattern: "a".into(),
                replace: "b".into(),
                global: true,
                case_insensitive: false,
                line: 1,
            },
        );
    }
}

#[test]
fn header_op_delete_insert() {
    let items = parse_rc(":0\n@I Subject: hello").unwrap();
    assert_eq!(
        recipe(&items[0]).action,
        Action::HeaderOp(HeaderOp::DeleteInsert {
            field: "Subject".into(),
            value: "hello".into(),
        }),
    );
}

#[test]
fn header_op_add_if_not() {
    let items = parse_rc(":0\n@a Lines: 42").unwrap();
    assert_eq!(
        recipe(&items[0]).action,
        Action::HeaderOp(HeaderOp::AddIfNot {
            field: "Lines".into(),
            value: "42".into(),
        }),
    );
}

#[test]
fn dupecheck() {
    let items = parse_rc(":0 Wh:\n@D 8192 $HOME/.msgid-cache").unwrap();
    assert_eq!(
        recipe(&items[0]).action,
        Action::DupeCheck {
            maxlen: "8192".into(),
            cache: "$HOME/.msgid-cache".into(),
        },
    );
}

#[test]
fn header_op_in_block() {
    let items = parse_rc(":0\n{\n:0\n@I Subject: test\n}").unwrap();
    let inner = nested(&items[0]);
    assert_eq!(
        recipe(&inner[0]).action,
        Action::HeaderOp(HeaderOp::DeleteInsert {
            field: "Subject".into(),
            value: "test".into(),
        }),
    );
}

#[test]
fn multiline_double_quote() {
    let items = parse_rc("FOO=\"hello\nworld\"").unwrap();
    assert_eq!(
        items[0],
        Item::Assign {
            name: "FOO".into(),
            value: "\"hello\nworld\"".into(),
            line: 1,
        },
    );
}

#[test]
fn multiline_single_quote() {
    let items = parse_rc("FOO='hello\nworld'").unwrap();
    assert_eq!(
        items[0],
        Item::Assign {
            name: "FOO".into(),
            value: "'hello\nworld'".into(),
            line: 1,
        },
    );
}

#[test]
fn unclosed_quote_eof() {
    let items = parse_rc("FOO=\"hello\nworld").unwrap();
    assert_eq!(
        items[0],
        Item::Assign {
            name: "FOO".into(),
            value: "\"hello\nworld".into(),
            line: 1,
        },
    );
}
