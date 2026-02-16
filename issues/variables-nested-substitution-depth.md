# Nested `${var:-...}` substitution has no depth limit

Severity: low

`expand_braced` recurses through `subst_with` for default values.
Deeply nested defaults like `${A:-${B:-${C:-...}}}` in an rcfile could
cause a stack overflow. The `LINEBUF` limit bounds output size but not
recursion depth.

Since rcfiles are trusted input this is low severity.

## Location

- `src/variables/substitution.rs:174` (recursive call in `expand_braced`)
