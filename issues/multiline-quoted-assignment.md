# Multi-line quoted strings in assignments

Severity: low

Procmail's `readparse` is character-based and handles quoted strings that
span multiple lines.  For example:

    LOG="
    "

writes a newline to the log.  Rockmail's parser is line-based, so the
assignment value is truncated at the end of the first line and the
closing `"` is parsed as a separate (invalid) line.

## Location

- `src/config/parser/mod.rs` — `collect_continuation` joins lines only
  on trailing backslash; `parse_assignment` operates on a single
  collected line
- `src/variables/substitution/mod.rs` — substitution sees only what
  the parser hands it

## Suggested fix

Either switch assignment parsing to character-based reading (matching
procmail's `readparse`), or add a special case in `collect_continuation`
that detects an unmatched quote and consumes additional lines until the
quote is closed.
