# Macro expansion bypasses pattern length limit

Severity: medium

`Matcher::new` checks `pattern.len() > MAX_PATTERN_LEN` (4096) before
calling `expand_macros`. Macros like `^FROM_DAEMON` expand to ~300 bytes
each, so a pattern near the limit stuffed with macro keys could expand
well beyond 4096 bytes.

The `regex` crate has its own internal size limits and will reject
extremely large patterns, so this is not directly exploitable. But the
intent of `MAX_PATTERN_LEN` is defeated.

## Location

- `src/re/matcher.rs:260-262` (length check)
- `src/re/matcher.rs:76-92` (`expand_macros`)

## Suggested fix

Check length after expansion, or cap the expanded result.
