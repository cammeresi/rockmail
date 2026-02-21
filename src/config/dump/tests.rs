use std::path::PathBuf;

use super::*;
use crate::config::{
    Action, Condition, Flags, HeaderOp, Item, MailParts, Recipe, SizeOp, Weight,
};

fn flags() -> Flags {
    Flags::new()
}

fn recipe(f: Flags, conds: Vec<Condition>, action: Action) -> Recipe {
    Recipe::new(f, None, conds, action)
}

#[test]
fn flags_default() {
    assert_eq!(fmt_flags(&flags()), "H (grep header)");
}

#[test]
fn flags_hb() {
    let f = Flags {
        grep: MailParts::Full,
        ..Flags::new()
    };
    assert!(fmt_flags(&f).contains("HB"));
}

#[test]
fn flags_all() {
    let f = Flags {
        grep: MailParts::Body,
        case: true,
        chain: true,
        succ: true,
        r#else: true,
        err: true,
        pass: MailParts::Headers,
        filter: true,
        copy: true,
        wait: true,
        quiet: true,
        ignore: true,
        raw: true,
        ..Default::default()
    };
    let s = fmt_flags(&f);
    for kw in [
        "B ", "D ", "A ", "a ", "E ", "e ", "h ", "f ", "c ", "W ", "i ", "r ",
    ] {
        assert!(s.contains(kw), "missing {kw:?} in {s:?}");
    }
}

#[test]
fn weight_none() {
    assert_eq!(fmt_weight(None), "");
}

#[test]
fn weight_some() {
    let s = fmt_weight(Some(Weight { w: 1.0, x: 0.5 }));
    assert!(s.contains("1"));
    assert!(s.contains("0.5"));
}

#[test]
fn cond_regex() {
    let c = Condition::Regex {
        pattern: "^From:".into(),
        negate: false,
        weight: None,
    };
    let s = fmt_cond(&c);
    assert!(s.contains("regex"));
}

#[test]
fn cond_regex_negated() {
    let c = Condition::Regex {
        pattern: "spam".into(),
        negate: true,
        weight: None,
    };
    assert!(fmt_cond(&c).contains("NOT"));
}

#[test]
fn cond_size() {
    for op in [SizeOp::Less, SizeOp::Greater] {
        let c = Condition::Size {
            op,
            bytes: 100,
            negate: false,
            weight: None,
        };
        let s = fmt_cond(&c);
        assert!(s.contains("100"));
    }
}

#[test]
fn cond_size_negated() {
    let c = Condition::Size {
        op: SizeOp::Less,
        bytes: 50,
        negate: true,
        weight: None,
    };
    assert!(fmt_cond(&c).contains("!"));
}

#[test]
fn cond_shell() {
    let c = Condition::Shell {
        cmd: "test -f /tmp/x".into(),
        negate: false,
        weight: None,
    };
    assert!(fmt_cond(&c).contains("shell"));
}

#[test]
fn cond_variable() {
    let c = Condition::Variable {
        name: "FROM".into(),
        pattern: "user@".into(),
        weight: None,
    };
    let s = fmt_cond(&c);
    assert!(s.contains("FROM"));
    assert!(s.contains("matches"));
}

#[test]
fn cond_subst() {
    let inner = Condition::Regex {
        pattern: "x".into(),
        negate: false,
        weight: None,
    };
    let c = Condition::Subst {
        inner: Box::new(inner),
        negate: true,
    };
    assert!(fmt_cond(&c).contains("NOT subst"));
}

#[test]
fn cond_weighted() {
    let c = Condition::Regex {
        pattern: "test".into(),
        negate: false,
        weight: Some(Weight { w: 2.0, x: 0.8 }),
    };
    let s = fmt_cond(&c);
    assert!(s.contains("2"));
    assert!(s.contains("0.8"));
}

#[test]
fn action_folder() {
    let a = Action::Folder(vec![PathBuf::from("/tmp/inbox")]);
    assert!(fmt_action(&a, 0).contains("deliver to"));
}

#[test]
fn action_maildir() {
    let a = Action::Folder(vec![PathBuf::from("/tmp/Maildir/")]);
    assert!(fmt_action(&a, 0).contains("Maildir"));
}

#[test]
fn action_pipe() {
    let a = Action::Pipe {
        cmd: "cat".into(),
        capture: None,
    };
    assert!(fmt_action(&a, 0).contains("pipe"));
}

#[test]
fn action_pipe_capture() {
    let a = Action::Pipe {
        cmd: "cat".into(),
        capture: Some("OUT".into()),
    };
    let s = fmt_action(&a, 0);
    assert!(s.contains("pipe"));
    assert!(s.contains("OUT"));
}

#[test]
fn action_forward() {
    let a = Action::Forward(vec!["a@b".into(), "c@d".into()]);
    assert!(fmt_action(&a, 0).contains("forward"));
}

#[test]
fn action_nested() {
    let items = vec![Item::Assign {
        name: "X".into(),
        value: "1".into(),
        line: 1,
    }];
    let a = Action::Nested(items);
    assert!(fmt_action(&a, 0).contains("nested"));
}

#[test]
fn action_dupecheck() {
    let a = Action::DupeCheck {
        maxlen: "8192".into(),
        cache: "/tmp/cache".into(),
    };
    assert!(fmt_action(&a, 0).contains("@D"));
}

#[test]
fn item_assign() {
    let s = fmt_item_str(
        &Item::Assign {
            name: "X".into(),
            value: "hello".into(),
            line: 1,
        },
        1,
        0,
    );
    assert!(s.contains("ASSIGN"));
}

#[test]
fn item_unset() {
    let s = fmt_item_str(
        &Item::Assign {
            name: "X".into(),
            value: String::new(),
            line: 1,
        },
        1,
        0,
    );
    assert!(s.contains("UNSET"));
}

#[test]
fn item_recipe() {
    let r = recipe(
        flags(),
        vec![],
        Action::Folder(vec![PathBuf::from("/dev/null")]),
    );
    let s = fmt_item_str(&Item::Recipe { recipe: r, line: 1 }, 1, 0);
    assert!(s.contains("RECIPE"));
}

#[test]
fn item_subst() {
    let s = fmt_item_str(
        &Item::Subst {
            name: "X".into(),
            pattern: "a".into(),
            replace: "b".into(),
            global: true,
            case_insensitive: true,
            line: 1,
        },
        1,
        0,
    );
    assert!(s.contains("SUBST"));
    assert!(s.contains("gi"));
}

#[test]
fn item_headerop() {
    let op = HeaderOp::DeleteInsert {
        field: "X-Tag".into(),
        value: "yes".into(),
    };
    let s = fmt_item_str(&Item::HeaderOp { op, line: 1 }, 1, 0);
    assert!(s.contains("HEADEROP"));
}

#[test]
fn item_include() {
    let s = fmt_item_str(
        &Item::Include {
            path: "/etc/rc".into(),
            line: 1,
        },
        1,
        0,
    );
    assert!(s.contains("INCLUDERC"));
}

#[test]
fn item_switch() {
    let s = fmt_item_str(
        &Item::Switch {
            path: "/etc/rc".into(),
            line: 1,
        },
        1,
        0,
    );
    assert!(s.contains("SWITCHRC"));
}

#[test]
fn item_switch_abort() {
    let s = fmt_item_str(
        &Item::Switch {
            path: String::new(),
            line: 1,
        },
        1,
        0,
    );
    assert!(s.contains("abort"));
}

#[test]
fn subst_flags_combos() {
    assert_eq!(fmt_subst_flags(false, false), "");
    assert_eq!(fmt_subst_flags(true, false), "g");
    assert_eq!(fmt_subst_flags(false, true), "i");
    assert_eq!(fmt_subst_flags(true, true), "gi");
}

#[test]
fn recipe_with_lockfile() {
    let r = Recipe::new(
        flags(),
        Some("mylock".into()),
        vec![],
        Action::Folder(vec![PathBuf::from("/tmp/mbox")]),
    );
    let mut out = String::new();
    fmt_recipe(&mut out, &r, 0);
    assert!(out.contains("Lock: mylock"));
}

#[test]
fn recipe_with_auto_lockfile() {
    let r = Recipe::new(
        flags(),
        Some(String::new()),
        vec![],
        Action::Folder(vec![PathBuf::from("/tmp/mbox")]),
    );
    let mut out = String::new();
    fmt_recipe(&mut out, &r, 0);
    assert!(out.contains("(auto)"));
}

#[test]
fn recipe_with_conditions() {
    let conds = vec![
        Condition::Regex {
            pattern: "^Subject:".into(),
            negate: false,
            weight: None,
        },
        Condition::Size {
            op: SizeOp::Less,
            bytes: 500,
            negate: false,
            weight: None,
        },
    ];
    let r = recipe(
        flags(),
        conds,
        Action::Folder(vec![PathBuf::from("/tmp/x")]),
    );
    let mut out = String::new();
    fmt_recipe(&mut out, &r, 0);
    assert!(out.contains("Conditions:"));
    assert!(out.contains("1."));
    assert!(out.contains("2."));
}

#[test]
fn run_simple_rcfile() {
    let rc = "MAILDIR=/tmp\n:0\n* ^From: test\n/dev/null\n";
    let items = run(rc, "test.rc").unwrap();
    assert_eq!(items.len(), 2);
}

#[test]
fn run_empty() {
    let items = run("", "empty.rc").unwrap();
    assert!(items.is_empty());
}
