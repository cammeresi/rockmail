use super::*;

#[test]
fn basic_match() {
    let m = Matcher::new("hello", false).unwrap();
    let r = m.exec("hello world");
    assert!(r.matched);
    assert!(r.capture.is_none());
}

#[test]
fn case_insensitive() {
    let m = Matcher::new("HELLO", true).unwrap();
    assert!(m.exec("hello world").matched);
    assert!(m.exec("HELLO world").matched);
}

#[test]
fn case_sensitive() {
    let m = Matcher::new("HELLO", false).unwrap();
    assert!(!m.exec("hello world").matched);
    assert!(m.exec("HELLO world").matched);
}

#[test]
fn caret_matches_newline() {
    let m = Matcher::new("^Subject:", true).unwrap();
    let text = "From: foo\nSubject: bar";
    assert!(m.exec(text).matched);
}

#[test]
fn dollar_matches_newline() {
    let m = Matcher::new("foo$", false).unwrap();
    let text = "foo\nbar";
    assert!(m.exec(text).matched);
}

#[test]
fn double_caret_anchor_start() {
    let m = Matcher::new("^^From:", false).unwrap();
    assert!(m.exec("From: test").matched);
    assert!(!m.exec("X-From: test\nFrom: foo").matched);
}

#[test]
fn double_caret_anchor_end() {
    let m = Matcher::new("done^^", false).unwrap();
    assert!(m.exec("all done").matched);
    assert!(!m.exec("done here").matched);
}

#[test]
fn word_boundary_start() {
    let m = Matcher::new(r"\<word", false).unwrap();
    assert!(m.exec("a word here").matched);
    assert!(!m.exec("aword here").matched);
}

#[test]
fn word_boundary_end() {
    let m = Matcher::new(r"word\>", false).unwrap();
    assert!(m.exec("a word here").matched);
    assert!(!m.exec("a wording").matched);
}

#[test]
fn match_capture() {
    let m = Matcher::new(r"Subject: \/.*", false).unwrap();
    let r = m.exec("Subject: Hello World");
    assert!(r.matched);
    assert_eq!(r.capture, Some("Hello World".to_string()));
}

#[test]
fn match_capture_partial() {
    let m = Matcher::new(r"From: \/.+@", false).unwrap();
    let r = m.exec("From: user@example.com");
    assert!(r.matched);
    assert_eq!(r.capture, Some("user@".to_string()));
}

#[test]
fn no_match() {
    let m = Matcher::new("xyz", false).unwrap();
    let r = m.exec("hello world");
    assert!(!r.matched);
    assert!(r.capture.is_none());
}

#[test]
fn word_at_text_start() {
    let m = Matcher::new(r"\<word", false).unwrap();
    assert!(m.exec("word here").matched);
}

#[test]
fn word_at_text_end() {
    let m = Matcher::new(r"word\>", false).unwrap();
    assert!(m.exec("the word").matched);
}

#[test]
fn word_after_newline() {
    let m = Matcher::new(r"\<foo", false).unwrap();
    assert!(m.exec("bar\nfoo").matched);
}

#[test]
fn both_anchors() {
    let m = Matcher::new("^^exact^^", false).unwrap();
    assert!(m.exec("exact").matched);
    assert!(!m.exec("exact ").matched);
    assert!(!m.exec(" exact").matched);
}

#[test]
fn alternation() {
    let m = Matcher::new("foo|bar", false).unwrap();
    assert!(m.exec("foo").matched);
    assert!(m.exec("bar").matched);
    assert!(!m.exec("baz").matched);
}

#[test]
fn groups() {
    let m = Matcher::new("(foo)+", false).unwrap();
    assert!(m.exec("foofoo").matched);
}

#[test]
fn capture_with_groups() {
    let m = Matcher::new(r"X-(\w+): \/.*", false).unwrap();
    let r = m.exec("X-Custom: value");
    assert!(r.matched);
    assert_eq!(r.capture, Some("value".to_string()));
}

#[test]
fn escape_normal() {
    let m = Matcher::new(r"\.", false).unwrap();
    assert!(m.exec("a.b").matched);
    assert!(!m.exec("axb").matched);
}
