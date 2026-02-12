use super::*;

#[test]
fn decimal() {
    assert_eq!(value_as_int("42", -1), 42);
    assert_eq!(value_as_int("-3", 0), -3);
    assert_eq!(value_as_int("0", 5), 0);
}

#[test]
fn truthy_aliases() {
    for s in ["on", "On", "ON", "y", "Y", "t", "T", "e", "E"] {
        assert_eq!(value_as_int(s, -1), 1, "expected 1 for {s:?}");
    }
}

#[test]
fn falsy_aliases() {
    for s in ["off", "Off", "OFF", "n", "N", "f", "F", "d", "D"] {
        assert_eq!(value_as_int(s, -1), 0, "expected 0 for {s:?}");
    }
}

#[test]
fn all_alias() {
    assert_eq!(value_as_int("a", -1), 2);
    assert_eq!(value_as_int("A", -1), 2);
}

#[test]
fn fallback() {
    assert_eq!(value_as_int("", 7), 7);
    assert_eq!(value_as_int("junk", 99), 99);
}

#[test]
fn whitespace() {
    assert_eq!(value_as_int("  42  ", 0), 42);
    assert_eq!(value_as_int(" yes ", 0), 1);
}

#[test]
fn is_true() {
    assert!(value_is_true("1"));
    assert!(value_is_true("yes"));
    assert!(value_is_true("on"));
    assert!(value_is_true("y"));
    assert!(value_is_true("t"));
    assert!(!value_is_true("0"));
    assert!(!value_is_true("no"));
    assert!(!value_is_true("off"));
    assert!(!value_is_true(""));
}
