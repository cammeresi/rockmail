use super::*;

#[test]
fn regex() {
    let c = Condition::parse("^From:.*spam").unwrap();
    let Condition::Regex {
        pattern,
        negate,
        weight,
    } = c
    else {
        panic!("expected regex");
    };
    assert_eq!(pattern, "^From:.*spam");
    assert!(!negate);
    assert!(weight.is_none());
}

#[test]
fn negated() {
    let c = Condition::parse("! ^From:.*friend").unwrap();
    let Condition::Regex {
        pattern, negate, ..
    } = c
    else {
        panic!("expected regex");
    };
    assert_eq!(pattern, "^From:.*friend");
    assert!(negate);
}

#[test]
fn size() {
    let c = Condition::parse("< 10000").unwrap();
    let Condition::Size { op, bytes, weight } = c else {
        panic!("expected size");
    };
    assert_eq!(op, Ordering::Less);
    assert_eq!(bytes, 10000);
    assert!(weight.is_none());
}

#[test]
fn shell() {
    let c = Condition::parse("? test -f /tmp/flag").unwrap();
    let Condition::Shell {
        cmd,
        negate,
        weight,
    } = c
    else {
        panic!("expected shell");
    };
    assert_eq!(cmd, "test -f /tmp/flag");
    assert!(!negate);
    assert!(weight.is_none());
}

#[test]
fn variable() {
    let c = Condition::parse("SENDER ?? ^admin").unwrap();
    let Condition::Variable {
        name,
        pattern,
        weight,
    } = c
    else {
        panic!("expected variable");
    };
    assert_eq!(name, "SENDER");
    assert_eq!(pattern, "^admin");
    assert!(weight.is_none());
}

#[test]
fn subst() {
    let c = Condition::parse("$ ^From:.*${SENDER}").unwrap();
    let Condition::Subst { inner, negate } = c else {
        panic!("expected subst");
    };
    assert!(!negate);
    let Condition::Regex { pattern, .. } = *inner else {
        panic!("expected inner regex");
    };
    assert_eq!(pattern, "^From:.*${SENDER}");
}

#[test]
fn negated_subst() {
    let c = Condition::parse("! $ ^From:.*${SENDER}").unwrap();
    let Condition::Subst { inner, negate } = c else {
        panic!("expected subst");
    };
    assert!(negate);
    let Condition::Regex {
        pattern, negate, ..
    } = *inner
    else {
        panic!("expected inner regex");
    };
    assert_eq!(pattern, "^From:.*${SENDER}");
    assert!(!negate);
}

#[test]
fn escape() {
    let c = Condition::parse("\\!literal").unwrap();
    let Condition::Regex {
        pattern, negate, ..
    } = c
    else {
        panic!("expected regex");
    };
    assert_eq!(pattern, "!literal");
    assert!(!negate);
}

#[test]
fn size_greater() {
    let c = Condition::parse("> 50000").unwrap();
    let Condition::Size { op, bytes, .. } = c else {
        panic!("expected size");
    };
    assert_eq!(op, Ordering::Greater);
    assert_eq!(bytes, 50000);
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

#[test]
fn weighted_regex() {
    let c = Condition::parse("2000^0 ^From:.*john").unwrap();
    let Condition::Regex {
        pattern,
        negate,
        weight,
    } = c
    else {
        panic!("expected regex");
    };
    assert_eq!(pattern, "^From:.*john");
    assert!(!negate);
    let w = weight.unwrap();
    assert!((w.w - 2000.0).abs() < 0.001);
    assert!((w.x - 0.0).abs() < 0.001);
}

#[test]
fn weighted_negative() {
    let c = Condition::parse("-100^1 ^>").unwrap();
    let Condition::Regex {
        pattern, weight, ..
    } = c
    else {
        panic!("expected regex");
    };
    assert_eq!(pattern, "^>");
    let w = weight.unwrap();
    assert!((w.w - -100.0).abs() < 0.001);
    assert!((w.x - 1.0).abs() < 0.001);
}

#[test]
fn weighted_decimal() {
    let c = Condition::parse("350^.9 :-\\)").unwrap();
    let Condition::Regex {
        pattern, weight, ..
    } = c
    else {
        panic!("expected regex");
    };
    assert_eq!(pattern, ":-\\)");
    let w = weight.unwrap();
    assert!((w.w - 350.0).abs() < 0.001);
    assert!((w.x - 0.9).abs() < 0.001);
}

#[test]
fn weighted_size() {
    let c = Condition::parse("-100^3 > 2000").unwrap();
    let Condition::Size { op, bytes, weight } = c else {
        panic!("expected size");
    };
    assert_eq!(op, Ordering::Greater);
    assert_eq!(bytes, 2000);
    let w = weight.unwrap();
    assert!((w.w - -100.0).abs() < 0.001);
    assert!((w.x - 3.0).abs() < 0.001);
}

#[test]
fn weighted_shell() {
    let c = Condition::parse("100^0 ? test -f /flag").unwrap();
    let Condition::Shell {
        cmd,
        negate,
        weight,
    } = c
    else {
        panic!("expected shell");
    };
    assert_eq!(cmd, "test -f /flag");
    assert!(!negate);
    let w = weight.unwrap();
    assert!((w.w - 100.0).abs() < 0.001);
}

#[test]
fn malformed_weight_double_dot() {
    // 1..2^3 should be treated as regex, not weight
    let c = Condition::parse("1..2^3 pattern").unwrap();
    let Condition::Regex {
        pattern, weight, ..
    } = c
    else {
        panic!("expected regex");
    };
    assert!(weight.is_none());
    assert_eq!(pattern, "1..2^3 pattern");
}

#[test]
fn malformed_weight_multiple_dots() {
    let c = Condition::parse("1.2.3^4.5.6 pattern").unwrap();
    let Condition::Regex {
        pattern, weight, ..
    } = c
    else {
        panic!("expected regex");
    };
    assert!(weight.is_none());
    assert_eq!(pattern, "1.2.3^4.5.6 pattern");
}

#[test]
fn weight_x_equals_one() {
    let c = Condition::parse("5^1 pattern").unwrap();
    let Condition::Regex { weight, .. } = c else {
        panic!("expected regex");
    };
    let w = weight.unwrap();
    assert!((w.x - 1.0).abs() < 0.001);
}

#[test]
fn weight_negative_exponent() {
    let c = Condition::parse("10^-2 pattern").unwrap();
    let Condition::Regex { weight, .. } = c else {
        panic!("expected regex");
    };
    let w = weight.unwrap();
    assert!((w.w - 10.0).abs() < 0.001);
    assert!((w.x - -2.0).abs() < 0.001);
}
