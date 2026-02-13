# No tests for $= (last score) variable

## Component
`src/engine/mod.rs`, `src/engine/tests.rs`

## Severity
Moderate

## Description

The engine sets `ctx.last_score` after condition evaluation, but no
tests verify:

- `$=` contains the correct score after a weighted recipe
- `$=` is accessible via variable expansion
- `$=` reflects the score from the most recent recipe, not a stale
  value
- Score is correct after multiple recipes with weighted conditions

The test infrastructure (`Test` struct in `tests.rs`) doesn't expose
`engine.ctx.last_score`, making it difficult to verify.
