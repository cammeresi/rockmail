# No COMSAT notification support

## Component
`src/delivery/`

## Severity
Low

## Description

Procmail sends UDP notifications to the biff/comsat service on
delivery (`comsat.c`).  The `COMSAT` variable is declared in
corpmail's builtins but never used.

## Notes

COMSAT/biff is rarely used on modern systems.  This is a low
priority compatibility feature.
