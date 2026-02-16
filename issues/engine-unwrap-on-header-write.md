# `unwrap()` on infallible `Vec` write in header operations

Severity: low

`apply_header_op` calls `fields.write_to(&mut header).unwrap()` where
`header` is a `Vec<u8>`. Writing to a `Vec` cannot fail (infallible I/O),
so this is safe in practice but should use `.expect()` with a message for
clarity, or propagate the error.

## Location

- `src/engine/mod.rs:1236`
