# Regex recompiled on every condition evaluation

Severity: high

`eval_pattern` creates a new `Matcher` (compiling the regex) on every
call. Regex compilation is expensive relative to matching. For a
100-recipe rcfile with 1-2 conditions each, the same or similar patterns
may be compiled 100+ times.

## Location

- `src/engine/mod.rs:625`

## Suggested fix

Cache compiled `Matcher` objects, keyed by `(pattern, case_insensitive)`.
Patterns are expanded before matching, so caching must happen after
expansion (which limits hit rate but still helps for non-variable
patterns, which are the common case).
