# Dump tests are trivial

Severity: medium

`src/config/dump/tests.rs` has a single test that checks item count on a
small rcfile.  No error paths, complex items, or line-number correctness
are verified.

## Location

- `src/config/dump/tests.rs`

## Suggested fix

Add tests for:

- Parse error input
- Nested blocks, weighted conditions, subst items
- `@`-header ops, `@D` dedup
- `INCLUDERC`/`SWITCHRC` items in dump output
- Correct item line numbers
