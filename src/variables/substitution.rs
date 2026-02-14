use std::iter::Peekable;
use std::str::Chars;

use super::Environment;

#[cfg(test)]
mod tests;

/// Holds context for variable substitution (positional args, special vars)
pub struct SubstCtx {
    pub argv: Vec<String>,
    pub pid: u32,
    pub last_exit: i32,
    pub last_score: i64,
    pub rcfile: String,
    pub lastfolder: String,
}

impl Default for SubstCtx {
    fn default() -> Self {
        Self {
            argv: Vec::new(),
            pid: std::process::id(),
            last_exit: 0,
            last_score: 0,
            rcfile: String::new(),
            lastfolder: String::new(),
        }
    }
}

impl SubstCtx {
    pub fn new(argv: Vec<String>) -> Self {
        Self {
            argv,
            ..Default::default()
        }
    }
}

/// Escape a value for literal use in a procmail regex (goodies.c:286-290).
fn regex_escape_into(s: &str, out: &mut String) {
    const RE_META: &str = "(|)*?+.^$[\\";
    let mut cs = s.chars();
    let Some(first) = cs.next() else { return };
    out.push('(');
    if RE_META.contains(first) {
        out.push('\\');
    }
    out.push(first);
    out.push(')');
    for c in cs {
        if RE_META.contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
}

fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn collect_name(chars: &mut Peekable<Chars>) -> String {
    let mut name = String::new();
    while let Some(&c) = chars.peek() {
        if is_name_char(c) {
            name.push(c);
            chars.next();
        } else {
            break;
        }
    }
    name
}

fn collect_to_brace(chars: &mut Peekable<Chars>) -> String {
    let mut s = String::new();
    let mut depth = 1;
    for c in chars.by_ref() {
        if c == '{' {
            depth += 1;
            s.push(c);
        } else if c == '}' {
            depth -= 1;
            if depth == 0 {
                break;
            }
            s.push(c);
        } else {
            s.push(c);
        }
    }
    s
}

fn skip_to_brace(chars: &mut Peekable<Chars>) {
    let mut depth = 1;
    for c in chars.by_ref() {
        if c == '{' {
            depth += 1;
        } else if c == '}' {
            depth -= 1;
            if depth == 0 {
                break;
            }
        }
    }
}

fn expand_braced(
    chars: &mut Peekable<Chars>, ctx: &SubstCtx, env: &Environment,
    out: &mut String,
) {
    let name = collect_name(chars);
    if name.is_empty() {
        skip_to_brace(chars);
        return;
    }

    let val = env.get(&name);

    match chars.peek() {
        Some(&'}') => {
            chars.next();
            if let Some(v) = val {
                out.push_str(v);
            }
        }
        Some(&':') => {
            chars.next();
            match chars.next() {
                Some('-') => {
                    let alt = collect_to_brace(chars);
                    match val {
                        Some(v) if !v.is_empty() => out.push_str(v),
                        _ => out.push_str(&subst(env, ctx, &alt)),
                    }
                }
                Some('+') => {
                    let alt = collect_to_brace(chars);
                    if let Some(v) = val
                        && !v.is_empty()
                    {
                        out.push_str(&subst(env, ctx, &alt));
                    }
                }
                _ => skip_to_brace(chars),
            }
        }
        Some(&'-') => {
            chars.next();
            let alt = collect_to_brace(chars);
            match val {
                Some(v) => out.push_str(v),
                None => out.push_str(&subst(env, ctx, &alt)),
            }
        }
        Some(&'+') => {
            chars.next();
            let alt = collect_to_brace(chars);
            if val.is_some() {
                out.push_str(&subst(env, ctx, &alt));
            }
        }
        _ => skip_to_brace(chars),
    }
}

fn expand_var(
    chars: &mut Peekable<Chars>, ctx: &SubstCtx, env: &Environment,
    out: &mut String,
) {
    match chars.peek() {
        None => out.push('$'),
        Some(&'{') => {
            chars.next();
            expand_braced(chars, ctx, env, out);
        }
        Some(&'$') => {
            chars.next();
            out.push_str(&ctx.pid.to_string());
        }
        Some(&'?') => {
            chars.next();
            out.push_str(&ctx.last_exit.to_string());
        }
        Some(&'#') => {
            chars.next();
            out.push_str(&ctx.argv.len().to_string());
        }
        Some(&'_') => {
            chars.next();
            out.push_str(&ctx.rcfile);
        }
        Some(&'-') => {
            chars.next();
            out.push_str(&ctx.lastfolder);
        }
        Some(&'=') => {
            chars.next();
            out.push_str(&ctx.last_score.to_string());
        }
        Some(&c) if c.is_ascii_digit() => {
            chars.next();
            let idx = (c as usize) - ('0' as usize);
            if idx == 0 {
                // $0 is program name, not tracked here
            } else if let Some(arg) = ctx.argv.get(idx - 1) {
                out.push_str(arg);
            }
        }
        Some(&'\\') => {
            chars.next();
            if chars.peek().is_some_and(|&c| is_name_start(c)) {
                let name = collect_name(chars);
                if let Some(val) = env.get(&name) {
                    regex_escape_into(val, out);
                }
            } else {
                out.push('$');
                out.push('\\');
            }
        }
        Some(&c) if is_name_start(c) => {
            let name = collect_name(chars);
            if let Some(val) = env.get(&name) {
                out.push_str(val);
            }
        }
        Some(_) => out.push('$'),
    }
}

pub fn subst(env: &Environment, ctx: &SubstCtx, s: &str) -> String {
    subst_limited(env, ctx, s, usize::MAX).0
}

/// Like `subst`, but truncates output at `limit` bytes.
/// Returns `(result, overflowed)`.
pub fn subst_limited(
    env: &Environment, ctx: &SubstCtx, s: &str, limit: usize,
) -> (String, bool) {
    let mut out = String::with_capacity(s.len().min(limit));
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next) = chars.peek()
                && (next == '$'
                    || next == '\\'
                    || next == '"'
                    || next == '\''
                    || next == '`')
            {
                out.push(chars.next().unwrap());
            } else {
                out.push(c);
            }
        } else if c == '$' {
            expand_var(&mut chars, ctx, env, &mut out);
        } else {
            out.push(c);
        }
        if out.len() >= limit {
            out.truncate(limit);
            return (out, true);
        }
    }
    (out, false)
}
