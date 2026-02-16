# `Matcher::find_from` has no tests

Severity: medium

`find_from` is a public method used by the scoring loop but has zero
direct test coverage. Edge cases like `pos == text.len()` and
`pos > text.len()` (which panics) are not exercised.

## Location

- `src/re/matcher.rs:310-313`
