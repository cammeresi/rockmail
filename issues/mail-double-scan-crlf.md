# CRLF normalization scans message twice

Severity: low

`normalize_crlf` calls `data.contains(&b'\r')` to check whether CR
bytes exist, then `strip_cr` scans the data again to remove them.
Similarly, `unfold_header` checks `contains(&b'\n')` before scanning
again.

For typical email (no CRLFs), the fast-path borrow is fine. When CRLFs
are present, the double scan is minor overhead on already-small data
(headers).

## Location

- `src/mail/message.rs:8-12` (`normalize_crlf` + `strip_cr`)
- `src/mail/message.rs:57-63` (`unfold_header`)
