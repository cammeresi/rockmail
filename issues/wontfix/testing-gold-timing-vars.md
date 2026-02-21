# LOCKSLEEP, LOCKTIMEOUT, TIMEOUT, NORESRETRY, SUSPEND have no gold tests

Severity: low

These timing/resource variables are parsed and stored but never tested
against procmail. They affect lock retry intervals, stale lock breaking,
child process timeouts, and resource shortage handling.

Testing these in gold tests is impractical because they require timing-
dependent scenarios (stale locks, hung processes, resource exhaustion).
Unit tests for the parsing/clamping logic would be more appropriate.

## Location

- `src/variables/builtins.rs:84-88` (defaults)

## Suggested fix

Add unit tests in `src/variables/` or `src/engine/` that verify parsing
and value clamping for each variable.
