# `SHELLFLAGS.def.unwrap()` in trap handler

Severity: low

`run_trap` uses `SHELLFLAGS.def.unwrap()`. The builtin has
`def: Some("-c")` at compile time so this cannot currently fail, but it is
a footgun if builtins are ever reorganized.

## Location

- `src/engine/mod.rs:1454`

## Suggested fix

Use `.unwrap_or("-c")` or `get_or_default`.
