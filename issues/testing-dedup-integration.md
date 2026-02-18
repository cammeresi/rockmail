# `@D` dedup has no integration test

Severity: medium

3 engine unit tests exist (`dupecheck_new_message`, `dupecheck_duplicate`,
`dupecheck_no_msgid`) and 7 cache data structure tests exist in
`src/dedup/tests.rs`, but no test runs the actual rockmail binary with
`@D` in an rcfile.

Missing coverage:
- Integration test with actual binary
- Cache overflow behavior
- Concurrent access
- Corrupt cache file
