# run_backtick silently ignores errors

Severity: low

`run_backtick` (src/engine/mod.rs:211) discards errors in four places:

1. **Spawn failure** (line 224-225): returns empty string with no diagnostic.
2. **stdin write** (line 228): `let _ = w.write_all(input)` — broken-pipe
   is expected, but other errors are silently lost.
3. **stdout read** (line 232): `let _ = read_to_end(...)` — I/O errors
   silently produce a truncated or empty result.
4. **wait_timeout** (line 234): `let _ = wait_timeout(...)` — timeout
   or signal errors are discarded.

Procmail's C code (`backtstrstrip()` in `goodies.c`) also ignores most of
these errors, so this may be intentional for compatibility.  At minimum,
spawn failures should produce an `eprintln!` diagnostic.

## Location

- `src/engine/mod.rs:211-239`
