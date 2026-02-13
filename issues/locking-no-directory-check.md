# No directory check in force-unlock

## Component
`src/bin/lockfile.rs`

## Severity
Low

## Description

Procmail prevents force-unlocking directories by checking
`S_ISDIR(stbuf.st_mode)` before unlinking (locking.c:63).
Corpmail's `try_force_unlock()` has no such check.
