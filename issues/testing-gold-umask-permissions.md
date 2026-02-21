# UMASK has no gold test

Severity: low

Assigning `UMASK` changes the file creation mask. The engine parses the
octal value and calls `umask()` on assignment, but this is never tested
against procmail.

Missing coverage:
- Set `UMASK=022`, deliver to mbox, verify file permissions
- Change `UMASK` between deliveries, verify different permissions

## Location

- `src/engine/mod.rs:358` (`set_var` match arm for `VAR_UMASK`)

## Suggested fix

Add a gold test that sets `UMASK`, delivers a message, and compares file
permissions between rockmail and procmail.
