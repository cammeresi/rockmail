# `expand()` clones entire environment and message on every call

Severity: high

`expand()` unconditionally clones all environment variables and copies
`msg.as_bytes()` into a Vec to build a closure for backtick execution,
even when the string being expanded contains no backticks.

```rust
let envs: Vec<_> = self.env.iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect();
let input = msg.as_bytes().to_vec();
```

`expand()` is called 10+ times per recipe (conditions, actions, paths,
locks). For a 100-recipe rcfile with a 100-variable environment and a
10KB message, this is ~100MB of unnecessary allocation and copying.

## Location

- `src/engine/mod.rs:415-421`

## Suggested fix

Defer the closure construction to `subst_limited_with` so the env/message
are only cloned if a backtick is actually encountered. Or use a reference-
based approach that avoids cloning entirely.
