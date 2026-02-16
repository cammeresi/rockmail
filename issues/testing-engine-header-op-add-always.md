# @A (AddAlways) header operation has no test

Severity: medium

The engine has tests for @I (DeleteInsert), @i (RenameInsert),
@a (AddIfNot), and @D (Delete), but @A (AddAlways) has no unit or
integration test.

## Location

- `src/engine/mod.rs:1223-1229` (implementation)
- `src/engine/tests.rs` (missing test)
