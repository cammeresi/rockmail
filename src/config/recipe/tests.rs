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
fn flags_parse() {
    let f = Flags::parse("BDcw");
    assert!(!f.head);
    assert!(f.body);
    assert!(f.case);
    assert!(f.copy);
    assert!(f.wait);
}

#[test]
fn flags_w_quiet() {
    let f = Flags::parse("W");
    assert!(f.wait);
    assert!(f.quiet);
}

#[test]
fn flags_h_resets_default() {
    let f = Flags::parse("H");
    assert!(f.head);
    assert!(!f.body);
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
