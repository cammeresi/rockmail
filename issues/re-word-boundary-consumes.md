# Word boundaries (\<, \>) differ from procmail

## Component
`src/re/matcher.rs`

## Severity
High

## Description

Procmail's `\<` and `\>` match a non-word character (or text
boundary) and CONSUME that character — it's included in the match.
Pattern `\<word\>` against "a word here" matches " word ".

Corpmail translates `\<`/`\>` to `\b` (zero-width word boundary),
which does NOT consume any character.  The same pattern matches
"word" without the surrounding spaces.

This is documented in COMPATIBILITY.md but affects MATCH capture
content when `\/` is used with word boundaries.

## Location
`src/re/matcher.rs` lines 93-101, 138-140.
