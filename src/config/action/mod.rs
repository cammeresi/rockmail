use std::path::PathBuf;

use super::{HeaderOp, Item, is_var_name};

#[cfg(test)]
mod tests;

/// The action line of a recipe
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Deliver to one or more folders.
    Folder(Vec<PathBuf>),
    /// Pipe to program (optionally capture to variable).
    Pipe {
        /// Shell command to execute.
        cmd: String,
        /// Variable name for captured output (`VAR=|cmd`).
        capture: Option<String>,
    },
    /// Forward to addresses
    Forward(Vec<String>),
    /// Nested block
    Nested(Vec<Item>),
    /// Duplicate detection (`@D maxlen cache`).
    DupeCheck {
        /// Max cache size in bytes.
        maxlen: String,
        /// Path to the cache file.
        cache: String,
    },
    /// Header manipulation (`@i`/`@I`/`@a`/`@A`).
    HeaderOp(HeaderOp),
}

impl Action {
    /// Parse an action line
    pub fn parse_line(s: &str) -> Self {
        let s = s.trim();

        // Forward: ! addr1 addr2 ...
        if let Some(rest) = s.strip_prefix('!') {
            let addrs: Vec<String> =
                rest.split_whitespace().map(|a| a.to_string()).collect();
            if !addrs.is_empty() {
                return Action::Forward(addrs);
            }
            // Empty forward falls through to folder
        }

        // Pipe: VAR=| cmd
        if let Some(eq) = s.find('=') {
            let before = &s[..eq];
            let after = s[eq + 1..].trim_start();
            if after.starts_with('|') && is_var_name(before) {
                let cmd = after[1..].trim_start();
                return Action::Pipe {
                    cmd: cmd.to_string(),
                    capture: Some(before.to_string()),
                };
            }
        }

        // Pipe: | cmd
        if let Some(rest) = s.strip_prefix('|') {
            return Action::Pipe {
                cmd: rest.trim_start().to_string(),
                capture: None,
            };
        }

        if let Some(rest) = s.strip_prefix("@D")
            && let Some((maxlen, cache)) =
                rest.trim_start().split_once(char::is_whitespace)
        {
            return Action::DupeCheck {
                maxlen: maxlen.trim().into(),
                cache: cache.trim().into(),
            };
        }

        if let Some(op) = HeaderOp::parse(s) {
            return Action::HeaderOp(op);
        }

        // Otherwise it's folder path(s) (nested blocks handled at parser level)
        let paths = s.split_whitespace().map(PathBuf::from).collect();
        Action::Folder(paths)
    }
}
