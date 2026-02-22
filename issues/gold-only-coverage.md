# Important behaviors only tested via gold tests

Severity: medium

Several code paths are only verified by gold tests gated on
`feature = "gold"`, making them invisible to `cargo test` by default.
If procmail is not installed, these paths have no coverage at all.

## Location

- `tests/rockmail_gold.rs`
- `src/engine/mod.rs`

## Suggested fix

Add non-gold integration tests or engine unit tests for:

- HOST mismatch abort
- DELIVERED variable interaction with default delivery
- ORGMAIL fallback when DEFAULT is inaccessible
- Raw (`r`) flag in mbox delivery
