# `PatternError::Regex` variant never tested

Severity: low

Only `PatternError::TooLong` is tested (via the MAX_PATTERN_LEN check).
No test triggers an actual regex compilation error (unmatched `[`,
unmatched `(`, etc.) to exercise the `PatternError::Regex` path.

## Location

- `src/re/matcher.rs:64` (variant definition)
