use std::path::PathBuf;

use super::{Item, is_var_name};

#[cfg(test)]
mod tests;

/// The action line of a recipe
#[derive(Debug, Clone)]
pub enum Action {
    /// Deliver to a file/directory
    Folder(PathBuf),
    /// Pipe to program (optionally capture to variable)
    Pipe {
        cmd: String,
        capture: Option<String>,
    },
    /// Forward to addresses
    Forward(Vec<String>),
    /// Nested block
    Nested(Vec<Item>),
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

        // Otherwise it's a folder path (nested blocks handled at parser level)
        Action::Folder(PathBuf::from(s))
    }
}
