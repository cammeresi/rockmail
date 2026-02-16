# Shell conditions have minimal test coverage

Severity: high

`eval_shell` (engine/mod.rs:586) has only one gold test
(`subst_in_shell_condition`). No unit tests.

Missing coverage:
- Negated shell conditions (`! ? cmd`)
- Shell exit code-based weighted scoring (engine/mod.rs:603-617)
- Timeout behavior
- Error handling (command not found, etc.)
