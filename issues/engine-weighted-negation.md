# Negated weighted condition logic is wrong

## Component
`src/engine/mod.rs`

## Severity
Critical

## Description

Corpmail handles negation of weighted conditions by simply negating
the score:

```rust
score = if negate { -score } else { score }
```

Procmail has distinct iteration logic for negated weighted patterns
(`misc.c:532-535`).  For a negated pattern that does NOT match:

    score += weight  (single addition, no iteration)

For a negated pattern that DOES match, the condition contributes
nothing.  This is fundamentally different from negating the positive
score.

## Impact

Negated weighted conditions produce wrong scores, potentially
causing recipes to match or fail incorrectly.
