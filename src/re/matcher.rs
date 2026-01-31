//! Procmail regex compiler - wraps the `regex` crate with procmail extensions.
//!
//! Extensions:
//! - `^^` at start: anchor at absolute start of text
//! - `^^` at end: anchor at absolute end of text
//! - `\<` and `\>`: word boundaries (zero-width, using `\b`)
//! - `\/`: capture point - text after this goes to $MATCH
//! - `^` and `$`: match newlines (multiline mode)

use regex::{Regex, RegexBuilder};
use std::iter::Peekable;
use std::str::Chars;
use thiserror::Error;

const MAX_PATTERN_LEN: usize = 4096;

#[derive(Error, Debug)]
pub enum PatternError {
    #[error("pattern too long: {0} bytes (max {MAX_PATTERN_LEN})")]
    TooLong(usize),
    #[error("regex compilation failed: {0}")]
    Regex(#[from] regex::Error),
}

/// Result of a regex match, including any captured MATCH text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchResult<'a> {
    /// Whether the pattern matched.
    pub matched: bool,
    /// Text captured after `\/` (for $MATCH variable).
    pub capture: Option<&'a str>,
}

/// Compiled procmail regex matcher.
#[derive(Debug)]
pub struct Matcher {
    regex: Regex,
    /// Index of capture group for `\/` extraction, if present.
    capture_group: Option<usize>,
}

impl Matcher {
    /// Compile a procmail regex pattern.
    pub fn new(
        pattern: &str, case_insensitive: bool,
    ) -> Result<Self, PatternError> {
        if pattern.len() > MAX_PATTERN_LEN {
            return Err(PatternError::TooLong(pattern.len()));
        }

        let compiled = compile(pattern);

        let regex = RegexBuilder::new(&compiled)
            .case_insensitive(case_insensitive)
            .multi_line(true)
            .build()?;

        Ok(Self {
            regex,
            capture_group: compiled_capture_group(pattern),
        })
    }

    /// Match against text.
    pub fn exec<'a>(&self, text: &'a str) -> MatchResult<'a> {
        let no_match = MatchResult {
            matched: false,
            capture: None,
        };

        // Fast path: no capture needed, use find() instead of captures()
        let Some(group) = self.capture_group else {
            return match self.regex.is_match(text) {
                true => MatchResult {
                    matched: true,
                    capture: None,
                },
                false => no_match,
            };
        };

        // Slow path: need captures
        let Some(caps) = self.regex.captures(text) else {
            return no_match;
        };

        let capture = caps.get(group).map(|m| m.as_str());

        MatchResult {
            matched: true,
            capture,
        }
    }
}

/// Compile a procmail pattern to a Rust regex pattern.
/// Anchors (^^) are compiled into \A and \z for efficiency.
fn compile(pat: &str) -> String {
    // Capacity: worst case is \< or \> expanding to \b (same size),
    // plus \A and \z anchors (4 bytes total)
    let mut out = String::with_capacity(pat.len() + 4);
    let mut chars = pat.chars().peekable();
    let mut group_count = 0usize;
    let mut anchor_start = false;
    let mut anchor_end = false;
    let mut at_start = true;

    while let Some(c) = chars.next() {
        match c {
            '^' if chars.peek() == Some(&'^') => {
                chars.next();
                if at_start {
                    anchor_start = true;
                }
                if chars.peek().is_none() {
                    anchor_end = true;
                }
            }
            '\\' => handle_escape(
                &mut chars,
                &mut out,
                &mut group_count,
                &mut anchor_end,
            ),
            '(' => {
                if chars.peek() != Some(&'?') {
                    group_count += 1;
                }
                out.push('(');
            }
            _ => out.push(c),
        }
        at_start = false;
    }

    // Prepend/append anchors to compiled pattern
    let mut result = String::with_capacity(out.len() + 4);
    if anchor_start {
        result.push_str(r"\A");
    }
    result.push_str(&out);
    if anchor_end {
        result.push_str(r"\z");
    }
    result
}

/// Handle backslash escapes during compilation.
fn handle_escape(
    chars: &mut Peekable<Chars>, out: &mut String, group_count: &mut usize,
    anchor_end: &mut bool,
) {
    match chars.peek() {
        Some('<') | Some('>') => {
            chars.next();
            out.push_str(r"\b");
        }
        Some('/') => {
            chars.next();
            *group_count += 1;
            out.push('(');
            while let Some(c2) = chars.next() {
                if push_char(out, c2, chars, group_count) {
                    *anchor_end = true;
                }
            }
            out.push(')');
        }
        Some(&c2) => {
            chars.next();
            out.push('\\');
            out.push(c2);
        }
        None => out.push_str("\\\\"),
    }
}

/// Compute capture group index for a pattern (for \/ extraction).
fn compiled_capture_group(pat: &str) -> Option<usize> {
    let mut chars = pat.chars().peekable();
    let mut group_count = 0usize;

    while let Some(c) = chars.next() {
        match c {
            '^' if chars.peek() == Some(&'^') => {
                chars.next();
            }
            '\\' => match chars.peek() {
                Some('<') | Some('>') => {
                    chars.next();
                }
                Some('/') => {
                    chars.next();
                    group_count += 1;
                    return Some(group_count);
                }
                Some(_) => {
                    chars.next();
                }
                None => {}
            },
            '(' if chars.peek() != Some(&'?') => {
                group_count += 1;
            }
            _ => {}
        }
    }
    None
}

/// Process a single char inside a `\/` capture region.
/// Returns true if `^^` was seen at pattern end (signals anchor_end).
fn push_char(
    out: &mut String, c: char,
    chars: &mut std::iter::Peekable<std::str::Chars>, groups: &mut usize,
) -> bool {
    match c {
        '\\' => match chars.peek() {
            Some('<') | Some('>') => {
                chars.next();
                out.push_str(r"\b");
            }
            Some(&c2) => {
                chars.next();
                out.push('\\');
                out.push(c2);
            }
            None => {
                out.push_str("\\\\");
            }
        },
        '(' => {
            if chars.peek() != Some(&'?') {
                *groups += 1;
            }
            out.push('(');
        }
        '^' if chars.peek() == Some(&'^') => {
            chars.next();
            // ^^ at end of pattern means anchor_end
            if chars.peek().is_none() {
                return true;
            }
        }
        _ => {
            out.push(c);
        }
    }
    false
}

#[cfg(test)]
mod tests {
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
}
