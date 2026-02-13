# Signal handling (SIGUSR1/2) not integrated

## Component
`src/engine/mod.rs`

## Severity
Low

## Description

Procmail uses SIGUSR1 to toggle verbose mode at runtime.  Corpmail
has a `signals.rs` module and the engine has `set_verbose()`, but
the signal handler is not wired up to the engine.

## Impact

Cannot toggle verbose logging at runtime for debugging live
delivery issues.
