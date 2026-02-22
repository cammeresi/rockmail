# Maildir unique-file collision/retry logic untested

Severity: low

The `UniqueFile` error path and retry loop for unique filename generation
are not directly exercised by tests.

## Location

- `src/delivery/maildir/tests.rs`
- `src/delivery/maildir/mod.rs`

## Suggested fix

Add tests for:

- Unique filename conflict (file already exists at computed path)
- The retry loop in unique filename generation
- `link_unique` fallback behavior
