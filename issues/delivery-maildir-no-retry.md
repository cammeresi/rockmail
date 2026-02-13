# No Maildir retry logic on filename collision

## Component
`src/delivery/maildir.rs`

## Severity
High

## Description

Procmail retries up to 5 times (`MAILDIRretries` in `config.h:243`)
when `hard_link()` or `rename()` fails with EEXIST during Maildir
delivery (`mailfold.c:258-279`).

Corpmail generates a unique filename and attempts delivery once.
Under high load with concurrent deliveries, a filename collision
causes immediate failure instead of retry with a new name.

## Expected behavior

Retry filename generation and delivery up to N times on EEXIST.
