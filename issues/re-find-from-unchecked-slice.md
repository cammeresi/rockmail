# `find_from` panics on out-of-bounds position

Severity: medium

`find_from` slices with `&text[pos..]`, which panics if `pos > text.len()`.
Current callers check bounds first, but the function is `pub` and a future
caller might not.

## Location

- `src/re/matcher.rs:310-313`

## Suggested fix

```rust
let m = self.regex.find(text.get(pos..)?)?;
```
