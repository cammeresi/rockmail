# Header ops have incomplete integration coverage

Severity: medium

Engine unit tests exist for all four ops (7 tests at
`src/engine/tests.rs:768`), but only `@A` has an integration test
(`tests/rockmail.rs:616`).

Missing integration tests:
- `@I` (delete all matching, then insert)
- `@i` (rename existing to Old-Header, insert new)
- `@a` (add only if header not present)
- Variable expansion in header values
- RFC 2047 encoding of non-ASCII values
