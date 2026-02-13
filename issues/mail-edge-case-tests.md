# Missing edge case tests for mail module

## Component
`src/mail/message/tests.rs`, `src/mail/from_line/tests.rs`

## Severity
Moderate

## Description

Missing test coverage for:

- Binary null bytes in headers
- Very long header lines (RFC 5322 ~998 char limit)
- Header with empty value ("Header:\n")
- UTF-8 invalid sequences in headers (from_utf8_lossy fallback)
- Empty From_ line ("From \n") — envelope_sender returns Some("")
- From_ with no space after sender ("From sender" no newline)
- Content-Length mismatch with actual body length
- CRLF inside header continuation lines
