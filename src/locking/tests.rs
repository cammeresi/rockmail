use super::*;

#[test]
fn truncate_normal() {
    let mut p = "foo.lock".to_string();
    assert!(truncate_lock_path(&mut p));
    assert_eq!(p, "foo.loc");
}

#[test]
fn truncate_too_short() {
    let mut p = "x".to_string();
    assert!(!truncate_lock_path(&mut p));
    assert_eq!(p, "x");
}

#[test]
fn truncate_slash() {
    let mut p = "/x".to_string();
    assert!(!truncate_lock_path(&mut p));
    assert_eq!(p, "/x");
}

#[test]
fn truncate_empty() {
    let mut p = String::new();
    assert!(!truncate_lock_path(&mut p));
}
