# Shell exit code not used in weighted scoring

## Component
`src/engine/mod.rs`

## Severity
Critical

## Description

Procmail uses the shell command's exit code (0-255) in weighted
scoring (`misc.c:580-587`):

- Success (exit 0): `score += weight`
- Failure (exit N>0): `score += xponent` (not weight)

Corpmail treats the shell result as binary:

```rust
let score = if ok { wt.w } else { wt.x };
```

This is close but not quite right — procmail uses the exit code
value itself as a multiplier in the negated case, iterating
`weight *= xponent` for each count down from the exit code.

## Impact

Cannot accurately rank based on shell command exit codes.  A command
returning exit 50 should contribute differently than exit 1.

## Procmail reference
`misc.c:577-590` — shell command scoring with exit code.
