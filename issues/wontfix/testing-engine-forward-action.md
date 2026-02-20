# Forward action has no actual delivery tests

Severity: high

The forward action (`!`) is only tested via dryrun logging
(tests/rockmail.rs:492). No test actually invokes sendmail or validates
the addresses passed to it.

Missing coverage:
- Actual sendmail invocation (needs mock/stub)
- Multiple addresses
- SENDMAILFLAGS behavior
- Error handling (sendmail not found, exit code)

## Comments

This feature is difficult to test automatically.  It has been manually
tested and found to work.
