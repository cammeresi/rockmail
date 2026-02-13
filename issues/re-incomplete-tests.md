# Incomplete regex extension test coverage

## Component
`src/re/matcher/tests.rs`

## Severity
Moderate

## Description

Missing test scenarios:

**^^ anchors:**
- `^^` inside groups or alternations
- Triple caret `^^^` behavior
- `^^` combined with `\/` capture

**\/ capture:**
- Empty capture (pattern ends at `\/`)
- Multiple groups before capture point
- Double `\/` in same pattern
- Capture before newline (`.` vs `\n`)

**Macros:**
- Multiple macro expansion in single pattern
- TO_ vs TO distinction (address vs word boundary)
- FROM_DAEMON vs FROM_MAILER feature overlap
- Resent-* header variants with macros

**Multiline:**
- `^` at true start vs after newline
- `$` at true end vs before newline
- Character classes with newlines in negation
