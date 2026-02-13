# set_envelope_sender() and strip_from_line() are untested

## Component
`src/mail/message.rs`

## Severity
High

## Description

`set_envelope_sender()` (lines 302-330) and `strip_from_line()`
(lines 333-341) mutate the message by adjusting data, header_end,
and body_start offsets.  Neither has any test coverage.

`set_envelope_sender()` has duplicated logic to find the newline
position after "From " (lines 305-306 and 320-321) and a complex
offset calculation using isize casts that could panic on edge cases.

## Missing tests

- set_envelope_sender with no existing From_ line
- set_envelope_sender with existing From_ line
- set_envelope_sender with very long sender string
- strip_from_line correctness
- strip_from_line on message without From_ line (no-op)
