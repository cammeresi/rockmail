use super::*;

#[test]
fn simple_var() {
    let mut env = Environment::new();
    env.set("TEST_VAR", "hello");
    let ctx = SubstCtx::default();
    assert_eq!(subst(&env, &ctx, "$TEST_VAR"), "hello");
    assert_eq!(subst(&env, &ctx, "${TEST_VAR}"), "hello");
}

#[test]
fn default_value() {
    let mut env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst(&env, &ctx, "${UNSET:-fallback}"), "fallback");
    assert_eq!(subst(&env, &ctx, "${UNSET-fallback}"), "fallback");

    env.set("EMPTY", "");
    assert_eq!(subst(&env, &ctx, "${EMPTY:-fallback}"), "fallback");
    assert_eq!(subst(&env, &ctx, "${EMPTY-fallback}"), ""); // -: set but empty
}

#[test]
fn alternate() {
    let mut env = Environment::new();
    env.set("SET", "value");
    let ctx = SubstCtx::default();
    assert_eq!(subst(&env, &ctx, "${SET:+alt}"), "alt");
    assert_eq!(subst(&env, &ctx, "${SET+alt}"), "alt");

    assert_eq!(subst(&env, &ctx, "${UNSET:+alt}"), "");
    assert_eq!(subst(&env, &ctx, "${UNSET+alt}"), "");
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

    assert_eq!(subst(&env, &ctx, "$#"), "2");
    assert_eq!(subst(&env, &ctx, "$1"), "arg1");
    assert_eq!(subst(&env, &ctx, "$2"), "arg2");
    assert_eq!(subst(&env, &ctx, "$3"), "");
    assert_eq!(subst(&env, &ctx, "$?"), "42");
    assert_eq!(subst(&env, &ctx, "$="), "100");
    assert_eq!(subst(&env, &ctx, "$_"), "/etc/procmailrc");
    assert_eq!(subst(&env, &ctx, "$-"), "/var/mail/user");
    assert!(subst(&env, &ctx, "$$").parse::<u32>().is_ok());
}

#[test]
fn escape() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst(&env, &ctx, "\\$HOME"), "$HOME");
    assert_eq!(subst(&env, &ctx, "\\\\"), "\\");
}

#[test]
fn nested_default() {
    let mut env = Environment::new();
    env.set("INNER", "nested");
    let ctx = SubstCtx::default();
    assert_eq!(subst(&env, &ctx, "${OUTER:-$INNER}"), "nested");
}

#[test]
fn trailing_dollar() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst(&env, &ctx, "price$"), "price$");
}

#[test]
fn unknown_char_after_dollar() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst(&env, &ctx, "$@foo"), "$@foo");
}

#[test]
fn empty_braces() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst(&env, &ctx, "${}"), "");
}

#[test]
fn dollar_zero() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst(&env, &ctx, "$0"), "");
}

#[test]
fn other_escapes() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst(&env, &ctx, "\\\"quoted\\\""), "\"quoted\"");
    assert_eq!(subst(&env, &ctx, "\\'single\\'"), "'single'");
    assert_eq!(subst(&env, &ctx, "\\`backtick\\`"), "`backtick`");
}

#[test]
fn non_recursive_expansion() {
    // $A where A="B" should produce "B", not the value of $B
    let mut env = Environment::new();
    env.set("A", "B");
    env.set("B", "deep");
    let ctx = SubstCtx::default();
    assert_eq!(subst(&env, &ctx, "$A"), "B");
    assert_eq!(subst(&env, &ctx, "${A}"), "B");
}

#[test]
fn whitespace_in_braces() {
    // Spaces before variable name → empty name → empty result
    let mut env = Environment::new();
    env.set("VAR", "val");
    let ctx = SubstCtx::default();
    assert_eq!(subst(&env, &ctx, "${ VAR }"), "");
    assert_eq!(subst(&env, &ctx, "${ VAR}"), "");
    // Space after operator is part of default text
    assert_eq!(subst(&env, &ctx, "${VAR:- default}"), "val");
    assert_eq!(subst(&env, &ctx, "${UNSET:- default}"), " default");
}

#[test]
fn escape_in_default() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst(&env, &ctx, "${X:-\\$literal}"), "$literal");
    assert_eq!(subst(&env, &ctx, "${X:-\\\\backslash}"), "\\backslash");
}

#[test]
fn empty_vs_unset() {
    let mut env = Environment::new();
    env.set("EMPTY", "");
    let ctx = SubstCtx::default();

    // :- treats empty same as unset
    assert_eq!(subst(&env, &ctx, "${EMPTY:-fb}"), "fb");
    assert_eq!(subst(&env, &ctx, "${UNSET:-fb}"), "fb");

    // - treats empty as set
    assert_eq!(subst(&env, &ctx, "${EMPTY-fb}"), "");
    assert_eq!(subst(&env, &ctx, "${UNSET-fb}"), "fb");

    // :+ treats empty as unset
    assert_eq!(subst(&env, &ctx, "${EMPTY:+alt}"), "");
    assert_eq!(subst(&env, &ctx, "${UNSET:+alt}"), "");

    // + treats empty as set
    assert_eq!(subst(&env, &ctx, "${EMPTY+alt}"), "alt");
    assert_eq!(subst(&env, &ctx, "${UNSET+alt}"), "");
}

#[test]
fn long_name_and_value() {
    let mut env = Environment::new();
    let name = "A".repeat(1000);
    let val = "x".repeat(10000);
    env.set(name.clone(), val.clone());
    let ctx = SubstCtx::default();
    assert_eq!(subst(&env, &ctx, &format!("${name}")), val);
    assert_eq!(subst(&env, &ctx, &format!("${{{name}}}")), val);
}

#[test]
fn multiple_expansions() {
    let mut env = Environment::new();
    env.set("A", "1");
    env.set("B", "2");
    let ctx = SubstCtx::default();
    assert_eq!(subst(&env, &ctx, "$A+$B=$A$B"), "1+2=12");
}

#[test]
fn nested_braces_in_default() {
    let mut env = Environment::new();
    env.set("INNER", "yes");
    let ctx = SubstCtx::default();
    assert_eq!(subst(&env, &ctx, "${X:-${INNER}}"), "yes");
    assert_eq!(subst(&env, &ctx, "${X:-${ALSO_UNSET:-deep}}"), "deep");
}

#[test]
fn unescaped_backslash() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    // Backslash before non-special char is kept literally
    assert_eq!(subst(&env, &ctx, "\\n"), "\\n");
    assert_eq!(subst(&env, &ctx, "\\a\\b"), "\\a\\b");
}

#[test]
fn regex_escape() {
    let mut env = Environment::new();
    let ctx = SubstCtx::default();

    env.set("V", "user.name+tag");
    assert_eq!(subst(&env, &ctx, "$\\V"), "(u)ser\\.name\\+tag");

    env.set("V", "plain");
    assert_eq!(subst(&env, &ctx, "$\\V"), "(p)lain");

    env.set("V", "");
    assert_eq!(subst(&env, &ctx, "$\\V"), "");

    assert_eq!(subst(&env, &ctx, "$\\UNSET"), "");

    env.set("V", "^foo$");
    assert_eq!(subst(&env, &ctx, "$\\V"), "(\\^)foo\\$");
}

#[test]
fn regex_escape_literal_fallback() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    // $\ not followed by a name start char → literal "$\"
    assert_eq!(subst(&env, &ctx, "$\\9"), "$\\9");
    assert_eq!(subst(&env, &ctx, "$\\"), "$\\");
}

#[test]
fn overflow() {
    let mut env = Environment::new();
    env.set("V", "abcdefghij");
    let ctx = SubstCtx::default();

    let (r, over) = subst_limited(&env, &ctx, "$V$V", 21);
    assert_eq!(r, "abcdefghijabcdefghij");
    assert!(!over);

    let (r, over) = subst_limited(&env, &ctx, "$V$V", 15);
    assert_eq!(r, "abcdefghijabcde");
    assert!(over);

    // No expansion, under limit
    let (r, over) = subst_limited(&env, &ctx, "hello", 10);
    assert_eq!(r, "hello");
    assert!(!over);
}
