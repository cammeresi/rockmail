# Engine side-effect paths lack unit tests

Severity: medium

`apply_side_effect` handles several variables with side effects that are
only tested via gold tests (which require procmail installed).

## Location

- `src/engine/mod.rs` (`apply_side_effect`)

## Suggested fix

Add engine unit tests for:

- `VAR_MAILDIR` chdir failure path
- `VAR_LINEBUF` enforcement of `MIN_LINEBUF` (value below minimum)
- `VAR_HOST` mismatch setting `self.abort = true`
- `VAR_SHIFT` clamping when `n > argv.len()`
