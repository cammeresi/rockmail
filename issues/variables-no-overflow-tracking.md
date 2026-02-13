# No PROCMAIL_OVERFLOW tracking

## Component
`src/variables/`

## Severity
Low

## Description

Procmail sets `PROCMAIL_OVERFLOW=yes` (variables.c:156-158) when
variable expansion exceeds buffer limits.  Corpmail has the
`VAR_PROCMAIL_OVERFLOW` constant but no mechanism to set it.
