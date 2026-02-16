# Dir delivery has no unit tests

Severity: low

Plain directory delivery (`deliver_dir` in maildir.rs) has gold tests
but no unit tests. Maildir, mbox, MH, and pipe all have dedicated unit
test files.

## Location

- `src/delivery/maildir.rs:121-141` (`deliver_dir`)
