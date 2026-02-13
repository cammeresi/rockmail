# No ENAMETOOLONG handling

## Component
`src/locking/dotlock.rs`, `src/bin/lockfile.rs`

## Severity
Moderate

## Description

Procmail handles ENAMETOOLONG by truncating the base path and
retrying (locking.c:81-89).  Corpmail fails immediately on
path-too-long errors with no recovery.
