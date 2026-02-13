# No mbox truncation on partial write failure

## Component
`src/delivery/mbox.rs`

## Severity
High

## Description

Procmail tracks the file position with `lseek(SEEK_END)` before
writing and truncates back to that position if the write fails
partway through (`mailfold.c:32,83,92,240-241`).

Corpmail has no equivalent recovery logic.  A partial write (e.g.
from ENOSPC or EDQUOT) leaves a corrupted mbox file with an
incomplete message that breaks subsequent parsing.

## Expected behavior

Before writing, record the file length.  On any write error,
truncate the file back to the recorded length and report the error.

## Test coverage

No tests for delivery error recovery paths.
