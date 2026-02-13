# No signal safety during lock acquisition

## Component
`src/locking/`, `src/util/signals.rs`

## Severity
Moderate

## Description

Procmail uses `lcking |= lck_DELAYSIG` to delay signal processing
during critical locking sections, preventing signal handlers from
interrupting atomic operations.

Corpmail has no critical section protection.  The `create_lock()`
operation could be interrupted mid-way by a signal, leaving
temporary files behind.

## Procmail reference
`locking.c:48` — signal delay during lock loop.
