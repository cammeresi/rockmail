# `run_trap()` has no unit tests

Severity: high

`Engine::run_trap()` at `src/engine/mod.rs:1484` is a public function with
no unit tests.  Integration tests cover the happy path (`trap_runs_on_exit`,
`trap_receives_message_on_stdin`, `trap_exitcode_available`,
`trap_exit_overrides_exitcode`) but the function itself is not exercised
in isolation.
