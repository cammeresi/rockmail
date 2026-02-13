# Geometric series tail sum missing in weighted scoring

## Component
`src/engine/mod.rs`

## Severity
Critical

## Description

When `0 < x < 1` and regex matches end, procmail adds the convergent
tail sum `weight/(1-x)` to account for the infinite remaining terms
of the geometric series (`misc.c:548-551`).

Corpmail uses a closed-form formula `w * (x^n - 1) / (x - 1)` based
on the match count, which only accounts for actual matches found.
It does not add the tail sum for remaining potential matches.

Procmail's iterative approach:
```
for each match:
    score += weight
    weight *= exponent
if 0 < x < 1:
    score += weight / (1 - x)   // tail sum
```

## Impact

Scores differ significantly when `x < 1` and many matches occur.
Corpmail underscores relative to procmail.

## Procmail reference
`misc.c:529-562` — iterative matching with tail sum.
