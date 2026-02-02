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

#[cfg(test)]
mod tests;

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

    /// Count all non-overlapping matches in text. Used for weighted scoring.
    pub fn count_matches(&self, text: &str) -> usize {
        self.regex.find_iter(text).count()
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
