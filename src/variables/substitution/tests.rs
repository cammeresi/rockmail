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

    let (r, over) = subst_limited(&env, &ctx, "$V$V", 21, None);
    assert_eq!(r, "abcdefghijabcdefghij");
    assert!(!over);

    let (r, over) = subst_limited(&env, &ctx, "$V$V", 15, None);
    assert_eq!(r, "abcdefghijabcde");
    assert!(over);

    // No expansion, under limit
    let (r, over) = subst_limited(&env, &ctx, "hello", 10, None);
    assert_eq!(r, "hello");
    assert!(!over);
}

#[test]
fn backtick_with_runner() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    let run = |cmd: &str| cmd.to_uppercase();
    let r = subst_limited(&env, &ctx, "`hello`", usize::MAX, Some(&run));
    assert_eq!(r.0, "HELLO");
}

#[test]
fn backtick_var_inside() {
    let mut env = Environment::new();
    env.set("X", "world");
    let ctx = SubstCtx::default();
    let run = |cmd: &str| cmd.to_owned();
    let r = subst_limited(&env, &ctx, "`echo $X`", usize::MAX, Some(&run));
    assert_eq!(r.0, "echo world");
}

fn nope(_: &str) -> String {
    panic!("not called");
}

#[test]
#[should_panic(expected = "not called")]
fn nope_panics() {
    nope("a");
}

#[test]
fn backtick_escaped() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    // \` should produce literal backtick, not invoke runner
    let r = subst_limited(&env, &ctx, "\\`lit\\`", usize::MAX, Some(&nope));
    assert_eq!(r.0, "`lit`");
}

#[test]
fn backtick_unclosed() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    let run = |cmd: &str| cmd.to_owned();
    let r = subst_limited(&env, &ctx, "`unclosed", usize::MAX, Some(&run));
    assert_eq!(r.0, "unclosed");
}

#[test]
fn backtick_none_runner() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    // Without runner, backticks are literal
    assert_eq!(subst(&env, &ctx, "`hello`"), "`hello`");
}

#[test]
fn backtick_empty() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    let run = |cmd: &str| cmd.to_owned();
    let r = subst_limited(&env, &ctx, "``", usize::MAX, Some(&run));
    assert_eq!(r.0, "");
}

#[test]
fn backtick_in_default() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    let run = |_: &str| "val".to_owned();
    let r = subst_limited(&env, &ctx, "${X:-`cmd`}", usize::MAX, Some(&run));
    assert_eq!(r.0, "val");
}

fn sq(env: &Environment, ctx: &SubstCtx, s: &str) -> String {
    subst_quoted(env, ctx, s, usize::MAX, None).0
}

#[test]
fn double_quotes_stripped() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(sq(&env, &ctx, "\"hello\""), "hello");
}

#[test]
fn single_quotes_stripped() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(sq(&env, &ctx, "'hello'"), "hello");
}

#[test]
fn single_inside_double() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(sq(&env, &ctx, "\"it's\""), "it's");
}

#[test]
fn double_inside_single() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(sq(&env, &ctx, "'she said \"hi\"'"), "she said \"hi\"");
}

#[test]
fn expansion_in_double_quotes() {
    let mut env = Environment::new();
    env.set("VAR", "world");
    let ctx = SubstCtx::default();
    assert_eq!(sq(&env, &ctx, "\"$VAR\""), "world");
}

#[test]
fn no_expansion_in_single_quotes() {
    let mut env = Environment::new();
    env.set("VAR", "world");
    let ctx = SubstCtx::default();
    assert_eq!(sq(&env, &ctx, "'$VAR'"), "$VAR");
}

#[test]
fn escaped_quotes_not_delimiters() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    assert_eq!(sq(&env, &ctx, "\\\"literal\\\""), "\"literal\"");
}

#[test]
fn quote_stripping_with_var() {
    let mut env = Environment::new();
    env.set("NEWSUBJECT", "Thanksgiving Feast");
    let ctx = SubstCtx::default();
    assert_eq!(
        sq(&env, &ctx, "\"Re: $NEWSUBJECT\""),
        "Re: Thanksgiving Feast"
    );
}

#[test]
fn backtick_literal_in_single_quotes() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    let r = subst_quoted(&env, &ctx, "'`cmd`'", usize::MAX, Some(&nope));
    assert_eq!(r.0, "`cmd`");
}

#[test]
fn quotes_preserved_by_default() {
    let mut env = Environment::new();
    env.set("VAR", "world");
    let ctx = SubstCtx::default();
    assert_eq!(subst(&env, &ctx, "\"$VAR\""), "\"world\"");
    assert_eq!(subst(&env, &ctx, "'$VAR'"), "'world'");
}

fn esc(s: &str) -> String {
    let mut out = String::new();
    regex_escape_into(s, &mut out);
    out
}

#[test]
fn regex_escape_into_empty() {
    assert_eq!(esc(""), "");
}

#[test]
fn regex_escape_into_single_plain() {
    assert_eq!(esc("a"), "(a)");
}

#[test]
fn regex_escape_into_single_meta() {
    assert_eq!(esc("."), "(\\.)");
    assert_eq!(esc("^"), "(\\^)");
    assert_eq!(esc("$"), "(\\$)");
    assert_eq!(esc("*"), "(\\*)");
    assert_eq!(esc("?"), "(\\?)");
    assert_eq!(esc("+"), "(\\+)");
    assert_eq!(esc("("), "(\\()");
    assert_eq!(esc(")"), "(\\))");
    assert_eq!(esc("|"), "(\\|)");
    assert_eq!(esc("["), "(\\[)");
    assert_eq!(esc("\\"), "(\\\\)");
}

#[test]
fn regex_escape_into_plain() {
    assert_eq!(esc("abc"), "(a)bc");
    assert_eq!(esc("hello world"), "(h)ello world");
}

#[test]
fn regex_escape_into_meta_inside() {
    assert_eq!(esc("a.b"), "(a)\\.b");
    assert_eq!(esc("a(b|c)"), "(a)\\(b\\|c\\)");
}

#[test]
fn regex_escape_into_all_meta() {
    assert_eq!(esc(".*"), "(\\.)\\*");
    assert_eq!(esc("^$"), "(\\^)\\$");
}

#[test]
fn regex_escape_into_leading_meta() {
    assert_eq!(esc("^foo$"), "(\\^)foo\\$");
    assert_eq!(esc("[abc]"), "(\\[)abc]");
}

#[test]
fn regex_escape_into_non_meta_special() {
    // ] { } are NOT in RE_META, should pass through unescaped
    assert_eq!(esc("a]b"), "(a)]b");
    assert_eq!(esc("a{b}"), "(a){b}");
}

#[test]
fn depth_limit() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    // Build 40 levels: ${X:-${X:-${X:-...leaf...}}}
    let mut s = "leaf".to_owned();
    for _ in 0..40 {
        s = format!("${{X:-{s}}}");
    }
    let r = subst(&env, &ctx, &s);
    assert!(r.contains("${X:-"), "expected unexpanded syntax");
    assert!(r.contains("leaf"));
}

fn ctb(s: &str) -> String {
    let mut chars = s.chars().peekable();
    collect_to_brace(&mut chars)
}

#[test]
fn collect_to_brace_simple() {
    assert_eq!(ctb("hello}"), "hello");
}

#[test]
fn collect_to_brace_empty() {
    assert_eq!(ctb("}"), "");
}

#[test]
fn collect_to_brace_nested() {
    assert_eq!(ctb("a{b}c}"), "a{b}c");
    assert_eq!(ctb("{{}}}"), "{{}}");
}

#[test]
fn collect_to_brace_deep() {
    assert_eq!(ctb("a{b{c}d}e}"), "a{b{c}d}e");
}

#[test]
fn collect_to_brace_no_closing() {
    // Iterator exhausted without closing brace — collects everything
    assert_eq!(ctb("abc"), "abc");
    assert_eq!(ctb("a{b"), "a{b");
}

fn stb(s: &str) -> String {
    let mut chars = s.chars().peekable();
    skip_to_brace(&mut chars);
    chars.collect()
}

#[test]
fn skip_to_brace_simple() {
    assert_eq!(stb("hello}rest"), "rest");
}

#[test]
fn skip_to_brace_empty() {
    assert_eq!(stb("}rest"), "rest");
}

#[test]
fn skip_to_brace_nested() {
    assert_eq!(stb("a{b}c}rest"), "rest");
}

#[test]
fn skip_to_brace_deep() {
    assert_eq!(stb("a{b{c}d}e}rest"), "rest");
}

#[test]
fn skip_to_brace_no_closing() {
    assert_eq!(stb("abc"), "");
    assert_eq!(stb("a{b"), "");
}

#[test]
fn braced_invalid_operator_after_colon() {
    let mut env = Environment::new();
    env.set("V", "val");
    let ctx = SubstCtx::default();
    // :! is not a valid operator — should skip to closing brace, produce nothing
    assert_eq!(subst(&env, &ctx, "${V:!foo}"), "");
    assert_eq!(subst(&env, &ctx, "${V:}"), "");
}

#[test]
fn braced_invalid_operator() {
    let mut env = Environment::new();
    env.set("V", "val");
    let ctx = SubstCtx::default();
    // ! is not a valid operator — should skip to closing brace, produce nothing
    assert_eq!(subst(&env, &ctx, "${V!foo}"), "");
    assert_eq!(subst(&env, &ctx, "${V=foo}"), "");
}

#[test]
fn backtick_escape_backslash_and_dollar() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    let run = |cmd: &str| cmd.to_owned();
    // \\ inside backticks → literal backslash
    let r = subst_limited(&env, &ctx, "`a\\\\b`", usize::MAX, Some(&run));
    assert_eq!(r.0, "a\\b");
    // \$ inside backticks → literal dollar
    let r = subst_limited(&env, &ctx, "`a\\$b`", usize::MAX, Some(&run));
    assert_eq!(r.0, "a$b");
}

#[test]
fn multiple_backticks() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    let run = |cmd: &str| cmd.to_uppercase();
    let r = subst_limited(&env, &ctx, "`aa`+`bb`", usize::MAX, Some(&run));
    assert_eq!(r.0, "AA+BB");
}

#[test]
fn regex_escape_in_default() {
    let mut env = Environment::new();
    env.set("V", "a.b");
    let ctx = SubstCtx::default();
    assert_eq!(subst(&env, &ctx, "${X:-$\\V}"), "(a)\\.b");
}

#[test]
fn overflow_during_backtick() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    let run = |_: &str| "abcdefghij".to_owned();
    let (r, over) = subst_limited(&env, &ctx, "xx`cmd`", 5, Some(&run));
    assert_eq!(r, "xxabc");
    assert!(over);
}

#[test]
fn overflow_during_literal() {
    let env = Environment::new();
    let ctx = SubstCtx::default();
    let (r, over) = subst_limited(&env, &ctx, "abcdefghij", 5, None);
    assert_eq!(r, "abcde");
    assert!(over);
}
