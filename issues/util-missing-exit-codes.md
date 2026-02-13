# Missing exit codes from sysexits.h

## Component
`src/util/mod.rs`

## Severity
Low

## Description

Only 7 exit codes are defined.  Missing from procmail usage:

- `EX_DATAERR` (65) — invalid input data
- `EX_NOUSER` (67) — user not found
- `EX_OSFILE` (72) — OS file table exhaustion
- `EX_IOERR` (74) — I/O errors
- `EX_NOPERM` (77) — permission denied
