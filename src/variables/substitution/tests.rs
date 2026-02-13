use super::*;

#[test]
fn simple_var() {
    let mut env = Environment::new();
    env.set("TEST_VAR", "hello");
    let ctx = SubstCtx::default();
    assert_eq!(subst("$TEST_VAR", &ctx, &env), "hello");
    assert_eq!(subst("${TEST_VAR}", &ctx, &env), "hello");
}

#[test]
fn default_value() {
    let mut env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst("${UNSET:-fallback}", &ctx, &env), "fallback");
    assert_eq!(subst("${UNSET-fallback}", &ctx, &env), "fallback");

    env.set("EMPTY", "");
    assert_eq!(subst("${EMPTY:-fallback}", &ctx, &env), "fallback");
    assert_eq!(subst("${EMPTY-fallback}", &ctx, &env), ""); // -: set but empty
}

#[test]
fn alternate() {
    let mut env = Environment::new();
    env.set("SET", "value");
    let ctx = SubstCtx::default();
    assert_eq!(subst("${SET:+alt}", &ctx, &env), "alt");
    assert_eq!(subst("${SET+alt}", &ctx, &env), "alt");

    assert_eq!(subst("${UNSET:+alt}", &ctx, &env), "");
    assert_eq!(subst("${UNSET+alt}", &ctx, &env), "");
}

#[test]
fn special_vars() {
    let env = Environment::new();
    let ctx = SubstCtx {
        argv: vec!["arg1".into(), "arg2".into()],
        last_exit: 42,
        last_score: 100,
        rcfile: "/etc/procmailrc".into(),
        lastfolder: "/var/mail/user".into(),
        ..Default::default()
    };

    assert_eq!(subst("$#", &ctx, &env), "2");
    assert_eq!(subst("$1", &ctx, &env), "arg1");
    assert_eq!(subst("$2", &ctx, &env), "arg2");
    assert_eq!(subst("$3", &ctx, &env), "");
    assert_eq!(subst("$?", &ctx, &env), "42");
    assert_eq!(subst("$=", &ctx, &env), "100");
    assert_eq!(subst("$_", &ctx, &env), "/etc/procmailrc");
    assert_eq!(subst("$-", &ctx, &env), "/var/mail/user");
    assert!(subst("$$", &ctx, &env).parse::<u32>().is_ok());
}

#[test]
fn escape() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst("\\$HOME", &ctx, &env), "$HOME");
    assert_eq!(subst("\\\\", &ctx, &env), "\\");
}

#[test]
fn nested_default() {
    let mut env = Environment::new();
    env.set("INNER", "nested");
    let ctx = SubstCtx::default();
    assert_eq!(subst("${OUTER:-$INNER}", &ctx, &env), "nested");
}

#[test]
fn trailing_dollar() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst("price$", &ctx, &env), "price$");
}

#[test]
fn unknown_char_after_dollar() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst("$@foo", &ctx, &env), "$@foo");
}

#[test]
fn empty_braces() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst("${}", &ctx, &env), "");
}

#[test]
fn dollar_zero() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst("$0", &ctx, &env), "");
}

#[test]
fn other_escapes() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst("\\\"quoted\\\"", &ctx, &env), "\"quoted\"");
    assert_eq!(subst("\\'single\\'", &ctx, &env), "'single'");
    assert_eq!(subst("\\`backtick\\`", &ctx, &env), "`backtick`");
}

#[test]
fn non_recursive_expansion() {
    // $A where A="B" should produce "B", not the value of $B
    let mut env = Environment::new();
    env.set("A", "B");
    env.set("B", "deep");
    let ctx = SubstCtx::default();
    assert_eq!(subst("$A", &ctx, &env), "B");
    assert_eq!(subst("${A}", &ctx, &env), "B");
}

#[test]
fn whitespace_in_braces() {
    // Spaces before variable name → empty name → empty result
    let mut env = Environment::new();
    env.set("VAR", "val");
    let ctx = SubstCtx::default();
    assert_eq!(subst("${ VAR }", &ctx, &env), "");
    assert_eq!(subst("${ VAR}", &ctx, &env), "");
    // Space after operator is part of default text
    assert_eq!(subst("${VAR:- default}", &ctx, &env), "val");
    assert_eq!(subst("${UNSET:- default}", &ctx, &env), " default");
}

#[test]
fn escape_in_default() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst("${X:-\\$literal}", &ctx, &env), "$literal");
    assert_eq!(subst("${X:-\\\\backslash}", &ctx, &env), "\\backslash");
}

#[test]
fn empty_vs_unset() {
    let mut env = Environment::new();
    env.set("EMPTY", "");
    let ctx = SubstCtx::default();

    // :- treats empty same as unset
    assert_eq!(subst("${EMPTY:-fb}", &ctx, &env), "fb");
    assert_eq!(subst("${UNSET:-fb}", &ctx, &env), "fb");

    // - treats empty as set
    assert_eq!(subst("${EMPTY-fb}", &ctx, &env), "");
    assert_eq!(subst("${UNSET-fb}", &ctx, &env), "fb");

    // :+ treats empty as unset
    assert_eq!(subst("${EMPTY:+alt}", &ctx, &env), "");
    assert_eq!(subst("${UNSET:+alt}", &ctx, &env), "");

    // + treats empty as set
    assert_eq!(subst("${EMPTY+alt}", &ctx, &env), "alt");
    assert_eq!(subst("${UNSET+alt}", &ctx, &env), "");
}

#[test]
fn long_name_and_value() {
    let mut env = Environment::new();
    let name = "A".repeat(1000);
    let val = "x".repeat(10000);
    env.set(name.clone(), val.clone());
    let ctx = SubstCtx::default();
    assert_eq!(subst(&format!("${name}"), &ctx, &env), val);
    assert_eq!(subst(&format!("${{{name}}}"), &ctx, &env), val);
}

#[test]
fn multiple_expansions() {
    let mut env = Environment::new();
    env.set("A", "1");
    env.set("B", "2");
    let ctx = SubstCtx::default();
    assert_eq!(subst("$A+$B=$A$B", &ctx, &env), "1+2=12");
}

#[test]
fn nested_braces_in_default() {
    let mut env = Environment::new();
    env.set("INNER", "yes");
    let ctx = SubstCtx::default();
    assert_eq!(subst("${X:-${INNER}}", &ctx, &env), "yes");
    assert_eq!(subst("${X:-${ALSO_UNSET:-deep}}", &ctx, &env), "deep");
}

#[test]
fn unescaped_backslash() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    // Backslash before non-special char is kept literally
    assert_eq!(subst("\\n", &ctx, &env), "\\n");
    assert_eq!(subst("\\a\\b", &ctx, &env), "\\a\\b");
}
