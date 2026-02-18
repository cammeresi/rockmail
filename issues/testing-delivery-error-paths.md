# Delivery error paths are untested

Severity: medium

Happy paths are well tested for mbox (16), maildir (11), MH (6), and
pipe (10), but error/failure paths are not exercised.

Missing coverage:
- `src/delivery/mbox/mod.rs`: truncation on write failure, fsync failure
- `src/delivery/pipe/mod.rs`: partial write / large message buffering
- `src/delivery/mod.rs:153` `update_perms()` has no unit tests
- `src/delivery/mod.rs:170` `link_secondary()` has no unit tests
- All methods: parent directory doesn't exist
