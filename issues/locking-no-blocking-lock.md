# No blocking kernel lock (fdlock equivalent)

## Component
`src/locking/flock.rs`

## Severity
High

## Description

Procmail's `fdlock()` (locking.c:188-275) performs blocking kernel
locks using `fcntl(fd, F_SETLKW, ...)` with signal-aware retry.
It supports fallback between fcntl, lockf, and flock.

Corpmail uses `FlockArg::LockExclusiveNonblock` only (flock.rs
line 35).  There is no blocking lock variant, no fcntl range
lock support, and no signal-aware retry.

## Impact

Cannot sustain locks during mail delivery the way procmail does.
