use std::collections::HashMap;
use std::env;

/// Trait for environment variable access (allows mocking in tests)
pub trait Env {
    fn get(&self, name: &str) -> Option<String>;
}

/// Real environment
pub struct RealEnv;

impl Env for RealEnv {
    fn get(&self, name: &str) -> Option<String> {
        env::var(name).ok()
    }
}

/// Mock environment for testing
#[derive(Default)]
pub struct MockEnv {
    vars: HashMap<String, String>,
}

impl MockEnv {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, name: &str, value: &str) {
        self.vars.insert(name.to_string(), value.to_string());
    }
}

impl Env for MockEnv {
    fn get(&self, name: &str) -> Option<String> {
        self.vars.get(name).cloned()
    }
}

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

/// Expand variable substitutions using real environment
pub fn expand(s: &str, ctx: &SubstCtx) -> String {
    expand_with_env(s, ctx, &RealEnv)
}

/// Expand variable substitutions with custom environment
pub fn expand_with_env(s: &str, ctx: &SubstCtx, env: &dyn Env) -> String {
    let mut out = String::with_capacity(s.len());
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
                continue;
            }
            out.push(c);
        } else if c == '$' {
            expand_var(&mut chars, ctx, env, &mut out);
        } else {
            out.push(c);
        }
    }
    out
}

fn expand_var(
    chars: &mut std::iter::Peekable<std::str::Chars>, ctx: &SubstCtx,
    env: &dyn Env, out: &mut String,
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
        Some(&c) if is_name_start(c) => {
            let name = collect_name(chars);
            if let Some(val) = env.get(&name) {
                out.push_str(&val);
            }
        }
        Some(_) => out.push('$'),
    }
}

fn expand_braced(
    chars: &mut std::iter::Peekable<std::str::Chars>, ctx: &SubstCtx,
    env: &dyn Env, out: &mut String,
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
                out.push_str(&v);
            }
        }
        Some(&':') => {
            chars.next();
            match chars.next() {
                Some('-') => {
                    let alt = collect_to_brace(chars);
                    match val {
                        Some(ref v) if !v.is_empty() => out.push_str(v),
                        _ => out.push_str(&expand_with_env(&alt, ctx, env)),
                    }
                }
                Some('+') => {
                    let alt = collect_to_brace(chars);
                    if let Some(ref v) = val
                        && !v.is_empty()
                    {
                        out.push_str(&expand_with_env(&alt, ctx, env));
                    }
                }
                _ => skip_to_brace(chars),
            }
        }
        Some(&'-') => {
            chars.next();
            let alt = collect_to_brace(chars);
            match val {
                Some(ref v) => out.push_str(v),
                None => out.push_str(&expand_with_env(&alt, ctx, env)),
            }
        }
        Some(&'+') => {
            chars.next();
            let alt = collect_to_brace(chars);
            if val.is_some() {
                out.push_str(&expand_with_env(&alt, ctx, env));
            }
        }
        _ => skip_to_brace(chars),
    }
}

fn is_name_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn collect_name(chars: &mut std::iter::Peekable<std::str::Chars>) -> String {
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

fn collect_to_brace(
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> String {
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

fn skip_to_brace(chars: &mut std::iter::Peekable<std::str::Chars>) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_var() {
        let mut env = MockEnv::new();
        env.set("TEST_VAR", "hello");
        let ctx = SubstCtx::default();
        assert_eq!(expand_with_env("$TEST_VAR", &ctx, &env), "hello");
        assert_eq!(expand_with_env("${TEST_VAR}", &ctx, &env), "hello");
    }

    #[test]
    fn default_value() {
        let mut env = MockEnv::new();
        let ctx = SubstCtx::default();
        assert_eq!(
            expand_with_env("${UNSET:-fallback}", &ctx, &env),
            "fallback"
        );
        assert_eq!(
            expand_with_env("${UNSET-fallback}", &ctx, &env),
            "fallback"
        );

        env.set("EMPTY", "");
        assert_eq!(
            expand_with_env("${EMPTY:-fallback}", &ctx, &env),
            "fallback"
        );
        assert_eq!(expand_with_env("${EMPTY-fallback}", &ctx, &env), ""); // -: set but empty
    }

    #[test]
    fn alternate() {
        let mut env = MockEnv::new();
        env.set("SET", "value");
        let ctx = SubstCtx::default();
        assert_eq!(expand_with_env("${SET:+alt}", &ctx, &env), "alt");
        assert_eq!(expand_with_env("${SET+alt}", &ctx, &env), "alt");

        assert_eq!(expand_with_env("${UNSET:+alt}", &ctx, &env), "");
        assert_eq!(expand_with_env("${UNSET+alt}", &ctx, &env), "");
    }

    #[test]
    fn special_vars() {
        let env = MockEnv::new();
        let mut ctx = SubstCtx::default();
        ctx.argv = vec!["arg1".into(), "arg2".into()];
        ctx.last_exit = 42;
        ctx.last_score = 100;
        ctx.rcfile = "/etc/procmailrc".into();
        ctx.lastfolder = "/var/mail/user".into();

        assert_eq!(expand_with_env("$#", &ctx, &env), "2");
        assert_eq!(expand_with_env("$1", &ctx, &env), "arg1");
        assert_eq!(expand_with_env("$2", &ctx, &env), "arg2");
        assert_eq!(expand_with_env("$3", &ctx, &env), "");
        assert_eq!(expand_with_env("$?", &ctx, &env), "42");
        assert_eq!(expand_with_env("$=", &ctx, &env), "100");
        assert_eq!(expand_with_env("$_", &ctx, &env), "/etc/procmailrc");
        assert_eq!(expand_with_env("$-", &ctx, &env), "/var/mail/user");
        assert!(expand_with_env("$$", &ctx, &env).parse::<u32>().is_ok());
    }

    #[test]
    fn escape() {
        let env = MockEnv::new();
        let ctx = SubstCtx::default();
        assert_eq!(expand_with_env("\\$HOME", &ctx, &env), "$HOME");
        assert_eq!(expand_with_env("\\\\", &ctx, &env), "\\");
    }

    #[test]
    fn nested_default() {
        let mut env = MockEnv::new();
        env.set("INNER", "nested");
        let ctx = SubstCtx::default();
        assert_eq!(expand_with_env("${OUTER:-$INNER}", &ctx, &env), "nested");
    }
}
