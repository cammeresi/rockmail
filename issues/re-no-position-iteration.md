# Regex API lacks position-based iteration for scoring

## Component
`src/re/matcher.rs`

## Severity
High

## Description

Procmail's `bregexec()` returns a pointer to the position AFTER
each match, allowing iterative matching for weighted scoring:

```c
while ((chp2 = bregexec(re, text, chp, len, igncase))) {
    score += weight;
    weight *= xponent;
    if (chp >= chp2) break;  // empty match
    chp = chp2;
}
```

Corpmail's `exec()` returns a MatchResult struct and
`count_matches()` returns only a count.  Neither exposes match
positions for the engine to iterate with weight decay.

This is the root cause of several weighted scoring issues
(see engine-weighted-*.md).

## Location
`src/re/matcher.rs` lines 277-310.
