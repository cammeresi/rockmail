# Missing Content-Length header processing in formail

## Component
`src/bin/formail/main.rs`

## Severity
Moderate

## Description

During mailbox splitting, procmail uses Content-Length headers to
skip exactly that many bytes of body, warning if the declared length
exceeds actual length.  Corpmail has no equivalent logic.

## Procmail reference
`formail.c` lines 758-772.
