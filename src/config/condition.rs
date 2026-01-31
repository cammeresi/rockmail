use std::cmp::Ordering;

/// A condition line in a recipe (starts with *)
#[derive(Debug, Clone)]
pub enum Condition {
    /// Regular expression match (possibly negated with !)
    Regex { pattern: String, negate: bool },
    /// Size comparison: < or > bytes
    Size { op: Ordering, bytes: u64 },
    /// Shell command exit code (? prefix)
    Shell { cmd: String },
    /// Variable match: VAR ?? pattern
    Variable { name: String, pattern: String },
    /// Substitution prefix ($): expand then reparse, may be negated
    Subst { inner: Box<Condition>, negate: bool },
}

impl Condition {
    /// Parse a condition line (without the leading *)
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        let (s, negate) = if let Some(rest) = s.strip_prefix('!') {
            (rest.trim_start(), true)
        } else {
            (s, false)
        };

        // Check for $ prefix (substitution)
        if let Some(rest) = s.strip_prefix('$') {
            let inner = Self::parse_inner(rest.trim_start(), false)?;
            return Some(Condition::Subst {
                inner: Box::new(inner),
                negate,
            });
        }

        Self::parse_inner(s, negate)
    }

    fn parse_inner(s: &str, negate: bool) -> Option<Self> {
        // ? prefix: shell command
        if let Some(cmd) = s.strip_prefix('?') {
            return Some(Condition::Shell {
                cmd: cmd.trim_start().to_string(),
            });
        }

        // < prefix: size less than
        if let Some(rest) = s.strip_prefix('<') {
            let bytes = rest.trim().parse().ok()?;
            return Some(Condition::Size {
                op: Ordering::Less,
                bytes,
            });
        }

        // > prefix: size greater than
        if let Some(rest) = s.strip_prefix('>') {
            let bytes = rest.trim().parse().ok()?;
            return Some(Condition::Size {
                op: Ordering::Greater,
                bytes,
            });
        }

        // VAR ?? pattern
        if let Some(pos) = s.find("??") {
            let name = s[..pos].trim();
            let pattern = s[pos + 2..].trim_start();
            // H, B, HB, BH are special (override grep area)
            return Some(Condition::Variable {
                name: name.to_string(),
                pattern: pattern.to_string(),
            });
        }

        // \ at start: escape the first character
        let pattern = if let Some(rest) = s.strip_prefix('\\') {
            rest.to_string()
        } else {
            s.to_string()
        };

        Some(Condition::Regex { pattern, negate })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regex() {
        let c = Condition::parse("^From:.*spam").unwrap();
        match c {
            Condition::Regex { pattern, negate } => {
                assert_eq!(pattern, "^From:.*spam");
                assert!(!negate);
            }
            _ => panic!("expected regex"),
        }
    }

    #[test]
    fn negated() {
        let c = Condition::parse("! ^From:.*friend").unwrap();
        match c {
            Condition::Regex { pattern, negate } => {
                assert_eq!(pattern, "^From:.*friend");
                assert!(negate);
            }
            _ => panic!("expected regex"),
        }
    }

    #[test]
    fn size() {
        let c = Condition::parse("< 10000").unwrap();
        match c {
            Condition::Size { op, bytes } => {
                assert_eq!(op, Ordering::Less);
                assert_eq!(bytes, 10000);
            }
            _ => panic!("expected size"),
        }
    }

    #[test]
    fn shell() {
        let c = Condition::parse("? test -f /tmp/flag").unwrap();
        match c {
            Condition::Shell { cmd } => {
                assert_eq!(cmd, "test -f /tmp/flag");
            }
            _ => panic!("expected shell"),
        }
    }

    #[test]
    fn variable() {
        let c = Condition::parse("SENDER ?? ^admin").unwrap();
        match c {
            Condition::Variable { name, pattern } => {
                assert_eq!(name, "SENDER");
                assert_eq!(pattern, "^admin");
            }
            _ => panic!("expected variable"),
        }
    }

    #[test]
    fn subst() {
        let c = Condition::parse("$ ^From:.*${SENDER}").unwrap();
        match c {
            Condition::Subst { inner, negate } => {
                assert!(!negate);
                match *inner {
                    Condition::Regex { pattern, .. } => {
                        assert_eq!(pattern, "^From:.*${SENDER}");
                    }
                    _ => panic!("expected inner regex"),
                }
            }
            _ => panic!("expected subst"),
        }
    }

    #[test]
    fn negated_subst() {
        let c = Condition::parse("! $ ^From:.*${SENDER}").unwrap();
        match c {
            Condition::Subst { inner, negate } => {
                assert!(negate);
                match *inner {
                    Condition::Regex { pattern, negate } => {
                        assert_eq!(pattern, "^From:.*${SENDER}");
                        assert!(!negate);
                    }
                    _ => panic!("expected inner regex"),
                }
            }
            _ => panic!("expected subst"),
        }
    }

    #[test]
    fn escape() {
        let c = Condition::parse("\\!literal").unwrap();
        match c {
            Condition::Regex { pattern, negate } => {
                assert_eq!(pattern, "!literal");
                assert!(!negate);
            }
            _ => panic!("expected regex"),
        }
    }

    #[test]
    fn size_greater() {
        let c = Condition::parse("> 50000").unwrap();
        match c {
            Condition::Size { op, bytes } => {
                assert_eq!(op, Ordering::Greater);
                assert_eq!(bytes, 50000);
            }
            _ => panic!("expected size"),
        }
    }

    #[test]
    fn empty_returns_none() {
        assert!(Condition::parse("").is_none());
        assert!(Condition::parse("   ").is_none());
    }

    #[test]
    fn invalid_size_returns_none() {
        assert!(Condition::parse("< notanumber").is_none());
    }
}
