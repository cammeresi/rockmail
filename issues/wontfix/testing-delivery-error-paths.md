# Delivery error paths are untested

Severity: medium

Happy paths are well tested for mbox (16), maildir (11), MH (6), and
pipe (10), but error/failure paths are not exercised.

Remaining coverage gaps:
- `src/delivery/mbox/mod.rs`: truncation on write failure, fsync failure
  (requires injecting I/O errors mid-write, not easily testable)
