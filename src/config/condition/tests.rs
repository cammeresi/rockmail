use super::*;

#[test]
fn regex() {
    let c = Condition::parse("^From:.*spam").unwrap();
    match c {
        Condition::Regex { pattern, negate } => {
            assert_eq!(pattern, "^From:.*spam");
            assert!(!negate);
        }
        _ => panic!("expected regex"),
    }
}

#[test]
fn negated() {
    let c = Condition::parse("! ^From:.*friend").unwrap();
    match c {
        Condition::Regex { pattern, negate } => {
            assert_eq!(pattern, "^From:.*friend");
            assert!(negate);
        }
        _ => panic!("expected regex"),
    }
}

#[test]
fn size() {
    let c = Condition::parse("< 10000").unwrap();
    match c {
        Condition::Size { op, bytes } => {
            assert_eq!(op, Ordering::Less);
            assert_eq!(bytes, 10000);
        }
        _ => panic!("expected size"),
    }
}

#[test]
fn shell() {
    let c = Condition::parse("? test -f /tmp/flag").unwrap();
    match c {
        Condition::Shell { cmd } => {
            assert_eq!(cmd, "test -f /tmp/flag");
        }
        _ => panic!("expected shell"),
    }
}

#[test]
fn variable() {
    let c = Condition::parse("SENDER ?? ^admin").unwrap();
    match c {
        Condition::Variable { name, pattern } => {
            assert_eq!(name, "SENDER");
            assert_eq!(pattern, "^admin");
        }
        _ => panic!("expected variable"),
    }
}

#[test]
fn subst() {
    let c = Condition::parse("$ ^From:.*${SENDER}").unwrap();
    match c {
        Condition::Subst { inner, negate } => {
            assert!(!negate);
            match *inner {
                Condition::Regex { pattern, .. } => {
                    assert_eq!(pattern, "^From:.*${SENDER}");
                }
                _ => panic!("expected inner regex"),
            }
        }
        _ => panic!("expected subst"),
    }
}

#[test]
fn negated_subst() {
    let c = Condition::parse("! $ ^From:.*${SENDER}").unwrap();
    match c {
        Condition::Subst { inner, negate } => {
            assert!(negate);
            match *inner {
                Condition::Regex { pattern, negate } => {
                    assert_eq!(pattern, "^From:.*${SENDER}");
                    assert!(!negate);
                }
                _ => panic!("expected inner regex"),
            }
        }
        _ => panic!("expected subst"),
    }
}

#[test]
fn escape() {
    let c = Condition::parse("\\!literal").unwrap();
    match c {
        Condition::Regex { pattern, negate } => {
            assert_eq!(pattern, "!literal");
            assert!(!negate);
        }
        _ => panic!("expected regex"),
    }
}

#[test]
fn size_greater() {
    let c = Condition::parse("> 50000").unwrap();
    match c {
        Condition::Size { op, bytes } => {
            assert_eq!(op, Ordering::Greater);
            assert_eq!(bytes, 50000);
        }
        _ => panic!("expected size"),
    }
}

#[test]
fn empty_returns_none() {
    assert!(Condition::parse("").is_none());
    assert!(Condition::parse("   ").is_none());
}

#[test]
fn invalid_size_returns_none() {
    assert!(Condition::parse("< notanumber").is_none());
}
