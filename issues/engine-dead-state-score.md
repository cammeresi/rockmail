# State::score field is dead code

## Component
`src/engine/mod.rs`

## Severity
Low

## Description

The `State` struct has a `score` field marked `#[allow(dead_code)]`:

```rust
pub struct State {
    pub last_cond: bool,
    pub last_succ: bool,
    pub prev_cond: bool,
    #[allow(dead_code)]
    pub score: f64,
    pub depth: usize,
}
```

The actual score is stored in `SubstCtx::last_score`.  This field
is never read or written — it appears to be left over from a
refactoring.

## Fix

Remove the field or consolidate with `SubstCtx::last_score`.
