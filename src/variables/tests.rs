use std::time::Duration;

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
    assert_eq!(value_as_int("orange", 5), 5);
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

#[test]
fn env_get_set() {
    let mut e = Environment::new();
    assert_eq!(e.get("FOO"), None);
    e.set("FOO", "bar");
    assert_eq!(e.get("FOO"), Some("bar"));
}

#[test]
fn env_get_or_default_explicit() {
    let mut e = Environment::new();
    e.set("SHELL", "/bin/zsh");
    assert_eq!(e.get_or_default(&SHELL), "/bin/zsh");
}

#[test]
fn env_get_or_default_builtin() {
    let e = Environment::new();
    assert_eq!(e.get_or_default(&SHELL), "/bin/sh");
}

#[test]
fn env_get_or_default_no_default() {
    let e = Environment::new();
    assert_eq!(e.get_or_default(&HOME), "");
}

#[test]
fn env_get_or_default_str_key() {
    let e = Environment::new();
    assert_eq!(e.get_or_default("UNKNOWN"), "");
}

#[test]
fn env_get_num_set() {
    let mut e = Environment::new();
    e.set("TIMEOUT", "300");
    assert_eq!(e.get_num(&TIMEOUT), 300);
}

#[test]
fn env_get_num_default() {
    let e = Environment::new();
    assert_eq!(e.get_num(&TIMEOUT), 960);
}

#[test]
fn env_get_num_unparseable() {
    let mut e = Environment::new();
    e.set("TIMEOUT", "junk");
    assert_eq!(e.get_num(&TIMEOUT), 960);
}

#[test]
fn env_get_num_no_default() {
    let e = Environment::new();
    assert_eq!(e.get_num(&HOME), 0);
}

#[test]
fn env_remove() {
    let mut e = Environment::new();
    e.set("X", "1");
    e.remove("X");
    assert_eq!(e.get("X"), None);
}

#[test]
fn env_remove_absent() {
    let mut e = Environment::new();
    e.remove("NONEXISTENT");
    assert_eq!(e.get("NONEXISTENT"), None);
}

#[test]
fn env_set_default() {
    let mut e = Environment::new();
    e.set_default(&SHELL);
    assert_eq!(e.get(&SHELL), Some("/bin/sh"));
}

#[test]
fn env_set_default_no_default() {
    let mut e = Environment::new();
    e.set_default(&HOME);
    assert_eq!(e.get(&HOME), Some(""));
}

#[test]
fn env_set_all_defaults() {
    let mut e = Environment::new();
    e.set_all_defaults();
    assert_eq!(e.get(&SHELL), Some("/bin/sh"));
    assert_eq!(e.get(&TIMEOUT), Some("960"));
    assert_eq!(e.get(&LOCKEXT), Some(".lock"));
    assert_eq!(e.get(&HOME), None);
}

#[test]
fn env_timeout_default() {
    let e = Environment::new();
    assert_eq!(e.timeout(), Duration::from_secs(960));
}

#[test]
fn env_timeout_custom() {
    let mut e = Environment::new();
    e.set("TIMEOUT", "30");
    assert_eq!(e.timeout(), Duration::from_secs(30));
}

#[test]
fn env_timeout_zero() {
    let mut e = Environment::new();
    e.set("TIMEOUT", "0");
    assert_eq!(e.timeout(), Duration::MAX);
}

#[test]
fn env_timeout_negative() {
    let mut e = Environment::new();
    e.set("TIMEOUT", "-1");
    assert_eq!(e.timeout(), Duration::MAX);
}

#[test]
fn env_iter() {
    let mut e = Environment::new();
    e.set("A", "1");
    e.set("B", "2");
    let mut pairs: Vec<_> = e.iter().collect();
    pairs.sort();
    assert_eq!(pairs, vec![("A", "1"), ("B", "2")]);
}

#[test]
fn env_set_overwrites() {
    let mut e = Environment::new();
    e.set("X", "old");
    e.set("X", "new");
    assert_eq!(e.get("X"), Some("new"));
}
