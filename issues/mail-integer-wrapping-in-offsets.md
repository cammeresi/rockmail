# Integer wrapping in Message offset adjustments

Severity: medium

`set_envelope_sender` and `refresh_envelope_sender` compute a delta via
`isize` casts and apply it to `header_end` and `body_start`:

```rust
let delta = from.len() as isize - old as isize;
self.header_end = (self.header_end as isize + delta) as usize;
self.body_start = (self.body_start as isize + delta) as usize;
```

If the result goes negative, the `as usize` cast wraps to a huge value,
causing a panic on the next slice access.

`strip_from_line` has a similar issue with unchecked subtraction:

```rust
self.header_end -= end;
self.body_start -= end;
```

In practice these are reached only through internal code paths that
construct valid messages, so exploitation via email input is unlikely.

## Location

- `src/mail/message.rs:324-326` (`set_envelope_sender`)
- `src/mail/message.rs:345-347` (`refresh_envelope_sender`)
- `src/mail/message.rs:357-358` (`strip_from_line`)

## Suggested fix

Add `debug_assert!` guards or use saturating/checked arithmetic.
