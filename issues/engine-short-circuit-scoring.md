# Short-circuit in eval_conditions may differ from procmail

## Component
`src/engine/mod.rs`

## Severity
Low

## Description

Corpmail's `eval_conditions` returns early when any condition fails:

```rust
if !r.matched {
    return Ok((false, score));
}
```

Procmail processes all conditions even after a non-weighted failure,
continuing to track scores.  This affects what value `$=` (last
score) contains after a failed recipe — procmail may report a
partial score while rockmail reports whatever was accumulated before
the failing condition.

## Impact

Minor behavioral difference visible only through the `$=` variable
after recipe failure.
