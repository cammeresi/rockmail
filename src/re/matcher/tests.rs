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
    assert_eq!(r.capture, Some("Hello World"));
}

#[test]
fn match_capture_partial() {
    let m = Matcher::new(r"From: \/.+@", false).unwrap();
    let r = m.exec("From: user@example.com");
    assert!(r.matched);
    assert_eq!(r.capture, Some("user@"));
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
    assert_eq!(r.capture, Some("value"));
}

#[test]
fn escape_normal() {
    let m = Matcher::new(r"\.", false).unwrap();
    assert!(m.exec("a.b").matched);
    assert!(!m.exec("axb").matched);
}

#[test]
fn capture_with_end_anchor() {
    // C2 fix: ^^ at end of \/ capture region sets anchor_end
    let m = Matcher::new(r"Subject: \/.*^^", false).unwrap();
    assert!(m.exec("Subject: test").matched);
    assert!(!m.exec("Subject: test\n").matched);
}

#[test]
fn non_capturing_group() {
    // C3 fix: (?:...) doesn't increment group count
    let m = Matcher::new(r"(?:prefix)\/captured", false).unwrap();
    let r = m.exec("prefixcaptured");
    assert!(r.matched);
    assert_eq!(r.capture, Some("captured"));
}

#[test]
fn non_capturing_in_capture() {
    let m = Matcher::new(r"\/(?:a|b)+", false).unwrap();
    let r = m.exec("aabba");
    assert!(r.matched);
    assert_eq!(r.capture, Some("aabba"));
}

#[test]
fn double_caret_middle_ignored() {
    // ^^ in middle of pattern is silently ignored (not an anchor)
    let m = Matcher::new("foo^^bar", false).unwrap();
    assert!(m.exec("foobar").matched);
    assert!(m.exec("prefix foobar suffix").matched);
}

#[test]
fn trailing_backslash() {
    // Trailing backslash becomes literal backslash
    let m = Matcher::new(r"foo\", false).unwrap();
    assert!(m.exec(r"foo\").matched);
    assert!(!m.exec("foo").matched);
}

#[test]
fn empty_pattern() {
    let m = Matcher::new("", false).unwrap();
    assert!(m.exec("anything").matched);
    assert!(m.exec("").matched);
}

#[test]
fn capture_only() {
    // Pattern is just \/
    let m = Matcher::new(r"\/", false).unwrap();
    let r = m.exec("text");
    assert!(r.matched);
    assert_eq!(r.capture, Some(""));
}

#[test]
fn capture_then_anchor() {
    let m = Matcher::new(r"\/foo^^", false).unwrap();
    assert!(m.exec("foo").matched);
    assert!(!m.exec("foo ").matched);
}

#[test]
fn pattern_too_long() {
    let long = "a".repeat(MAX_PATTERN_LEN + 1);
    let err = Matcher::new(&long, false).unwrap_err();
    assert!(matches!(err, PatternError::TooLong(_)));
}

#[test]
fn macro_to_underscore() {
    let m = Matcher::new("^TO_foo@", true).unwrap();
    assert!(m.exec("To: foo@bar.com").matched);
    assert!(m.exec("Cc: foo@bar.com").matched);
    assert!(m.exec("Bcc: foo@bar.com").matched);
    assert!(m.exec("Resent-To: foo@bar.com").matched);
    assert!(m.exec("X-Envelope-To: foo@bar.com").matched);
    assert!(!m.exec("From: foo@bar.com").matched);
}

#[test]
fn macro_to() {
    let m = Matcher::new("^TOuser", true).unwrap();
    assert!(m.exec("To: user").matched);
    assert!(m.exec("Cc: user").matched);
    // ^TO matches word boundary, so "notuser" shouldn't match
    assert!(!m.exec("To: notuser").matched);
}

#[test]
fn macro_from_daemon() {
    let m = Matcher::new("^FROM_DAEMON", true).unwrap();
    assert!(m.exec("From: MAILER-DAEMON@host").matched);
    assert!(m.exec("From: postmaster@host").matched);
    assert!(m.exec("Precedence: bulk").matched);
    assert!(m.exec("Mailing-List: foo").matched);
    assert!(!m.exec("From: user@host").matched);
}

#[test]
fn macro_from_mailer() {
    let m = Matcher::new("^FROM_MAILER", true).unwrap();
    assert!(m.exec("From: postmaster@host").matched);
    assert!(m.exec("From: MAILER-DAEMON@host").matched);
    assert!(!m.exec("From: user@host").matched);
}

#[test]
fn macro_expand() {
    assert_eq!(expand_macros("^TO_foo"), format!("{}foo", TO_SUBSTITUTE));
    assert_eq!(expand_macros("^TOfoo"), format!("{}foo", TO2_SUBSTITUTE));
    assert_eq!(expand_macros("plain"), "plain");
    assert_eq!(
        expand_macros("^FROM_DAEMON|^FROM_DAEMON"),
        format!("{FROMD_SUBSTITUTE}|{FROMD_SUBSTITUTE}"),
    );
}

#[test]
fn double_caret_in_alternation() {
    let m = Matcher::new("^^foo|bar", false).unwrap();
    assert!(m.exec("foo").matched);
    assert!(m.exec("bar").matched);
    assert!(!m.exec(" foo").matched);
}

#[test]
fn capture_with_double_caret() {
    let m = Matcher::new(r"^^\/.*^^", false).unwrap();
    let r = m.exec("hello");
    assert!(r.matched);
    assert_eq!(r.capture, Some("hello"));
    assert!(!m.exec("hello\n").matched);
}

#[test]
fn capture_empty() {
    let m = Matcher::new(r"foo\/", false).unwrap();
    let r = m.exec("foo");
    assert!(r.matched);
    assert_eq!(r.capture, Some(""));
}

#[test]
fn caret_after_newline() {
    let m = Matcher::new("^second", false).unwrap();
    assert!(m.exec("first\nsecond").matched);
    assert!(!m.exec("first second").matched);
}

#[test]
fn double_caret_ignores_after_newline() {
    let m = Matcher::new("^^second", false).unwrap();
    assert!(!m.exec("first\nsecond").matched);
    assert!(m.exec("second line").matched);
}

// The next four tests exercise push_char branches, which handle special
// characters inside a \/ capture region.  Some differ from procmail's
// consuming word boundaries; see COMPATIBILITY.md "Regex Word Boundaries".

#[test]
fn capture_word_boundary() {
    // \< and \> inside \/ become \b (zero-width); procmail would consume
    // the boundary character, so the captured text differs.
    let m = Matcher::new(r"\/\<word\>", false).unwrap();
    let r = m.exec("a word here");
    assert!(r.matched);
    assert_eq!(r.capture, Some("word"));
}

#[test]
fn capture_backslash_escape() {
    // Regular \. escape inside \/ capture
    let m = Matcher::new(r"\/foo\.bar", false).unwrap();
    let r = m.exec("foo.bar");
    assert!(r.matched);
    assert_eq!(r.capture, Some("foo.bar"));
    assert!(!m.exec("fooxbar").matched);
}

#[test]
fn capture_trailing_backslash() {
    // Trailing \ inside \/ becomes literal backslash
    let m = Matcher::new("foo\\/\\", false).unwrap();
    let r = m.exec("foo\\");
    assert!(r.matched);
    assert_eq!(r.capture, Some("\\"));
}

#[test]
fn capture_group_inside_capture() {
    // ( inside \/ increments group count so capture index stays correct
    let m = Matcher::new(r"\/(a|b)+x", false).unwrap();
    let r = m.exec("abax");
    assert!(r.matched);
    assert_eq!(r.capture, Some("abax"));
}

#[test]
fn capture_double_caret_mid() {
    // ^^ in the middle of \/ region is silently dropped (not at end,
    // so no anchor_end)
    let m = Matcher::new(r"\/foo^^bar", false).unwrap();
    let r = m.exec("foobar");
    assert!(r.matched);
    assert_eq!(r.capture, Some("foobar"));
}

#[test]
fn find_from_basic() {
    let m = Matcher::new("foo", false).unwrap();
    let text = "xx foo yy foo";
    assert_eq!(m.find_from(text, 0), Some((3, 6)));
    assert_eq!(m.find_from(text, 4), Some((10, 13)));
}

#[test]
fn find_from_at_end() {
    let m = Matcher::new("foo", false).unwrap();
    assert_eq!(m.find_from("foo", 3), None);
}

#[test]
fn find_from_past_end() {
    let m = Matcher::new("foo", false).unwrap();
    assert_eq!(m.find_from("foo", 99), None);
}
