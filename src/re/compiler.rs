//! Procmail regex compiler - wraps the `regex` crate with procmail extensions.
//!
//! Extensions:
//! - `^^` at start: anchor at absolute start of text
//! - `^^` at end: anchor at absolute end of text
//! - `\<` and `\>`: word boundaries (match non-word chars or text boundary)
//! - `\/`: capture point - text after this goes to $MATCH
//! - `^` and `$`: match newlines (multiline mode)

use regex::{Regex, RegexBuilder};

/// Result of a regex match, including any captured MATCH text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchResult {
    /// Whether the pattern matched.
    pub matched: bool,
    /// Text captured after `\/` (for $MATCH variable).
    pub capture: Option<String>,
}

/// Compiled procmail regex matcher.
pub struct Matcher {
    regex: Regex,
    /// Index of capture group for `\/` extraction, if present.
    capture_group: Option<usize>,
    /// Anchor at absolute start of text.
    anchor_start: bool,
    /// Anchor at absolute end of text.
    anchor_end: bool,
}

impl Matcher {
    /// Compile a procmail regex pattern.
    pub fn new(pattern: &str, case_insensitive: bool) -> Result<Self, String> {
        let (compiled, capture_group, anchor_start, anchor_end) =
            compile(pattern)?;

        let regex = RegexBuilder::new(&compiled)
            .case_insensitive(case_insensitive)
            .multi_line(true)
            .build()
            .map_err(|e| e.to_string())?;

        Ok(Self {
            regex,
            capture_group,
            anchor_start,
            anchor_end,
        })
    }

    /// Match against text.
    pub fn exec(&self, text: &str) -> MatchResult {
        let m = self.regex.captures(text);
        let m = match m {
            Some(c) => c,
            None => {
                return MatchResult {
                    matched: false,
                    capture: None,
                };
            }
        };

        let full = m.get(0).unwrap();

        // Check absolute anchors
        if self.anchor_start && full.start() != 0 {
            return MatchResult {
                matched: false,
                capture: None,
            };
        }
        if self.anchor_end && full.end() != text.len() {
            return MatchResult {
                matched: false,
                capture: None,
            };
        }

        let capture = self
            .capture_group
            .and_then(|g| m.get(g).map(|c| c.as_str().to_string()));

        MatchResult {
            matched: true,
            capture,
        }
    }
}

/// Compile a procmail pattern to a Rust regex pattern.
/// Returns (pattern, capture_group_index, anchor_start, anchor_end).
fn compile(pat: &str) -> Result<(String, Option<usize>, bool, bool), String> {
    let mut out = String::with_capacity(pat.len() * 2);
    let mut chars = pat.chars().peekable();
    let mut capture_group = None;
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
                // If at end, we'll set anchor_end after loop
                // For now, mark that we saw ^^ by checking if nothing follows
                if chars.peek().is_none() {
                    anchor_end = true;
                }
            }
            '\\' => {
                match chars.peek() {
                    Some('<') => {
                        chars.next();
                        // Word start boundary - match non-word char or start
                        // Procmail's \< is [^a-zA-Z0-9_] but also matches newlines
                        // Use (?:^|[^a-zA-Z0-9_])
                        out.push_str("(?:^|[^a-zA-Z0-9_])");
                    }
                    Some('>') => {
                        chars.next();
                        // Word end boundary
                        out.push_str("(?:$|[^a-zA-Z0-9_])");
                    }
                    Some('/') => {
                        chars.next();
                        // Start capture for $MATCH
                        group_count += 1;
                        capture_group = Some(group_count);
                        out.push('(');
                        // Capture everything remaining
                        while let Some(c2) = chars.next() {
                            push_char(
                                &mut out,
                                c2,
                                &mut chars,
                                &mut group_count,
                            )?;
                        }
                        out.push(')');
                    }
                    Some(&c2) => {
                        chars.next();
                        // Standard escape
                        out.push('\\');
                        out.push(c2);
                    }
                    None => {
                        out.push_str("\\\\");
                    }
                }
            }
            '(' => {
                group_count += 1;
                out.push('(');
            }
            _ => {
                out.push(c);
            }
        }
        at_start = false;
    }

    Ok((out, capture_group, anchor_start, anchor_end))
}

fn push_char(
    out: &mut String, c: char,
    chars: &mut std::iter::Peekable<std::str::Chars>, groups: &mut usize,
) -> Result<(), String> {
    match c {
        '\\' => match chars.peek() {
            Some('<') => {
                chars.next();
                out.push_str("(?:^|[^a-zA-Z0-9_])");
            }
            Some('>') => {
                chars.next();
                out.push_str("(?:$|[^a-zA-Z0-9_])");
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
            *groups += 1;
            out.push('(');
        }
        '^' if chars.peek() == Some(&'^') => {
            chars.next();
            // ^^ at end of pattern means anchor end
            // Just skip it; anchor_end is handled in main compile
        }
        _ => {
            out.push(c);
        }
    }
    Ok(())
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
}
