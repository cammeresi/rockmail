use std::path::PathBuf;

use super::*;

#[test]
fn flags_default() {
    let f = Flags::new();
    assert!(f.head);
    assert!(!f.body);
    assert!(f.pass_head);
    assert!(f.pass_body);
}

#[test]
fn flag_h() {
    let f = Flags::parse("H");
    assert!(f.head);
    assert!(!f.body);
}

#[test]
fn flag_b() {
    let f = Flags::parse("B");
    assert!(!f.head);
    assert!(f.body);
}

#[test]
fn flag_hb() {
    let f = Flags::parse("HB");
    assert!(f.head);
    assert!(f.body);
}

#[test]
fn flag_d() {
    let f = Flags::parse("D");
    assert!(f.case);
}

#[test]
fn flag_chain_a() {
    let f = Flags::parse("A");
    assert!(f.chain);
}

#[test]
fn flag_succ_a() {
    let f = Flags::parse("a");
    assert!(f.succ);
}

#[test]
fn flag_else() {
    let f = Flags::parse("E");
    assert!(f.else_);
}

#[test]
fn flag_err() {
    let f = Flags::parse("e");
    assert!(f.err);
}

#[test]
fn flag_pass_head() {
    let f = Flags::parse("h");
    assert!(f.pass_head);
}

#[test]
fn flag_pass_body() {
    let f = Flags::parse("b");
    assert!(f.pass_body);
}

#[test]
fn flag_filter() {
    let f = Flags::parse("f");
    assert!(f.filter);
}

#[test]
fn flag_copy() {
    let f = Flags::parse("c");
    assert!(f.copy);
}

#[test]
fn flag_wait() {
    let f = Flags::parse("w");
    assert!(f.wait);
    assert!(!f.quiet);
}

#[test]
fn flag_wait_quiet() {
    let f = Flags::parse("W");
    assert!(f.wait);
    assert!(f.quiet);
}

#[test]
fn flag_ignore() {
    let f = Flags::parse("i");
    assert!(f.ignore);
}

#[test]
fn flag_raw() {
    let f = Flags::parse("r");
    assert!(f.raw);
}

#[test]
fn flag_empty() {
    let f = Flags::parse("");
    assert!(f.head);
    assert!(!f.body);
    assert!(f.pass_head);
    assert!(f.pass_body);
    assert!(!f.case);
    assert!(!f.chain);
    assert!(!f.succ);
    assert!(!f.else_);
    assert!(!f.err);
    assert!(!f.filter);
    assert!(!f.copy);
    assert!(!f.wait);
    assert!(!f.quiet);
    assert!(!f.ignore);
    assert!(!f.raw);
}

#[test]
fn flag_whitespace_ignored() {
    let f = Flags::parse("c w");
    assert!(f.copy);
    assert!(f.wait);
}

#[test]
fn flag_all_combined() {
    let f = Flags::parse("HBDAaEehbfcwWir");
    assert!(f.head);
    assert!(f.body);
    assert!(f.case);
    assert!(f.chain);
    assert!(f.succ);
    assert!(f.else_);
    assert!(f.err);
    assert!(f.pass_head);
    assert!(f.pass_body);
    assert!(f.filter);
    assert!(f.copy);
    assert!(f.wait);
    assert!(f.quiet);
    assert!(f.ignore);
    assert!(f.raw);
}

#[test]
fn is_delivering() {
    let folder = Recipe::new(
        Flags::new(),
        None,
        vec![],
        Action::Folder(PathBuf::from("spam/")),
    );
    assert!(folder.is_delivering());

    let forward = Recipe::new(
        Flags::new(),
        None,
        vec![],
        Action::Forward(vec!["a@b.com".into()]),
    );
    assert!(forward.is_delivering());

    let pipe = Recipe::new(
        Flags::new(),
        None,
        vec![],
        Action::Pipe {
            cmd: "cat".into(),
            capture: None,
        },
    );
    assert!(pipe.is_delivering());

    let capture = Recipe::new(
        Flags::new(),
        None,
        vec![],
        Action::Pipe {
            cmd: "cat".into(),
            capture: Some("OUT".into()),
        },
    );
    assert!(!capture.is_delivering());

    let nested =
        Recipe::new(Flags::new(), None, vec![], Action::Nested(vec![]));
    assert!(!nested.is_delivering());
}
