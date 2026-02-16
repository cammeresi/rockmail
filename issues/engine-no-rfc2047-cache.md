# RFC 2047 header decoding repeated on every condition

Severity: low

`grep_text` and `get_variable_text` call `rfc2047::decode()` on every
invocation. If multiple conditions in the same recipe match against
headers, the same headers are decoded multiple times.

For headers without encoded words the fast path returns a borrowed
`Cow`, so the cost is just the scan for `=?`. For headers with encoded
words, each decode allocates.

## Location

- `src/engine/mod.rs:485-503` (`grep_text`, `get_variable_text`)
