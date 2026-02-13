# No lock timeout/retry (LOCKTIMEOUT, LOCKSLEEP)

## Component
`src/engine/mod.rs`

## Severity
Moderate

## Description

Procmail supports `LOCKTIMEOUT` (forced unlock after timeout) and
`LOCKSLEEP` (configurable retry interval between lock attempts).

Corpmail's `FileLock::acquire_temp()` attempts to acquire the lock
once.  If another process holds the lock, delivery fails immediately
instead of retrying with backoff.

## Expected behavior

Retry lock acquisition with `LOCKSLEEP` interval (default 8 seconds)
up to `LOCKTIMEOUT` (default 1024 seconds), forcibly removing stale
locks after the timeout.
