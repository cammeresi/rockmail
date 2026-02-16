# Macro expansion uses O(n^2) string replacement

Severity: medium

`expand_macros` uses a `while find / replace_range` loop. Each `find`
is O(n) and each `replace_range` shifts the tail of the string, also
O(n). For a pattern with many macro keys this is quadratic in the
pattern length.

In practice patterns are small (<4KB) and macros are rare, so this is
unlikely to matter, but the algorithm is unnecessarily expensive.

## Location

- `src/re/matcher.rs:76-92`

## Suggested fix

Build the result in a single forward pass, replacing macros as they
are encountered.
