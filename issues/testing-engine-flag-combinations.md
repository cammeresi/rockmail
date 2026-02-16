# Recipe flag combinations largely untested

Severity: medium

Individual flags are tested, but combinations are not. Several flags
have no dedicated tests at all:

- `D` (case sensitive) — only implicit via gold tests
- `i` (ignore errors) — not tested
- `E` (else-if) — not tested as distinct from `e`
- `w` (wait + quiet) — quiet behavior not tested
- Combined flags like `fwi`, `HBD`, `cw` — not tested
