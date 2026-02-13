# No tests for variable expansion in action lines

## Component
`src/config/action.rs`, `src/engine/mod.rs`

## Severity
Moderate

## Description

Action lines are expanded at execution time in the engine, but
`src/config/action/tests.rs` only tests parsing, not expansion.

Missing test coverage:

- `${MAILDIR}/important` expands correctly at delivery time
- Pipe commands with variable arguments
- Forward addresses containing variables
- Side effects from pipe capture assignments (e.g. `MAILDIR=| cmd`
  correctly triggers directory change)
