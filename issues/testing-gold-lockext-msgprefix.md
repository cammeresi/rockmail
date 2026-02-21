# LOCKEXT and MSGPREFIX have no gold tests

Severity: low

`LOCKEXT` controls the suffix for auto-generated lockfiles (default
`.lock`). `MSGPREFIX` controls the filename prefix for MH delivery
(default `msg.`). Neither is tested against procmail.

Missing coverage:
- Change `LOCKEXT`, verify lockfile suffix changes
- Change `MSGPREFIX`, verify MH filenames use new prefix

## Location

- `src/variables/builtins.rs:78` (`LOCKEXT` default)
- `src/variables/builtins.rs:79` (`MSGPREFIX` default)

## Suggested fix

Add gold tests that reassign each variable and verify the resulting
filenames match procmail.
