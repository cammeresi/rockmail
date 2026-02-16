# Most Environment methods have no direct tests

Severity: medium

The substitution engine is well tested (54 tests), but `Environment`
methods are largely untested:

- `get_or_default` — fallback chain not verified
- `get_num` — numeric parsing with defaults
- `set_default` / `set_all_defaults` — bulk initialization
- `remove` — variable deletion
- `timeout` — Duration conversion

These are exercised indirectly through engine integration tests but
have no unit tests validating their individual behavior.

## Location

- `src/variables/environment.rs`
