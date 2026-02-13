# No file permission updates on delivery

## Component
`src/delivery/`

## Severity
Moderate

## Description

Procmail updates folder permissions after delivery using
`chmod(boxname, mode|UPDATE_MASK)` if the required bits are missing
(`mailfold.c:220-221,308-310,322-325`).

Corpmail never calls `chmod()` on created folders or files.
Permissions depend entirely on the process umask, which may not
match what procmail would produce.

## Impact

Delivered folders/files may have incorrect permissions, potentially
causing read failures for MUAs or security issues if too permissive.
