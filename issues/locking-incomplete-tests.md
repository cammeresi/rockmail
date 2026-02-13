# Incomplete locking test coverage

## Component
`src/locking/dotlock/tests.rs`, `src/bin/lockfile.rs`

## Severity
Moderate

## Description

Missing test scenarios:

- Stale lock age (timeout-based force unlock)
- Stale lock size validation
- ENAMETOOLONG handling
- Directory as lockfile target
- Signal interruption during lock acquisition
- NFS-specific errors (ENOSPC, EDQUOT, EIO)
- Repeated lock attempts with transient errors
- fdlock/fcntl locking (not implemented at all)
- No integration tests comparing with procmail lock behavior
