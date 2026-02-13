# No timestamp parsing in From_ lines

## Component
`src/mail/from_line.rs`

## Severity
Moderate

## Description

Procmail's `findtstamp()` (from.c lines 41-61) parses and validates
the timestamp portion of From_ lines.  This is used for generating
From_ lines with proper timestamps or refreshing them.

Corpmail can generate new From_ lines with the current time but
cannot parse, validate, or preserve timestamps from existing
From_ lines.  `envelope_sender()` extracts the sender but ignores
the timestamp entirely.

## Impact

Messages forwarded or modified may lose timestamp integrity.
