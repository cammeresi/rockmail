# LOG variable has no gold test

Severity: medium

Assigning `LOG` immediately appends the value to the current logfile.
This is distinct from `LOGFILE` (which sets the log destination). The
write-on-assign behavior is untested against procmail.

Missing coverage:
- `LOG=text` with `LOGFILE` set, verify text appears in log
- Multiple `LOG` assignments, verify all appear in order

## Location

- `src/engine/mod.rs:397` (`set_var` match arm for `VAR_LOG`)

## Suggested fix

Add a gold test that sets `LOGFILE`, then assigns `LOG` one or more
times, and compares the resulting logfile content.
