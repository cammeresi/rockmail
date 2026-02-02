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

    // Look for w^x pattern: optional minus, digits/dots, ^, optional minus, digits/dots
    let mut i = 0;
    let bytes = s.as_bytes();

    // Parse w (weight)
    if i < bytes.len() && bytes[i] == b'-' {
        i += 1;
    }
    let w_start = if i > 0 { 0 } else { i };
    while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
        i += 1;
    }
    if i == w_start || (i == w_start + 1 && bytes[w_start] == b'-') {
        return (None, s);
    }

    // Must have ^
    if i >= bytes.len() || bytes[i] != b'^' {
        return (None, s);
    }
    let w_end = i;
    i += 1;

    // Parse x (exponent)
    if i < bytes.len() && bytes[i] == b'-' {
        i += 1;
    }
    let x_start = w_end + 1;
    while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
        i += 1;
    }
    if i == x_start || (i == x_start + 1 && bytes[x_start] == b'-') {
        return (None, s);
    }

    let Ok(w) = s[..w_end].parse::<f64>() else {
        return (None, s);
    };
    let Ok(x) = s[w_end + 1..i].parse::<f64>() else {
        return (None, s);
    };

    (Some(Weight { w, x }), s[i..].trim_start())
}
