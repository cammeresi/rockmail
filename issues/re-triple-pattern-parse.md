# Pattern parsed three times during Matcher construction

Severity: low

`Matcher::new` processes the pattern three times:

1. `expand_macros` — iterates chars to find/replace macro keys
2. `compile` — iterates chars to translate procmail regex to Rust regex
3. `compiled_capture_group` — iterates chars to find the `\/` group index

These could be combined into a single pass.

## Location

- `src/re/matcher.rs:264-274`
