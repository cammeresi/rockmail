# `eval_variable` forces `into_owned()` on Cow

Severity: medium

`eval_variable` calls `get_variable_text().into_owned()`, converting a
`Cow<str>` to an owned `String` unconditionally. When the Cow is
borrowed (the common case for regular variables), this forces a copy
of the variable value.

## Location

- `src/engine/mod.rs:679`

## Suggested fix

Pass `&str` via `Cow::as_ref()` instead of forcing ownership.
