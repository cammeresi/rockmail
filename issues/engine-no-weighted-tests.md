# Insufficient test coverage for weighted scoring

## Component
`src/engine/tests.rs`, `tests/rockmail_gold.rs`

## Severity
High

## Description

Weighted scoring has minimal test coverage.  The existing tests only
cover basic positive/zero/negated regex cases.  Missing:

**Unit tests:**
- Weighted size conditions
- Weighted shell conditions with varying exit codes
- Multiple weighted conditions combined in one recipe
- Mixed weighted and non-weighted conditions
- Score at exactly 0.0 vs epsilon
- Convergence behavior with `0 < x < 1` and many matches
- Empty-match edge cases

**Gold tests:**
- No gold tests for weighted scoring at all
- `RcBuilder` doesn't support generating weighted conditions

**$= variable:**
- No tests verify `$=` contains the correct last score
- Test infrastructure doesn't expose `engine.ctx.last_score`

## Impact

The most complex part of condition evaluation has the least test
coverage.
