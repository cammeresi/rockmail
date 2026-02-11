use std::cmp::Ordering;

#[cfg(test)]
mod tests;

/// Weight and exponent for scored conditions (w^x syntax).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Weight {
    pub w: f64,
    pub x: f64,
}

/// Parse w^x weight prefix from condition. Returns (weight, rest).
fn parse_weight(s: &str) -> (Option<Weight>, &str) {
    let s = s.trim_start();
    let Some(caret) = s.find('^') else {
        return (None, s);
    };

    let Ok(w) = s[..caret].parse::<f64>() else {
        return (None, s);
    };

    let rest = &s[caret + 1..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
        .unwrap_or(rest.len());

    let Ok(x) = rest[..end].parse::<f64>() else {
        return (None, s);
    };

    (Some(Weight { w, x }), rest[end..].trim_start())
}

fn parse_shell(s: &str, weight: Option<Weight>) -> Option<Condition> {
    let cmd = s.strip_prefix('?')?;
    Some(Condition::Shell {
        cmd: cmd.trim_start().to_string(),
        weight,
    })
}

fn parse_size(
    rest: &str, weight: Option<Weight>, op: Ordering,
) -> Option<Condition> {
    let bytes = rest.trim().parse().ok()?;
    Some(Condition::Size { op, bytes, weight })
}

fn parse_size_less(s: &str, weight: Option<Weight>) -> Option<Condition> {
    let rest = s.strip_prefix('<')?;
    parse_size(rest, weight, Ordering::Less)
}

fn parse_size_greater(s: &str, weight: Option<Weight>) -> Option<Condition> {
    let rest = s.strip_prefix('>')?;
    parse_size(rest, weight, Ordering::Greater)
}

fn parse_variable(s: &str, weight: Option<Weight>) -> Option<Condition> {
    let pos = s.find("??")?;
    let name = s[..pos].trim();
    let pattern = s[pos + 2..].trim_start();
    Some(Condition::Variable {
        name: name.to_string(),
        pattern: pattern.to_string(),
        weight,
    })
}

fn parse_regex(s: &str, negate: bool, weight: Option<Weight>) -> Condition {
    let pattern = s.strip_prefix('\\').unwrap_or(s).to_string();
    Condition::Regex {
        pattern,
        negate,
        weight,
    }
}

fn parse_subst(
    s: &str, negate: bool, weight: Option<Weight>,
) -> Option<Condition> {
    let rest = s.strip_prefix('$')?;
    let inner = parse_inner(rest.trim_start(), false, weight)?;
    Some(Condition::Subst {
        inner: Box::new(inner),
        negate,
    })
}

fn parse_inner(
    s: &str, negate: bool, weight: Option<Weight>,
) -> Option<Condition> {
    // These prefixes commit: if present but malformed, return None
    if s.starts_with('$') {
        return parse_subst(s, negate, weight);
    }
    if s.starts_with('?') {
        return parse_shell(s, weight);
    }
    if s.starts_with('<') {
        return parse_size_less(s, weight);
    }
    if s.starts_with('>') {
        return parse_size_greater(s, weight);
    }
    if s.contains("??") {
        return parse_variable(s, weight);
    }
    Some(parse_regex(s, negate, weight))
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
    /// Weight applies to inner condition; negation inverts boolean but not
    /// score.
    Subst { inner: Box<Condition>, negate: bool },
}

impl Condition {
    /// Parse a condition line (without the leading *)
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }

        let (weight, s) = parse_weight(s);

        let (s, negate) = if let Some(rest) = s.strip_prefix('!') {
            (rest.trim_start(), true)
        } else {
            (s, false)
        };

        parse_inner(s, negate, weight)
    }
}
