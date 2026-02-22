use super::*;

#[test]
fn regex() {
    assert_eq!(
        Condition::parse("^From:.*spam").unwrap(),
        Condition::Regex {
            pattern: "^From:.*spam".into(),
            negate: false,
            weight: None,
        },
    );
}

#[test]
fn negated() {
    assert_eq!(
        Condition::parse("! ^From:.*friend").unwrap(),
        Condition::Regex {
            pattern: "^From:.*friend".into(),
            negate: true,
            weight: None,
        },
    );
}

#[test]
fn size() {
    assert_eq!(
        Condition::parse("< 10000").unwrap(),
        Condition::Size {
            op: SizeOp::Less,
            bytes: 10000,
            negate: false,
            weight: None,
        },
    );
}

#[test]
fn shell() {
    assert_eq!(
        Condition::parse("? test -f /tmp/flag").unwrap(),
        Condition::Shell {
            cmd: "test -f /tmp/flag".into(),
            negate: false,
            weight: None,
        },
    );
}

#[test]
fn variable() {
    assert_eq!(
        Condition::parse("SENDER ?? ^admin").unwrap(),
        Condition::Variable {
            name: "SENDER".into(),
            pattern: "^admin".into(),
            weight: None,
        },
    );
}

#[test]
fn subst() {
    assert_eq!(
        Condition::parse("$ ^From:.*${SENDER}").unwrap(),
        Condition::Subst {
            inner: Box::new(Condition::Regex {
                pattern: "^From:.*${SENDER}".into(),
                negate: false,
                weight: None,
            }),
            negate: false,
        },
    );
}

#[test]
fn negated_subst() {
    assert_eq!(
        Condition::parse("! $ ^From:.*${SENDER}").unwrap(),
        Condition::Subst {
            inner: Box::new(Condition::Regex {
                pattern: "^From:.*${SENDER}".into(),
                negate: false,
                weight: None,
            }),
            negate: true,
        },
    );
}

#[test]
fn escape() {
    assert_eq!(
        Condition::parse("\\!literal").unwrap(),
        Condition::Regex {
            pattern: "!literal".into(),
            negate: false,
            weight: None,
        },
    );
}

#[test]
fn size_greater() {
    assert_eq!(
        Condition::parse("> 50000").unwrap(),
        Condition::Size {
            op: SizeOp::Greater,
            bytes: 50000,
            negate: false,
            weight: None,
        },
    );
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
    assert_eq!(
        Condition::parse("2000^0 ^From:.*john").unwrap(),
        Condition::Regex {
            pattern: "^From:.*john".into(),
            negate: false,
            weight: Some(Weight { w: 2000.0, x: 0.0 }),
        },
    );
}

#[test]
fn weighted_negative() {
    assert_eq!(
        Condition::parse("-100^1 ^>").unwrap(),
        Condition::Regex {
            pattern: "^>".into(),
            negate: false,
            weight: Some(Weight { w: -100.0, x: 1.0 }),
        },
    );
}

#[test]
fn weighted_decimal() {
    assert_eq!(
        Condition::parse("350^.9 :-\\)").unwrap(),
        Condition::Regex {
            pattern: ":-\\)".into(),
            negate: false,
            weight: Some(Weight { w: 350.0, x: 0.9 }),
        },
    );
}

#[test]
fn weighted_size() {
    assert_eq!(
        Condition::parse("-100^3 > 2000").unwrap(),
        Condition::Size {
            op: SizeOp::Greater,
            bytes: 2000,
            negate: false,
            weight: Some(Weight { w: -100.0, x: 3.0 }),
        },
    );
}

#[test]
fn weighted_shell() {
    assert_eq!(
        Condition::parse("100^0 ? test -f /flag").unwrap(),
        Condition::Shell {
            cmd: "test -f /flag".into(),
            negate: false,
            weight: Some(Weight { w: 100.0, x: 0.0 }),
        },
    );
}

#[test]
fn malformed_weight_double_dot() {
    // 1..2^3 should be treated as regex, not weight
    assert_eq!(
        Condition::parse("1..2^3 pattern").unwrap(),
        Condition::Regex {
            pattern: "1..2^3 pattern".into(),
            negate: false,
            weight: None,
        },
    );
}

#[test]
fn malformed_weight_multiple_dots() {
    assert_eq!(
        Condition::parse("1.2.3^4.5.6 pattern").unwrap(),
        Condition::Regex {
            pattern: "1.2.3^4.5.6 pattern".into(),
            negate: false,
            weight: None,
        },
    );
}

#[test]
fn weight_x_equals_one() {
    assert_eq!(
        Condition::parse("5^1 pattern").unwrap(),
        Condition::Regex {
            pattern: "pattern".into(),
            negate: false,
            weight: Some(Weight { w: 5.0, x: 1.0 }),
        },
    );
}

#[test]
fn weight_negative_exponent() {
    assert_eq!(
        Condition::parse("10^-2 pattern").unwrap(),
        Condition::Regex {
            pattern: "pattern".into(),
            negate: false,
            weight: Some(Weight { w: 10.0, x: -2.0 }),
        },
    );
}

#[test]
fn negated_size_less() {
    assert_eq!(
        Condition::parse("! < 5000").unwrap(),
        Condition::Size {
            op: SizeOp::Less,
            bytes: 5000,
            negate: true,
            weight: None,
        },
    );
}

#[test]
fn negated_size_greater() {
    assert_eq!(
        Condition::parse("! > 2000").unwrap(),
        Condition::Size {
            op: SizeOp::Greater,
            bytes: 2000,
            negate: true,
            weight: None,
        },
    );
}

#[test]
fn weighted_variable() {
    assert_eq!(
        Condition::parse("100^1 SENDER ?? ^admin").unwrap(),
        Condition::Variable {
            name: "SENDER".into(),
            pattern: "^admin".into(),
            weight: Some(Weight { w: 100.0, x: 1.0 }),
        },
    );
}

#[test]
fn subst_shell() {
    assert_eq!(
        Condition::parse("$ ? test -f /tmp/${FILE}").unwrap(),
        Condition::Subst {
            inner: Box::new(Condition::Shell {
                cmd: "test -f /tmp/${FILE}".into(),
                negate: false,
                weight: None,
            }),
            negate: false,
        },
    );
}

#[test]
fn subst_variable() {
    assert_eq!(
        Condition::parse("$ LAST ?? first").unwrap(),
        Condition::Subst {
            inner: Box::new(Condition::Variable {
                name: "LAST".into(),
                pattern: "first".into(),
                weight: None,
            }),
            negate: false,
        },
    );
}

#[test]
fn weighted_negated_size() {
    assert_eq!(
        Condition::parse("-10^1 ! > 5").unwrap(),
        Condition::Size {
            op: SizeOp::Greater,
            bytes: 5,
            negate: true,
            weight: Some(Weight { w: -10.0, x: 1.0 }),
        },
    );
}
