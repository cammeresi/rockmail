# No tests for flock locking

## Component
`src/locking/flock.rs`

## Severity
Moderate

## Description

`FileLock` has no unit tests. Testing is tricky because `flock` is
per-open-file-description — the same process can acquire a second flock
on the same file via a different fd without conflict. Meaningful
contention tests require a child process holding the lock.

Tests needed:

- `acquire_temp`: lock acquired, file removed on drop
- `acquire_temp_retry`: retry succeeds when lock is released by another process
- `acquire_temp_retry`: times out when lock is held past `LOCKTIMEOUT`
- `acquire_temp_retry`: stale lock removal (file older than timeout is removed)
