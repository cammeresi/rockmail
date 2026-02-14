use std::borrow::Borrow;
use std::iter::Peekable;
use std::str::Lines;

use thiserror::Error;

use super::{Action, Condition, Flags, Item, Recipe, is_var_name};

#[cfg(test)]
mod tests;

const MAX_DEPTH: usize = 100;

#[derive(Error, Debug)]
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

/// Strip an inline comment: `value  # comment` → `value`.
/// Matches procmail (`goodies.c:184`): `#` starts a comment only at a word
/// boundary, i.e. when preceded by whitespace.  Mid-word `#` is literal.
fn strip_comment(s: &str) -> &str {
    let bytes = s.as_bytes();
    for i in 1..bytes.len() {
        if bytes[i] == b'#'
            && (bytes[i - 1] == b' ' || bytes[i - 1] == b'\t')
        {
            return s[..i].trim_end();
        }
    }
    s
}

/// Parser state
pub struct Parser<'a> {
    lines: Peekable<Lines<'a>>,
    pos: usize,
    depth: usize,
}

impl<'a> Parser<'a> {
    pub fn new<T>(input: &'a T) -> Self
    where
        T: Borrow<str> + ?Sized,
    {
        Self {
            lines: input.borrow().lines().peekable(),
            pos: 0,
            depth: 0,
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
        if line.is_some() {
            self.pos += 1;
        }
        line
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

        result
    }

    fn parse_assignment(&self, line: &str) -> Option<Item> {
        if let Some(eq) = line.find('=') {
            let name = line[..eq].trim();
            let value = strip_comment(line[eq + 1..].trim());
            if is_var_name(name) {
                if name == "INCLUDERC" {
                    Some(Item::Include(value.to_string()))
                } else if name == "SWITCHRC" {
                    Some(Item::Switch(value.to_string()))
                } else {
                    Some(Item::Assign {
                        name: name.to_string(),
                        value: value.to_string(),
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
                    Some(Item::Switch(String::new()))
                } else {
                    Some(Item::Assign {
                        name: name.to_string(),
                        value: String::new(),
                    })
                }
            } else {
                None
            }
        }
    }

    fn parse_recipe_header(
        &self, line: &str, line_num: usize,
    ) -> Result<(Flags, Option<String>), ParseError> {
        // Format: :0 [flags] [ : [lockfile] ]
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
        let header = self.advance().ok_or(ParseError::UnexpectedEof(line))?;
        let (flags, lockfile) = self.parse_recipe_header(header, line)?;

        let mut conds = Vec::new();
        self.skip_blank_and_comments();

        // Collect conditions (lines starting with *)
        while let Some(line) = self.peek() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix('*') {
                self.advance();
                // Handle line continuation
                let full = self.collect_continuation(rest);
                if let Some(c) = Condition::parse(&full) {
                    conds.push(c);
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

            // Recipe starts with :
            if trimmed.starts_with(':') {
                return self.parse_recipe().map(|r| Some(Item::Recipe(r)));
            }

            // Closing brace: return None to end nested block
            if trimmed.starts_with('}') {
                return Ok(None);
            }

            // Variable assignment: NAME=value or NAME (unset)
            self.advance();
            if let Some(item) = self.parse_assignment(trimmed) {
                return Ok(Some(item));
            }

            // Unrecognized line: skip and continue loop
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
pub fn parse(input: &str) -> Result<Vec<Item>, ParseError> {
    Parser::new(input).parse()
}
