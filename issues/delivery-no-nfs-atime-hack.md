# No NFS atime hack

## Component
`src/delivery/mbox.rs`

## Severity
Low

## Description

Procmail has a `NO_NFS_ATIME_HACK` guard (`mailfold.c:95-101`) that
writes the first byte, checks if `atime == mtime`, and sleeps if
needed.  This tricks NFS into proper cache invalidation by ensuring
access time differs from modification time.

Corpmail has no equivalent.  On NFS mounts, other processes might
not see new mail until the cache expires.

## Notes

This is a legacy compatibility concern.  NFS mbox delivery is
increasingly rare.
