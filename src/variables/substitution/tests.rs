use super::*;

#[test]
fn simple_var() {
    let mut env = MockEnv::new();
    env.set("TEST_VAR", "hello");
    let ctx = SubstCtx::default();
    assert_eq!(subst("$TEST_VAR", &ctx, &env), "hello");
    assert_eq!(subst("${TEST_VAR}", &ctx, &env), "hello");
}

#[test]
fn default_value() {
    let mut env = MockEnv::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst("${UNSET:-fallback}", &ctx, &env), "fallback");
    assert_eq!(subst("${UNSET-fallback}", &ctx, &env), "fallback");

    env.set("EMPTY", "");
    assert_eq!(subst("${EMPTY:-fallback}", &ctx, &env), "fallback");
    assert_eq!(subst("${EMPTY-fallback}", &ctx, &env), ""); // -: set but empty
}

#[test]
fn alternate() {
    let mut env = MockEnv::new();
    env.set("SET", "value");
    let ctx = SubstCtx::default();
    assert_eq!(subst("${SET:+alt}", &ctx, &env), "alt");
    assert_eq!(subst("${SET+alt}", &ctx, &env), "alt");

    assert_eq!(subst("${UNSET:+alt}", &ctx, &env), "");
    assert_eq!(subst("${UNSET+alt}", &ctx, &env), "");
}

#[test]
fn special_vars() {
    let env = MockEnv::new();
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
    let env = MockEnv::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst("\\$HOME", &ctx, &env), "$HOME");
    assert_eq!(subst("\\\\", &ctx, &env), "\\");
}

#[test]
fn nested_default() {
    let mut env = MockEnv::new();
    env.set("INNER", "nested");
    let ctx = SubstCtx::default();
    assert_eq!(subst("${OUTER:-$INNER}", &ctx, &env), "nested");
}

#[test]
fn trailing_dollar() {
    let env = MockEnv::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst("price$", &ctx, &env), "price$");
}

#[test]
fn unknown_char_after_dollar() {
    let env = MockEnv::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst("$@foo", &ctx, &env), "$@foo");
}

#[test]
fn empty_braces() {
    let env = MockEnv::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst("${}", &ctx, &env), "");
}

#[test]
fn dollar_zero() {
    let env = MockEnv::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst("$0", &ctx, &env), "");
}

#[test]
fn other_escapes() {
    let env = MockEnv::new();
    let ctx = SubstCtx::default();
    assert_eq!(subst("\\\"quoted\\\"", &ctx, &env), "\"quoted\"");
    assert_eq!(subst("\\'single\\'", &ctx, &env), "'single'");
    assert_eq!(subst("\\`backtick\\`", &ctx, &env), "`backtick`");
}
