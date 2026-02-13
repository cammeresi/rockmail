# Signal handling is minimal compared to procmail

## Component
`src/util/signals.rs`

## Severity
Moderate

## Description

Current implementation handles 4 signals (SIGHUP, SIGINT, SIGQUIT,
SIGTERM) with a single atomic flag, and ignores SIGPIPE.

Procmail has significantly more:

**Missing signals:**
- SIGALRM — timeout handling for child processes
- SIGUSR1/SIGUSR2 — verbose toggle at runtime
- SIGCHLD — explicitly set to SIG_DFL
- SIGXCPU/SIGXFSZ — ignored
- SIGLOST — ignored

**Missing behaviors:**
- Per-signal actions (requeue vs bounce vs terminate)
- Context-specific handlers (main vs child vs filter)
- Critical section protection (`lcking |= lck_DELAYSIG`)
- Safe signal registration (`qsignal` wrapper that checks
  for pre-existing SIG_IGN)
- Timeout/alarm infrastructure (alarm setup/reset)

**Missing process management:**
- Zombie collection (`waitpid(-1, WNOHANG)`)
- Child process timeout/termination via SIGTERM
