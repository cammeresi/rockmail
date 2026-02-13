# Score clamping differs from procmail

## Component
`src/engine/mod.rs`

## Severity
Moderate

## Description

Procmail clamps scores to `[MIN32, MAX32]` range and causes match
failure on underflow (`misc.c:622-625`):

```c
if(score<=MIN32)
    i=0;  // force match failure
```

Corpmail checks for NaN/infinity and clamps to 0:

```rust
if score.is_nan() || score.is_infinite() {
    0.0
}
```

The difference: procmail's underflow causes match failure; rockmail's
infinity becomes 0 (neutral).  Also, procmail uses integer clamping
(i32 range) while rockmail uses f64 throughout.

## Impact

Edge cases with very large or very small scores will behave
differently.
