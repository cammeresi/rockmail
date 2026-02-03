use std::cmp::Ordering;

#[cfg(test)]
mod tests;

/// Weight and exponent for scored conditions (w^x syntax).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Weight {
    pub w: f64,
    pub x: f64,
}

/// A condition line in a recipe (starts with *)
#[derive(Debug, Clone)]
pub enum Condition {
    /// Regular expression match (possibly negated with !)
    Regex {
        pattern: String,
        negate: bool,
        weight: Option<Weight>,
    },
    /// Size comparison: < or > bytes
    Size {
        op: Ordering,
        bytes: u64,
        weight: Option<Weight>,
    },
    /// Shell command exit code (? prefix)
    Shell { cmd: String, weight: Option<Weight> },
    /// Variable match: VAR ?? pattern
    Variable {
        name: String,
        pattern: String,
        weight: Option<Weight>,
    },
    /// Substitution prefix ($): expand then reparse, may be negated.
    /// Weight applies to inner condition; negation inverts boolean but not score.
    Subst { inner: Box<Condition>, negate: bool },
}

impl Condition {
    /// Parse a condition line (without the leading *)
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        // Check for w^x weight prefix
        let (weight, s) = parse_weight(s);

        let (s, negate) = if let Some(rest) = s.strip_prefix('!') {
            (rest.trim_start(), true)
        } else {
            (s, false)
        };

        // Check for $ prefix (substitution)
        if let Some(rest) = s.strip_prefix('$') {
            let inner = Self::parse_inner(rest.trim_start(), false, weight)?;
            return Some(Condition::Subst {
                inner: Box::new(inner),
                negate,
            });
        }

        Self::parse_inner(s, negate, weight)
    }

    fn parse_inner(
        s: &str, negate: bool, weight: Option<Weight>,
    ) -> Option<Self> {
        // ? prefix: shell command
        if let Some(cmd) = s.strip_prefix('?') {
            return Some(Condition::Shell {
                cmd: cmd.trim_start().to_string(),
                weight,
            });
        }

        // < prefix: size less than
        if let Some(rest) = s.strip_prefix('<') {
            let bytes = rest.trim().parse().ok()?;
            return Some(Condition::Size {
                op: Ordering::Less,
                bytes,
                weight,
            });
        }

        // > prefix: size greater than
        if let Some(rest) = s.strip_prefix('>') {
            let bytes = rest.trim().parse().ok()?;
            return Some(Condition::Size {
                op: Ordering::Greater,
                bytes,
                weight,
            });
        }

        // VAR ?? pattern
        if let Some(pos) = s.find("??") {
            let name = s[..pos].trim();
            let pattern = s[pos + 2..].trim_start();
            return Some(Condition::Variable {
                name: name.to_string(),
                pattern: pattern.to_string(),
                weight,
            });
        }

        // \ at start: escape the first character
        let pattern = if let Some(rest) = s.strip_prefix('\\') {
            rest.to_string()
        } else {
            s.to_string()
        };

        Some(Condition::Regex {
            pattern,
            negate,
            weight,
        })
    }
}

/// Parse w^x weight prefix from condition. Returns (weight, rest).
fn parse_weight(s: &str) -> (Option<Weight>, &str) {
    let s = s.trim_start();
    let Some(caret) = s.find('^') else {
        return (None, s);
    };

    let w_str = &s[..caret];
    if !is_valid_number(w_str) {
        return (None, s);
    }

    let rest = &s[caret + 1..];
    let x_end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
        .unwrap_or(rest.len());
    let x_str = &rest[..x_end];
    if !is_valid_number(x_str) {
        return (None, s);
    }

    let Ok(w) = w_str.parse::<f64>() else {
        return (None, s);
    };
    let Ok(x) = x_str.parse::<f64>() else {
        return (None, s);
    };

    (Some(Weight { w, x }), rest[x_end..].trim_start())
}

fn is_valid_number(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let s = s.strip_prefix('-').unwrap_or(s);
    if s.is_empty() {
        return false;
    }
    let dots = s.bytes().filter(|&b| b == b'.').count();
    dots <= 1 && s.bytes().all(|b| b.is_ascii_digit() || b == b'.')
}
