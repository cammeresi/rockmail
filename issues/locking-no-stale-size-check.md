# No stale lock size validation

## Component
`src/locking/dotlock.rs`, `src/bin/lockfile.rs`

## Severity
Moderate

## Description

Procmail checks both lock file size (`<= MAX_locksize`) and age
before force-unlocking stale locks (locking.c:56-57).  This
prevents treating large files as stale locks.

Corpmail's `lock_mtime()` only reads mtime; `try_force_unlock()`
in lockfile.rs checks age but not size.

## Impact

Force-unlock could remove legitimate files if they happen to be
at a lockfile path.
