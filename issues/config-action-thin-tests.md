# Action parser tests are thin

Severity: medium

Unit tests cover folder, pipe, capture-pipe, and forward parsing but skip
several action types.

## Location

- `src/config/action/tests.rs`

## Suggested fix

Add unit tests for:

- `@D` dedup action
- `@A`, `@a`, `@I`, `@i` header-op actions
- Nested block action (`{` ... `}`)
- Multi-target folder lines (secondary folders)
