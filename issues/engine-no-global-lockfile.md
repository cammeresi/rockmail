# Global LOCKFILE semaphore not implemented

Procmail's `LOCKFILE` variable acts as a global semaphore — when set, procmail
acquires the named lockfile before processing any recipes and holds it until
the variable is reassigned or unset. This prevents concurrent delivery.

Currently `VAR_LOCKFILE` is declared in `variables/builtins.rs` but never
read by the engine.
