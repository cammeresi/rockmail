# Unsanitized sender in generated From_ line

## Component
`src/bin/formail/main.rs`

## Severity
Low

## Description

`generate_from_line()` (line 599-604) puts a sender address extracted
from untrusted headers into mbox From_ line output via `format!`.

The `extract_address()` and `from_line_addr()` functions strip
whitespace, so the output is a single token and the mbox format
remains valid.  However, the sender is not further sanitized — it
could contain shell metacharacters if the From_ line is later parsed
by a naive script.

This matches procmail's behavior.
