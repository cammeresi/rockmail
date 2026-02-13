# Macro expansion only replaces first occurrence

## Component
`src/re/matcher.rs`

## Severity
Low

## Description

`expand_macros()` uses `result.find(key)` which only finds the
first occurrence.  A pattern containing a macro twice (e.g.
`^FROM_DAEMON|^FROM_MAILER`) would only expand the first one.

In practice procmail patterns rarely use multiple macros, but
the behavior differs from procmail which would expand all.

## Location
`src/re/matcher.rs` lines 73-89.
