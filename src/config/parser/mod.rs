use std::borrow::Borrow;
use std::iter::Peekable;
use std::str::Lines;

use miette::{NamedSource, Report, SourceOffset};
use thiserror::Error;

use super::{Action, Condition, Flags, Item, Recipe, is_var_name};
pub use warnings::ParseWarning;

#[cfg(test)]
mod tests;

const MAX_DEPTH: usize = 100;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum ParseError {
    #[error("line {0}: unexpected end of file")]
    UnexpectedEof(usize),
    #[error("line {0}: missing action line for recipe")]
    MissingAction(usize),
    #[error("line {0}: unclosed nested block")]
    UnclosedBlock(usize),
    #[error("line {0}: invalid recipe line: {1}")]
    Invalid(usize, String),
    #[error("line {0}: nesting too deep (max {MAX_DEPTH})")]
    TooDeep(usize),
}

mod warnings {
    // false positive from thiserror/miette derives
    #![allow(unused_assignments)]

    use core::mem;

    use miette::{Diagnostic, NamedSource, SourceOffset};
    use thiserror::Error;

    /// Non-fatal issues found while parsing an rcfile.
    #[derive(Error, Debug, Clone, Diagnostic)]
    pub enum ParseWarning {
        /// Unrecognized line that was skipped.
        #[error("skipped unrecognized line")]
        #[diagnostic(
            code(rockmail::skipped_line),
            help("expected recipe, assignment, or comment")
        )]
        SkippedLine {
            /// Source text of the rcfile.
            #[source_code]
            src: NamedSource<String>,
            /// Span of the offending line.
            #[label("this line")]
            span: SourceOffset,
        },

        /// Assignment with an invalid variable name.
        #[error("invalid variable name in assignment")]
        #[diagnostic(
            code(rockmail::bad_var_name),
            help("variable names must start with a letter or underscore")
        )]
        BadVarName {
            /// Source text of the rcfile.
            #[source_code]
            src: NamedSource<String>,
            /// Span of the offending line.
            #[label("bad name")]
            span: SourceOffset,
        },

        /// Condition that could not be parsed.
        #[error("unparseable condition")]
        #[diagnostic(
            code(rockmail::bad_condition),
            help("condition was ignored")
        )]
        BadCondition {
            /// Source text of the rcfile.
            #[source_code]
            src: NamedSource<String>,
            /// Span of the offending line.
            #[label("here")]
            span: SourceOffset,
        },

        /// Unrecognized recipe flag character.
        #[error("unknown recipe flag: {flag}")]
        #[diagnostic(
            code(rockmail::unknown_flag),
            help("valid flags: HBDaAeEhbfcwWir")
        )]
        UnknownFlag {
            /// The flag character.
            flag: char,
            /// Source text of the rcfile.
            #[source_code]
            src: NamedSource<String>,
            /// Span of the flag character.
            #[label("this flag")]
            span: SourceOffset,
        },
    }

    impl PartialEq for ParseWarning {
        fn eq(&self, other: &Self) -> bool {
            match (self, other) {
                (
                    Self::UnknownFlag { flag: a, .. },
                    Self::UnknownFlag { flag: b, .. },
                ) => a == b,
                _ => mem::discriminant(self) == mem::discriminant(other),
            }
        }
    }

    impl Eq for ParseWarning {}
}

/// True if `s` contains an unmatched `"` or `'`.
/// Tracks quote state the same way procmail does: `'` inside `"` is
/// literal and vice versa, `\"` escapes only outside single quotes.
fn has_unclosed_quote(s: &str) -> bool {
    let mut q = None; // None, Some('"'), Some('\'')
    let mut esc = false;
    for c in s.chars() {
        if esc {
            esc = false;
            continue;
        }
        if c == '\\' && q != Some('\'') {
            esc = true;
            continue;
        }
        match q {
            None if c == '"' || c == '\'' => q = Some(c),
            Some(open) if c == open => q = None,
            _ => {}
        }
    }
    q.is_some()
}

/// Strip an inline comment: `value  # comment` → `value`.
/// Matches procmail (`goodies.c:184`): `#` starts a comment only at a word
/// boundary, i.e. when preceded by whitespace.  Mid-word `#` is literal.
fn strip_comment(s: &str) -> &str {
    let bytes = s.as_bytes();
    for i in 1..bytes.len() {
        if bytes[i] == b'#' && (bytes[i - 1] == b' ' || bytes[i - 1] == b'\t') {
            return s[..i].trim_end();
        }
    }
    s
}

/// Parser state
pub struct Parser<'a> {
    input: &'a str,
    name: String,
    lines: Peekable<Lines<'a>>,
    pos: usize,
    offset: usize,
    depth: usize,
    warnings: Vec<ParseWarning>,
}

impl<'a> Parser<'a> {
    pub fn new<T>(input: &'a T) -> Self
    where
        T: Borrow<str> + ?Sized,
    {
        Self {
            input: input.borrow(),
            name: String::from("rcfile"),
            lines: input.borrow().lines().peekable(),
            pos: 0,
            offset: 0,
            depth: 0,
            warnings: Vec::new(),
        }
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    fn warn(&mut self, w: ParseWarning) {
        self.warnings.push(w);
    }

    #[cfg(test)]
    pub fn warnings(&self) -> &[ParseWarning] {
        &self.warnings
    }

    pub fn emit_warnings(&self) {
        for w in &self.warnings {
            eprintln!("{:?}", Report::new(w.clone()));
        }
    }

    fn line_num(&self) -> usize {
        self.pos + 1
    }

    fn peek(&mut self) -> Option<&'a str> {
        self.lines.peek().map(|v| &**v)
    }

    fn advance(&mut self) -> Option<&'a str> {
        let line = self.lines.next();
        if let Some(l) = line {
            self.pos += 1;
            // +1 for the newline (or end of input)
            self.offset += l.len() + 1;
        }
        line
    }

    /// Byte offset of the line about to be consumed (the peeked line).
    fn cur_offset(&self) -> usize {
        self.offset
    }

    fn src(&self) -> NamedSource<String> {
        NamedSource::new(&self.name, self.input.to_string())
    }

    fn skip_blank_and_comments(&mut self) {
        while let Some(line) = self.peek() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                self.advance();
            } else {
                break;
            }
        }
    }

    fn collect_continuation(&mut self, first: &str) -> String {
        let mut result = first.to_string();

        while result.ends_with('\\') {
            result.pop();
            if let Some(next) = self.advance() {
                result.push_str(next.trim());
            } else {
                break;
            }
        }

        while has_unclosed_quote(&result) {
            if let Some(next) = self.advance() {
                result.push('\n');
                result.push_str(next);
            } else {
                break;
            }
        }

        result
    }

    /// Parse `VAR =~ s/pat/rep/flags`.
    fn parse_subst(line: &str, line_num: usize) -> Option<Item> {
        let pos = line.find("=~")?;
        let name = line[..pos].trim();
        if !is_var_name(name) {
            return None;
        }
        let rhs = line[pos + 2..].trim();
        let mut chars = rhs.chars();
        if chars.next()? != 's' {
            return None;
        }
        let delim = chars.next()?;
        if delim.is_alphanumeric() {
            return None;
        }
        let rest = &rhs[2..];
        let mid = rest.find(delim)?;
        let pattern = &rest[..mid];
        let after = &rest[mid + delim.len_utf8()..];
        let end = after.find(delim)?;
        let replace = &after[..end];
        let flags = &after[end + delim.len_utf8()..];
        Some(Item::Subst {
            name: name.to_string(),
            pattern: pattern.to_string(),
            replace: replace.to_string(),
            global: flags.contains('g'),
            case_insensitive: flags.contains('i'),
            line: line_num,
        })
    }

    fn parse_assignment(&self, line: &str, line_num: usize) -> Option<Item> {
        if let Some(eq) = line.find('=') {
            let name = line[..eq].trim();
            let value = strip_comment(line[eq + 1..].trim());
            if is_var_name(name) {
                if name == "INCLUDERC" {
                    Some(Item::Include {
                        path: value.to_string(),
                        line: line_num,
                    })
                } else if name == "SWITCHRC" {
                    Some(Item::Switch {
                        path: value.to_string(),
                        line: line_num,
                    })
                } else {
                    Some(Item::Assign {
                        name: name.to_string(),
                        value: value.to_string(),
                        line: line_num,
                    })
                }
            } else {
                None
            }
        } else {
            // unset
            let name = line.trim();
            if is_var_name(name) {
                if name == "SWITCHRC" {
                    // aborts processing
                    Some(Item::Switch {
                        path: String::new(),
                        line: line_num,
                    })
                } else {
                    Some(Item::Assign {
                        name: name.to_string(),
                        value: String::new(),
                        line: line_num,
                    })
                }
            } else {
                None
            }
        }
    }

    fn parse_recipe_header(
        &mut self, line: &str, line_num: usize, line_offset: usize,
    ) -> Result<(Flags, Option<String>), ParseError> {
        // Format: :0 [flags] [ : [lockfile] ]
        let orig = line;
        let line = strip_comment(line.trim());
        let line = line
            .strip_prefix(':')
            .ok_or_else(|| ParseError::Invalid(line_num, line.to_string()))?;

        // Skip the leading number (legacy, usually 0)
        let line = line.trim_start_matches(|c: char| c.is_ascii_digit());
        let line = line.trim_start();

        // Check for trailing colon (local lockfile)
        let (flags_part, lockfile) = if let Some(colon_pos) = line.rfind(':') {
            let flags_str = line[..colon_pos].trim();
            let lock_str = line[colon_pos + 1..].trim();
            let lockfile = if lock_str.is_empty() {
                Some(String::new()) // auto-generate lockfile
            } else {
                Some(lock_str.to_string())
            };
            (flags_str, lockfile)
        } else {
            (line, None)
        };

        let flags = Flags::parse(flags_part);
        let src = self.src();
        for &flag in &flags.unknown {
            // Find the flag char's position within the original line
            let fpos = orig.find(flag).unwrap_or(0);
            self.warn(ParseWarning::UnknownFlag {
                flag,
                src: src.clone(),
                span: SourceOffset::from(line_offset + fpos),
            });
        }
        Ok((flags, lockfile))
    }

    fn parse_block(&mut self, start: usize) -> Result<Vec<Item>, ParseError> {
        if self.depth >= MAX_DEPTH {
            return Err(ParseError::TooDeep(start));
        }
        self.depth += 1;
        let mut items = Vec::new();
        loop {
            self.skip_blank_and_comments();
            let Some(line) = self.peek() else {
                self.depth -= 1;
                return Err(ParseError::UnclosedBlock(start));
            };
            if line.trim().starts_with('}') {
                self.advance();
                break;
            }
            if let Some(item) = self.next_item()? {
                items.push(item);
            } else {
                // next_item returned None, meaning } or EOF
                // We already checked for } above, so this is EOF
                self.depth -= 1;
                return Err(ParseError::UnclosedBlock(start));
            }
        }
        self.depth -= 1;
        Ok(items)
    }

    fn parse_action(
        &mut self, line: &str, line_num: usize,
    ) -> Result<Action, ParseError> {
        // Handle nested block
        if let Some(rest) = line.strip_prefix('{') {
            let rest = rest.trim();
            // Check for inline block content: { ... }
            if let Some(inner) = rest.strip_suffix('}') {
                let inner = inner.trim();
                if inner.is_empty() {
                    return Ok(Action::Nested(vec![]));
                }
                // Parse inline content
                let mut p = Parser::new(inner);
                p.depth = self.depth;
                let items = p.parse()?;
                return Ok(Action::Nested(items));
            }
            let items = self.parse_block(line_num)?;
            return Ok(Action::Nested(items));
        }

        let full = self.collect_continuation(line);
        Ok(Action::parse_line(&full))
    }

    fn parse_recipe(&mut self) -> Result<Recipe, ParseError> {
        let line = self.line_num();
        let hoff = self.cur_offset();
        let header = self.advance().ok_or(ParseError::UnexpectedEof(line))?;
        let (flags, lockfile) = self.parse_recipe_header(header, line, hoff)?;

        let mut conds = Vec::new();
        self.skip_blank_and_comments();

        // Collect conditions (lines starting with *)
        while let Some(line) = self.peek() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix('*') {
                let coff = self.cur_offset();
                self.advance();
                let full = self.collect_continuation(rest);
                if let Some(c) = Condition::parse(&full) {
                    conds.push(c);
                } else if !full.trim().is_empty() {
                    self.warn(ParseWarning::BadCondition {
                        src: self.src(),
                        span: SourceOffset::from(coff),
                    });
                }
            } else {
                break;
            }
        }

        self.skip_blank_and_comments();

        // Action line
        let aline = self.line_num();
        let action_line =
            self.advance().ok_or(ParseError::MissingAction(aline))?;
        let action = self.parse_action(action_line.trim(), aline)?;

        Ok(Recipe::new(flags, lockfile, conds, action))
    }

    fn next_item(&mut self) -> Result<Option<Item>, ParseError> {
        loop {
            self.skip_blank_and_comments();
            let Some(line) = self.peek() else {
                return Ok(None);
            };

            let trimmed = line.trim();
            let ln = self.line_num();

            // Recipe starts with :
            if trimmed.starts_with(':') {
                return self.parse_recipe().map(|r| {
                    Some(Item::Recipe {
                        recipe: r,
                        line: ln,
                    })
                });
            }

            // Closing brace: return None to end nested block
            if trimmed.starts_with('}') {
                return Ok(None);
            }

            let off = self.cur_offset();
            self.advance();
            let full = self.collect_continuation(trimmed);

            if let Some(item) = Self::parse_subst(&full, ln) {
                return Ok(Some(item));
            }

            if let Some(item) = self.parse_assignment(&full, ln) {
                return Ok(Some(item));
            }

            let src = self.src();
            let span = SourceOffset::from(off);
            if full.contains('=') {
                self.warn(ParseWarning::BadVarName { src, span });
            } else {
                self.warn(ParseWarning::SkippedLine { src, span });
            }
        }
    }

    /// Parse entire rcfile into items
    pub fn parse(&mut self) -> Result<Vec<Item>, ParseError> {
        let mut items = Vec::new();
        while let Some(item) = self.next_item()? {
            items.push(item);
        }
        Ok(items)
    }
}

/// Convenience function to parse an rcfile
pub fn parse(input: &str, name: &str) -> Result<Vec<Item>, ParseError> {
    let mut p = Parser::new(input);
    p.set_name(name);
    let items = p.parse()?;
    p.emit_warnings();
    Ok(items)
}
