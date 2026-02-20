---
name: testing
description: How to run tests in the rockmail project.  Use when running or writing tests.
---

# Testing

## Running tests

- `cargo test` — run all tests including gold tests (gold is a default feature)
- `cargo test --no-default-features` — run tests without gold tests
- `cargo test --test formail_gold` — run formail gold tests only
- `cargo test --test rockmail_gold` — run rockmail gold tests only
- `cargo test PATTERN` — run tests matching a name pattern

## Gold tests

Gold tests compare rockmail output against procmail output. They require
procmail to be installed. The gold binaries are found automatically; override
with `PROCMAIL_FORMAIL` or `PROCMAIL_ROCKMAIL` environment variables.

Gold test files:
- `tests/formail_gold.rs`
- `tests/rockmail_gold.rs`

Helpers are in `tests/common/mod.rs`. Key functions:
- `run_once()` — run both binaries once and compare
- `Gold::new()` / `Gold::run()` — stateful runner for multi-message tests
- `normalize_from_line` — normalize timestamps in From_ lines
- `normalize_message_id` — normalize generated Message-ID headers

Use `assert_eq()` for exact match or `assert_eq_with(normalizer)` when
output contains timestamps or generated IDs.

## Clippy

Run clippy with all targets and features to catch warnings in test code
and gold tests:

```
cargo clippy --all-targets --all-features
```

## Test conventions

- Test names should not be prefixed with "test_".
- Messages must have a blank line between headers and body.
- Prefer `assert_eq!` over `assert!(matches!(`. Using `assert_eq!` exercises
  `PartialEq` and `Debug` impls, which improves code coverage statistics,
  whereas `assert!(matches!(` contains hidden regions that will never be
  tested.  When the exact value is unpredictable (e.g. maildir paths), use an
  extraction helper that panics on variant mismatch, with a corresponding
  `#[should_panic]` test.
