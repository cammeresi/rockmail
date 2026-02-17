# Message should keep headers decomposed

Severity: low

`Message` stores the entire message as a flat byte buffer. Every header
operation requires parsing headers into a `FieldList`, mutating, serializing
back to bytes, and reconstructing the `Message` via `from_parts`. Even with
the batching fix in `apply_header_ops`, any non-adjacent header ops still
pay the full parse/serialize cost.

## Location

- `src/mail/message.rs` (`Message`)
- `src/engine/mod.rs` (`apply_header_ops`, `apply_op_to_fields`)

## Suggested fix

Store a `FieldList` inside `Message` instead of (or alongside) the raw
header bytes. Compose the flat byte representation lazily when
`as_bytes()` or `header()` is called, caching the result and invalidating
on mutation. This eliminates repeated parse/serialize cycles entirely.
