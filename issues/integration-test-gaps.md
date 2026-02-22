# Integration tests missing several scenarios

Severity: low

`tests/rockmail.rs` lacks coverage for several CLI and behavioral paths.

## Location

- `tests/rockmail.rs`

## Suggested fix

Add integration tests for:

- `-p` flag (pass-through: print message to stdout)
- `VERBOSE=off` suppressing output when LOGFILE is set
- Multiple rcfiles on the command line
