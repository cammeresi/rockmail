# FieldList methods only tested indirectly

Severity: low

Some `FieldList` methods (`remove_all`, `insert_before`, `append`) and
continuation-line handling in `parse_bytes` with folded headers are only
tested through `Message` tests, not directly.

## Location

- `src/field/tests.rs`
- `src/field/mod.rs`

## Suggested fix

Add direct unit tests for `FieldList` methods, especially:

- `remove_all` with multiple matching fields
- `insert_before` ordering
- `append` with continuation lines
- `parse_bytes` with folded (multi-line) headers
