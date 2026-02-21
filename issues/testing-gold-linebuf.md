# LINEBUF has no gold test

Severity: low

Assigning `LINEBUF` changes the internal line buffer size, with a
minimum of 128. The clamping and `PROCMAIL_OVERFLOW` behavior are
untested against procmail.

Missing coverage:
- Set `LINEBUF` below minimum, verify it clamps to 128
- Trigger overflow with a small `LINEBUF`, verify `PROCMAIL_OVERFLOW=yes`

## Location

- `src/engine/mod.rs:390` (`set_var` match arm for `VAR_LINEBUF`)
- `src/engine/mod.rs:477` (`PROCMAIL_OVERFLOW` assignment)

## Suggested fix

Add a gold test that sets a small `LINEBUF`, sends a message with a
long line, and verifies overflow behavior matches procmail.
