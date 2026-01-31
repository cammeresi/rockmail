use super::{Action, Condition, Flags, Item, Recipe};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("unexpected end of file")]
    UnexpectedEof,
    #[error("missing action line for recipe")]
    MissingAction,
    #[error("unclosed nested block")]
    UnclosedBlock,
    #[error("invalid recipe line: {0}")]
    Invalid(String),
}

/// Parser state
pub struct Parser<'a> {
    lines: Vec<&'a str>,
    pos: usize,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        let lines: Vec<_> = input.lines().collect();
        Self { lines, pos: 0 }
    }

    /// Parse entire rcfile into items
    pub fn parse(&mut self) -> Result<Vec<Item>, ParseError> {
        let mut items = Vec::new();
        while let Some(item) = self.next_item()? {
            items.push(item);
        }
        Ok(items)
    }

    fn peek(&self) -> Option<&'a str> {
        self.lines.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<&'a str> {
        let line = self.lines.get(self.pos).copied();
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

    fn next_item(&mut self) -> Result<Option<Item>, ParseError> {
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

        // Unrecognized line: skip it (procmail ignores garbage)
        Ok(None)
    }

    fn parse_assignment(&self, line: &str) -> Option<Item> {
        // Find = sign
        if let Some(eq) = line.find('=') {
            let name = line[..eq].trim();
            let value = line[eq + 1..].trim();
            if is_var_name(name) {
                return Some(Item::Assign {
                    name: name.to_string(),
                    value: value.to_string(),
                });
            }
        } else {
            // Unset: just the variable name
            let name = line.trim();
            if is_var_name(name) {
                return Some(Item::Assign {
                    name: name.to_string(),
                    value: String::new(),
                });
            }
        }
        None
    }

    fn parse_recipe(&mut self) -> Result<Recipe, ParseError> {
        let header = self.advance().ok_or(ParseError::UnexpectedEof)?;
        let (flags, lockfile) = self.parse_recipe_header(header)?;

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
        let action_line = self.advance().ok_or(ParseError::MissingAction)?;
        let action = self.parse_action(action_line.trim())?;

        Ok(Recipe::new(flags, lockfile, conds, action))
    }

    fn parse_recipe_header(
        &self, line: &str,
    ) -> Result<(Flags, Option<String>), ParseError> {
        // Format: :0 [flags] [ : [lockfile] ]
        let line = line.trim();
        let line = line
            .strip_prefix(':')
            .ok_or_else(|| ParseError::Invalid(line.to_string()))?;

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

    fn parse_action(&mut self, line: &str) -> Result<Action, ParseError> {
        // Handle nested block
        if let Some(rest) = line.strip_prefix('{') {
            // Nested block
            let rest = rest.trim();
            if !rest.is_empty() && !rest.starts_with('#') {
                // Inline content after { is not supported, but we could handle it
            }
            let items = self.parse_block()?;
            return Ok(Action::Nested(items));
        }

        let full = self.collect_continuation(line);
        Ok(Action::parse_line(&full))
    }

    fn parse_block(&mut self) -> Result<Vec<Item>, ParseError> {
        let mut items = Vec::new();
        loop {
            self.skip_blank_and_comments();
            let Some(line) = self.peek() else {
                return Err(ParseError::UnclosedBlock);
            };
            if line.trim().starts_with('}') {
                self.advance();
                break;
            }
            if let Some(item) = self.next_item()? {
                items.push(item);
            } else {
                break;
            }
        }
        Ok(items)
    }

    fn collect_continuation(&mut self, first: &str) -> String {
        let mut result = first.to_string();

        while result.ends_with('\\') {
            result.pop(); // remove trailing backslash
            if let Some(next) = self.advance() {
                result.push_str(next.trim());
            } else {
                break;
            }
        }

        result
    }
}

fn is_var_name(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Convenience function to parse an rcfile
pub fn parse(input: &str) -> Result<Vec<Item>, ParseError> {
    Parser::new(input).parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assignment() {
        let items = parse("MAILDIR=/var/mail\nVERBOSE=yes").unwrap();
        assert_eq!(items.len(), 2);
        match &items[0] {
            Item::Assign { name, value } => {
                assert_eq!(name, "MAILDIR");
                assert_eq!(value, "/var/mail");
            }
            _ => panic!("expected assign"),
        }
    }

    #[test]
    fn simple_recipe() {
        let rc = r#"
:0
* ^From:.*spam
/dev/null
"#;
        let items = parse(rc).unwrap();
        assert_eq!(items.len(), 1);
        match &items[0] {
            Item::Recipe(r) => {
                assert!(r.flags.head);
                assert_eq!(r.conds.len(), 1);
                match &r.action {
                    Action::Folder(p) => {
                        assert_eq!(p.to_str().unwrap(), "/dev/null")
                    }
                    _ => panic!("expected folder"),
                }
            }
            _ => panic!("expected recipe"),
        }
    }

    #[test]
    fn recipe_with_flags() {
        let rc = ":0 Bc:\n* ^Subject:.*test\nspam/";
        let items = parse(rc).unwrap();
        match &items[0] {
            Item::Recipe(r) => {
                assert!(!r.flags.head);
                assert!(r.flags.body);
                assert!(r.flags.copy);
                assert!(r.lockfile.is_some());
            }
            _ => panic!("expected recipe"),
        }
    }

    #[test]
    fn nested_block() {
        let rc = r#"
:0
* ^From:.*important
{
    :0 c
    backup/

    :0
    | /usr/bin/notify
}
"#;
        let items = parse(rc).unwrap();
        assert_eq!(items.len(), 1);
        match &items[0] {
            Item::Recipe(r) => match &r.action {
                Action::Nested(inner) => {
                    assert_eq!(inner.len(), 2);
                }
                _ => panic!("expected nested"),
            },
            _ => panic!("expected recipe"),
        }
    }

    #[test]
    fn forward() {
        let rc = ":0\n* ^To:.*admin\n! admin@example.com";
        let items = parse(rc).unwrap();
        match &items[0] {
            Item::Recipe(r) => match &r.action {
                Action::Forward(addrs) => {
                    assert_eq!(addrs[0], "admin@example.com");
                }
                _ => panic!("expected forward"),
            },
            _ => panic!("expected recipe"),
        }
    }

    #[test]
    fn pipe_capture() {
        let rc = ":0\nRESULT=| /usr/bin/filter";
        let items = parse(rc).unwrap();
        match &items[0] {
            Item::Recipe(r) => match &r.action {
                Action::Pipe { cmd, capture } => {
                    assert_eq!(cmd, "/usr/bin/filter");
                    assert_eq!(capture.as_deref(), Some("RESULT"));
                }
                _ => panic!("expected pipe"),
            },
            _ => panic!("expected recipe"),
        }
    }

    #[test]
    fn comments() {
        let rc = r#"
# This is a comment
MAILDIR=/var/mail  # inline comment not supported, this goes in value

:0  # recipe
* ^From:.*test
/dev/null
"#;
        let items = parse(rc).unwrap();
        assert_eq!(items.len(), 2);
    }
}
