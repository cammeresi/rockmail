# SHELL, SHELLFLAGS, SHELLMETAS, PATH have no gold tests

Severity: low

These variables control how pipe commands are executed. The defaults are
exercised implicitly by every pipe test, but reassigning them is never
tested against procmail.

Missing coverage:
- Change `SHELL` to a different interpreter
- Change `SHELLMETAS` to alter direct-exec vs shell-exec threshold
- Change `PATH` and verify command resolution changes
- Change `SHELLFLAGS` (unlikely to matter in practice)

## Location

- `src/engine/mod.rs` (pipe execution logic)

## Suggested fix

Add gold tests that reassign `SHELLMETAS` (e.g. clear it to force
direct exec) and verify the behavior matches procmail.
