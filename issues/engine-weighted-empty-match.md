# Empty-match special cases missing in weighted scoring

## Component
`src/engine/mod.rs`

## Severity
Critical

## Description

When a regex produces a zero-width match (e.g. `^` or `\b`),
procmail has special handling (`misc.c:543-551`):

- If `0 < x < 1`: adds `weight / (1 - x)` (geometric series sum)
- If `x >= 1` and `weight != 0`: adds `MIN32` or `MAX32`
- Then breaks to avoid an infinite loop

Corpmail's `count_matches()` approach doesn't account for this.
A zero-width pattern would produce an infinite number of matches
(or the regex engine would report one match), and the scoring
would be wrong in either case.

## Procmail reference
`misc.c:543-557` — empty match and convergence detection.
