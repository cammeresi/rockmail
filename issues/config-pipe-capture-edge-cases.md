# No edge-case tests for pipe capture syntax

## Component
`src/config/action.rs`

## Severity
Low

## Description

The pipe capture syntax `VAR=| cmd` has only one happy-path test
(`pipe_capture()`).  Missing edge-case coverage:

- `=| cmd` (empty variable name)
- `_=| cmd` (underscore-only name)
- `VAR =| cmd` (space before `=`)
- `VAR= | cmd` (space after `=`)
- Names with invalid characters

Procmail typically trims whitespace liberally; corpmail's behavior
at these boundaries is unspecified.
