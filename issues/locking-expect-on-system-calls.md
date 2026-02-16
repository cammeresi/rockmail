# `expect()` on system calls in dotlock

Severity: low

`compute_safe_hostname` and `random_u64` use `expect()` on `gethostname()`
and `/dev/urandom` operations. These should never fail in a normal
environment, but a hardened deployment (chroot without `/dev/urandom`,
unusual hostname encoding) would panic instead of returning a clean error.

## Location

- `src/locking/dotlock.rs:19-20` (`gethostname` and UTF-8 conversion)
- `src/locking/dotlock.rs:45-47` (`/dev/urandom` open and read)
