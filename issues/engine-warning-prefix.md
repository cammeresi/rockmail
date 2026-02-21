# Engine warnings missing program name prefix

Severity: low

Procmail's `nlog()` prefixes all diagnostic messages with `"procmail: "`.
The Rust engine uses plain `eprintln!()` without a prefix.

A `warning!` macro was added for the `"Exceeded LINEBUF"` message, but
most other engine `eprintln!()` calls that correspond to procmail `nlog()`
calls still lack the prefix.

## Examples

- `"Failed forking"` — procmail: `misc.c:116`
- `"can't chdir to"` — procmail: `misc.c:130`
- `"Invalid regexp"` — procmail: `regexp.c:373`
- `"Skipped"` — procmail: `misc.c:212`
- `"Couldn't make link to"` — procmail: `mailfold.c:181`
- `"Program failure"` — procmail: `misc.c:124`
- `"Deadlock attempted on"` — procmail: `locking.c:122`

## Suggested fix

Convert applicable `eprintln!()` calls in `src/engine/mod.rs` to use the
`warning!` macro.  Messages that correspond to procmail's `elog()` (no
prefix) — such as the delivery abstract lines — should remain as
`eprintln!()`.
