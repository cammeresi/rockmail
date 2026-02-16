# Header operations parse and rebuild entire message each time

Severity: medium

Each `apply_header_op` call parses all headers into a `FieldList`,
mutates it, serializes back to bytes, and reconstructs the `Message`.
When a recipe has N header directives, the message is fully rebuilt N
times.

For a 10KB message with 5 header operations, this is ~50KB of
unnecessary parsing and copying.

## Location

- `src/engine/mod.rs:1191-1237` (`apply_header_op`)

## Suggested fix

Batch header operations: collect all directives for a recipe, apply
them to a single `FieldList`, then rebuild the message once.
