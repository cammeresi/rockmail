# HOST has no gold test

Severity: low

Reassigning `HOST` triggers a reverse-DNS lookup that may reset the
value. This special side-effect behavior is untested against procmail.

Missing coverage:
- Assign `HOST=somevalue`, read it back
- Assign `HOST` to something that triggers reverse-DNS resolution

## Location

- `src/engine/mod.rs:380` (`set_var` match arm for `VAR_HOST`)

## Suggested fix

Add a gold test that assigns `HOST` and verifies the resulting value
matches procmail. The reverse-DNS behavior may be hard to test
deterministically.
