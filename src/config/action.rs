use super::Item;
use std::path::PathBuf;

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
            return Action::Forward(addrs);
        }

        // Pipe: [VAR=]| cmd
        // Check for VAR=| pattern
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

        // Nested block: { (handled at parser level, but recognize it)
        if s.starts_with('{') {
            return Action::Nested(Vec::new());
        }

        // Otherwise it's a folder path
        Action::Folder(PathBuf::from(s))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder() {
        match Action::parse_line("/var/mail/spam") {
            Action::Folder(p) => {
                assert_eq!(p.to_str().unwrap(), "/var/mail/spam")
            }
            _ => panic!("expected folder"),
        }
    }

    #[test]
    fn pipe() {
        match Action::parse_line("| /usr/bin/spamassassin") {
            Action::Pipe { cmd, capture } => {
                assert_eq!(cmd, "/usr/bin/spamassassin");
                assert!(capture.is_none());
            }
            _ => panic!("expected pipe"),
        }
    }

    #[test]
    fn pipe_capture() {
        match Action::parse_line("RESULT=| /usr/bin/filter") {
            Action::Pipe { cmd, capture } => {
                assert_eq!(cmd, "/usr/bin/filter");
                assert_eq!(capture.unwrap(), "RESULT");
            }
            _ => panic!("expected pipe with capture"),
        }
    }

    #[test]
    fn forward() {
        match Action::parse_line("! admin@example.com backup@example.com") {
            Action::Forward(addrs) => {
                assert_eq!(addrs.len(), 2);
                assert_eq!(addrs[0], "admin@example.com");
            }
            _ => panic!("expected forward"),
        }
    }

    #[test]
    fn maildir() {
        match Action::parse_line("Maildir/") {
            Action::Folder(p) => assert_eq!(p.to_str().unwrap(), "Maildir/"),
            _ => panic!("expected folder"),
        }
    }
}
