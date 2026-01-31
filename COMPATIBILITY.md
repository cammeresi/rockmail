# Corpmail Compatibility Notes

This file documents intentional differences from procmail behavior.

## Regex Word Boundaries

Procmail's `\<` and `\>` consume a non-word character:
- `\<` matches a non-word char (or start of text) before a word
- `\>` matches a non-word char (or end of text) after a word

Corpmail uses `\b` (zero-width word boundary) for both. This means:
- The boundary character is not consumed/included in the match
- No distinction between word-start vs word-end

Example where behavior differs:
- Pattern `\<word\>` against "a word here"
  - Procmail: matches " word " (spaces consumed)
  - Corpmail: matches "word" (zero-width boundaries)

This difference affects what gets captured with `\/` but the match
success/failure is the same in typical usage.

## Double Caret in Character Classes

The `^^` anchor syntax is not character-class-aware. A pattern like `[^^]`
(intended as a negated character class containing `^`) may be misinterpreted.
Workaround: use `[^\\^]` or reorder the class contents.

## Trailing Backslash

A trailing backslash in a pattern (e.g., `foo\`) is treated as a literal
backslash, matching `foo\`. This matches procmail behavior.
