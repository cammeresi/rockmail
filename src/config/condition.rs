use std::cmp::Ordering;

#[cfg(test)]
mod tests;

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
