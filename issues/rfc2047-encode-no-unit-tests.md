# rfc2047 encode has no unit tests

Severity: low

The `encode` function is only tested end-to-end via integration tests
(`header_op_rfc2047`, `header_op_not_rfc2047`).

## Location

- `src/rfc2047/tests.rs`
- `src/rfc2047/mod.rs` (`encode`)

## Suggested fix

Add unit tests calling `encode` directly with:

- Values exactly at the line-length limit
- ASCII-only values
- Mixed ASCII and non-ASCII values
- Multi-word headers (multiple encode calls)
