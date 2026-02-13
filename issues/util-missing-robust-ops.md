# Missing robust/retryable I/O operations

## Component
`src/util/`

## Severity
Moderate

## Description

Procmail's `robust.c` provides retryable wrappers for system calls
that survive transient errors:

- `sfork()` — retryable fork (process table full)
- `ropen()` — retryable open (EINTR, ENFILE)
- `rpipe()` — retryable pipe (ENFILE)
- `rdup()` — retryable dup (ENFILE)
- `rclose()` — EINTR-immune close
- `rread()` / `rwrite()` — EINTR-immune I/O
- `ssleep()` — alarm-aware sleep

Corpmail has none of these.  Standard Rust I/O will fail on
transient errors instead of retrying.

## Impact

Under resource pressure (many processes, low file descriptors),
corpmail will fail where procmail would retry and succeed.
