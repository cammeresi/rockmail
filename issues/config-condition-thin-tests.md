# Condition parser tests are thin

Severity: medium

Unit tests cover the four basic `Condition::parse` cases (regex, negated
regex, size, shell) but skip several condition types.

## Location

- `src/config/condition/tests.rs`

## Suggested fix

Add unit tests for:

- Weighted conditions (`w^x` syntax)
- `$` variable-substitution flag (`Condition::Subst`)
- `??` variable conditions
- Negated size conditions
