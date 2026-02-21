# LOCKFILE has no gold test

Severity: medium

The `LOCKFILE` variable acquires a global lockfile on assignment and
releases it when cleared. This side-effect logic in the engine is
untested against procmail.

Missing coverage:
- Assign `LOCKFILE`, verify lock file is created
- Clear `LOCKFILE`, verify lock file is removed
- Reassign `LOCKFILE`, verify old lock released and new one acquired

## Location

- `src/engine/mod.rs:396` (`set_var` → `set_globlock`)

## Suggested fix

Add a gold test that assigns `LOCKFILE=$MAILDIR/global.lock`, delivers a
message, and verifies the lockfile lifecycle matches procmail.
