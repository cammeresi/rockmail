use std::iter::Peekable;
use std::str::Chars;

use super::Environment;

#[cfg(test)]
mod tests;

const MAX_SUBST_DEPTH: usize = 32;

/// Callback that executes a shell command and returns captured stdout.
pub type BacktickFn<'a> = &'a dyn Fn(&str) -> String;

/// Signature of top-level substitution functions (`subst_limited`,
/// `subst_quoted`).
pub type SubstFn = fn(
    &Environment,
    &SubstCtx,
    &str,
    usize,
    Option<BacktickFn>,
) -> (String, bool);

/// Holds context for variable substitution (positional args, special vars).
pub struct SubstCtx {
    /// Positional arguments (`$1`, `$2`, ...).
    pub(crate) argv: Vec<String>,
    /// Process ID (`$$`).
    pub(crate) pid: u32,
    /// Last command exit code (`$?`).
    pub(crate) last_exit: i32,
    /// Last scoring result (`$=`).
    pub(crate) last_score: i64,
    /// Current rcfile path (`$_`).
    pub(crate) rcfile: String,
    /// Last folder delivered to (`$-`).
    pub(crate) lastfolder: String,
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
    /// Create a context with positional arguments.
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

/// Collect chars until closing backtick, expanding `$var` inside.
fn collect_backtick(
    chars: &mut Peekable<Chars>, env: &Environment, ctx: &SubstCtx,
    run: Option<BacktickFn>, depth: usize,
) -> String {
    let mut cmd = String::new();
    while let Some(c) = chars.next() {
        if c == '`' {
            return cmd;
        } else if c == '\\'
            && chars.peek().is_some_and(|&n| matches!(n, '`' | '\\' | '$'))
        {
            cmd.push(chars.next().unwrap());
        } else if c == '$' {
            expand_var(chars, ctx, env, &mut cmd, run, depth);
        } else {
            cmd.push(c);
        }
    }
    // Unclosed backtick: use what we collected (matches Procmail)
    cmd
}

fn expand_braced(
    chars: &mut Peekable<Chars>, ctx: &SubstCtx, env: &Environment,
    out: &mut String, run: Option<BacktickFn>, depth: usize,
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
                        _ => out
                            .push_str(&subst_with(env, ctx, &alt, run, depth)),
                    }
                }
                Some('+') => {
                    let alt = collect_to_brace(chars);
                    if let Some(v) = val
                        && !v.is_empty()
                    {
                        out.push_str(&subst_with(env, ctx, &alt, run, depth));
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
                None => out.push_str(&subst_with(env, ctx, &alt, run, depth)),
            }
        }
        Some(&'+') => {
            chars.next();
            let alt = collect_to_brace(chars);
            if val.is_some() {
                out.push_str(&subst_with(env, ctx, &alt, run, depth));
            }
        }
        _ => skip_to_brace(chars),
    }
}

fn expand_var(
    chars: &mut Peekable<Chars>, ctx: &SubstCtx, env: &Environment,
    out: &mut String, run: Option<BacktickFn>, depth: usize,
) {
    match chars.peek() {
        None => out.push('$'),
        Some(&'{') => {
            chars.next();
            expand_braced(chars, ctx, env, out, run, depth);
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

#[derive(Clone, Copy, PartialEq)]
enum Quote {
    None,
    Double,
    Single,
}

fn subst_impl(
    env: &Environment, ctx: &SubstCtx, s: &str, limit: usize,
    run: Option<BacktickFn>, preserve_quotes: bool, depth: usize,
) -> (String, bool) {
    let mut out = String::with_capacity(s.len().min(limit));
    let mut chars = s.chars().peekable();
    let mut q = Quote::None;

    while let Some(c) = chars.next() {
        if c == '\\' && (preserve_quotes || q != Quote::Single) {
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
        } else if !preserve_quotes && c == '"' && q != Quote::Single {
            q = if q == Quote::Double {
                Quote::None
            } else {
                Quote::Double
            };
        } else if !preserve_quotes && c == '\'' && q != Quote::Double {
            q = if q == Quote::Single {
                Quote::None
            } else {
                Quote::Single
            };
        } else if c == '$' && (preserve_quotes || q != Quote::Single) {
            expand_var(&mut chars, ctx, env, &mut out, run, depth);
        } else if c == '`' && (preserve_quotes || q != Quote::Single) {
            if let Some(runner) = run {
                let cmd = collect_backtick(&mut chars, env, ctx, run, depth);
                out.push_str(&runner(&cmd));
            } else {
                out.push(c);
            }
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

/// Internal: expand with optional backtick runner.
fn subst_with(
    env: &Environment, ctx: &SubstCtx, s: &str, run: Option<BacktickFn>,
    depth: usize,
) -> String {
    if depth >= MAX_SUBST_DEPTH {
        eprintln!(
            "variable substitution nested too deeply (limit {MAX_SUBST_DEPTH})"
        );
        return s.to_owned();
    }
    subst_impl(env, ctx, s, usize::MAX, run, true, depth + 1).0
}

/// Expand all `$variable` references in `s`.
pub fn subst(env: &Environment, ctx: &SubstCtx, s: &str) -> String {
    subst_impl(env, ctx, s, usize::MAX, None, true, 0).0
}

/// Like `subst`, but truncates output at `limit` bytes.
/// Returns `(result, overflowed)`.
pub fn subst_limited(
    env: &Environment, ctx: &SubstCtx, s: &str, limit: usize,
    run: Option<BacktickFn>,
) -> (String, bool) {
    subst_impl(env, ctx, s, limit, run, true, 0)
}

/// Expand variables and strip shell-like quotes (assignment context).
/// Matches procmail's `readparse(sarg=1)`.
pub fn subst_quoted(
    env: &Environment, ctx: &SubstCtx, s: &str, limit: usize,
    run: Option<BacktickFn>,
) -> (String, bool) {
    subst_impl(env, ctx, s, limit, run, false, 0)
}
