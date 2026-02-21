# LASTFOLDER and MATCH have no dedicated gold tests

Severity: medium

`LASTFOLDER` is set by the engine after each successful delivery but is
never read back in a gold test. `MATCH` is populated by `\/` extraction
but the only gold test using `$=` (score) doesn't exercise `\/`.

Missing coverage:
- Deliver to a folder, then use `$LASTFOLDER` in a subsequent recipe
- Use `\/` in a condition, then match `$MATCH` in a subsequent recipe
- Verify `$MATCH` is cleared between recipes (procmail behavior)

## Location

- `src/engine/mod.rs:937,965,1028,1063,1098` (`VAR_LASTFOLDER` assignments)
- `src/engine/mod.rs:701` (`VAR_MATCH` assignment)

## Suggested fix

Add gold tests that read `$LASTFOLDER` and `$MATCH` after they are set,
using variable conditions (`VAR ?? pattern`) or substitution in actions.
